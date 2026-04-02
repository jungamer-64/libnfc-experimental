use super::connstring::{build_path_speed_connstring, decode_path_speed_descriptor};
use super::pn53x::{Pn53xDevice, Pn53xProfile, Pn53xTransport, is_ack_frame};
use proximate_driver::{ConnectionString, Context, DeviceHandle, Driver, Error, ScanType};
use std::fs;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

#[cfg(target_os = "linux")]
use rustix::event::{PollFd, PollFlags, Timespec, poll};
#[cfg(target_os = "linux")]
use rustix::fd::OwnedFd;
#[cfg(target_os = "linux")]
use rustix::fs::{FlockOperation, Mode, OFlags, flock, open};
#[cfg(target_os = "linux")]
use rustix::io::{ioctl_fionread, read, write};
#[cfg(target_os = "linux")]
use rustix::termios::{OptionalActions, QueueSelector, Termios, tcflush, tcgetattr, tcsetattr};

#[cfg_attr(not(any(test, libnfc_driver_pn532_uart)), allow(dead_code))]
const DRIVER_NAME: &str = "pn532_uart";
#[cfg_attr(not(any(test, libnfc_driver_pn532_uart)), allow(dead_code))]
const DEFAULT_SPEED: u32 = 115_200;
#[cfg_attr(not(any(test, libnfc_driver_pn532_uart)), allow(dead_code))]
const PROBE_TIMEOUT_MS: i32 = 250;
const NFC_EIO: i32 = -1;
const NFC_ETIMEOUT: i32 = -6;
const NFC_EOPABORTED: i32 = -7;
const WAKEUP_FRAME: [u8; 16] = [
    0x55, 0x55, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

#[cfg_attr(not(any(test, libnfc_driver_pn532_uart)), allow(dead_code))]
pub(crate) struct Pn532UartDriver;

#[cfg_attr(not(any(test, libnfc_driver_pn532_uart)), allow(dead_code))]
impl Pn532UartDriver {
    pub(crate) const fn new() -> Self {
        Self
    }
}

impl Driver for Pn532UartDriver {
    fn name(&self) -> &str {
        DRIVER_NAME
    }

    fn scan_type(&self) -> ScanType {
        ScanType::Intrusive
    }

    fn scan(&self, _context: &Context) -> Result<Vec<ConnectionString>, Error> {
        let mut devices = Vec::new();

        for path in list_candidate_paths() {
            let Ok(connstring) = build_path_speed_connstring(DRIVER_NAME, &path, DEFAULT_SPEED)
            else {
                continue;
            };

            #[cfg(target_os = "linux")]
            {
                let Ok(port) = UartPort::open(&path, DEFAULT_SPEED) else {
                    continue;
                };
                if Pn53xDevice::probe_with_profile(
                    format!("PN532 UART ({path})"),
                    connstring.clone(),
                    Pn53xProfile::pn532(DRIVER_NAME),
                    port,
                    PROBE_TIMEOUT_MS,
                )
                .is_ok()
                {
                    devices.push(connstring);
                }
            }

            #[cfg(not(target_os = "linux"))]
            {
                devices.push(connstring);
            }
        }

        Ok(devices)
    }

    fn open(
        &self,
        _context: &Context,
        connstring: &ConnectionString,
    ) -> Result<Box<dyn DeviceHandle>, Error> {
        let descriptor = decode_path_speed_descriptor(connstring, DRIVER_NAME, DEFAULT_SPEED)?;
        #[cfg(target_os = "linux")]
        {
            let port = UartPort::open(&descriptor.path, descriptor.speed)?;
            let device = Pn53xDevice::probe_with_profile(
                format!("PN532 UART ({})", descriptor.path),
                connstring.clone(),
                Pn53xProfile::pn532(DRIVER_NAME),
                port,
                PROBE_TIMEOUT_MS,
            )?;
            Ok(Box::new(device))
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = descriptor;
            Err(Error::DriverOpenFailed(
                "pn532_uart is only available on Linux in this phase".into(),
            ))
        }
    }
}

pub(crate) fn list_candidate_paths() -> Vec<String> {
    let mut ports = Vec::new();
    let Ok(entries) = fs::read_dir("/dev") else {
        return ports;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !serial_name_prefixes()
            .iter()
            .any(|prefix| name.starts_with(prefix))
        {
            continue;
        }
        if !name
            .bytes()
            .last()
            .is_some_and(|byte| byte.is_ascii_digit())
        {
            continue;
        }
        ports.push(format!("/dev/{name}"));
    }

    ports.sort();
    ports
}

fn serial_name_prefixes() -> &'static [&'static str] {
    &["ttyUSB", "ttyS", "ttyACM", "ttyAMA", "ttyO"]
}

#[cfg(target_os = "linux")]
pub struct UartPort {
    fd: OwnedFd,
    original_termios: Termios,
    read_buffer: Vec<u8>,
    abort_requested: Arc<AtomicBool>,
}

#[cfg(target_os = "linux")]
impl UartPort {
    pub fn open(path: &str, speed: u32) -> Result<Self, Error> {
        let fd = open(
            path,
            OFlags::RDWR | OFlags::NONBLOCK | OFlags::NOCTTY,
            Mode::empty(),
        )
        .map_err(|error| {
            Error::DriverOpenFailed(format!("failed to open {path}: {}", error.raw_os_error()))
        })?;

        flock(&fd, FlockOperation::NonBlockingLockExclusive).map_err(|_| {
            Error::DriverOpenFailed(format!("serial port {path} is already in use"))
        })?;

        let original_termios = tcgetattr(&fd).map_err(|_| {
            Error::DriverOpenFailed(format!("failed to read terminal settings for {path}"))
        })?;
        let mut configured = original_termios.clone();
        configured.make_raw();
        configured
            .set_speed(speed)
            .map_err(|_| Error::DriverOpenFailed(format!("unsupported UART speed {speed}")))?;
        configured.special_codes[rustix::termios::SpecialCodeIndex::VMIN] = 0;
        configured.special_codes[rustix::termios::SpecialCodeIndex::VTIME] = 0;
        tcsetattr(&fd, OptionalActions::Now, &configured).map_err(|_| {
            Error::DriverOpenFailed(format!("failed to configure UART port {path}"))
        })?;
        tcflush(&fd, QueueSelector::IFlush).map_err(|_| {
            Error::DriverOpenFailed(format!("failed to flush UART input for {path}"))
        })?;

        Ok(Self {
            fd,
            original_termios,
            read_buffer: Vec::new(),
            abort_requested: Arc::new(AtomicBool::new(false)),
        })
    }

    pub(crate) fn flush_input(&mut self) -> Result<(), Error> {
        tcflush(&self.fd, QueueSelector::IFlush)
            .map_err(|_| device_error("uart_flush_input", NFC_EIO))?;
        self.read_buffer.clear();

        let mut available = ioctl_fionread(&self.fd).unwrap_or(0) as usize;
        while available > 0 {
            let chunk_len = available.min(256);
            let mut scratch = vec![0u8; chunk_len];
            match read(&self.fd, scratch.as_mut_slice()) {
                Ok(0) => break,
                Ok(read_len) => {
                    if read_len >= available {
                        break;
                    }
                    available -= read_len;
                }
                Err(_) => break,
            }
        }
        Ok(())
    }

    fn wait_for(&self, flags: PollFlags, timeout_ms: i32) -> Result<(), Error> {
        if self.abort_requested.swap(false, Ordering::SeqCst) {
            return Err(device_error("uart_abort", NFC_EOPABORTED));
        }

        let mut pollfd = [PollFd::new(&self.fd, flags)];
        let timeout = timeout_spec(timeout_ms);
        let ready =
            poll(&mut pollfd, timeout.as_ref()).map_err(|_| device_error("uart_poll", NFC_EIO))?;
        if ready == 0 {
            return Err(device_error("uart_poll", NFC_ETIMEOUT));
        }

        let revents = pollfd[0].revents();
        if revents.intersects(PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL) {
            return Err(device_error("uart_poll", NFC_EIO));
        }
        if !revents.intersects(flags) {
            return Err(device_error("uart_poll", NFC_EIO));
        }
        Ok(())
    }

    pub(crate) fn write_all(&mut self, payload: &[u8], timeout_ms: i32) -> Result<(), Error> {
        let mut written = 0usize;
        while written < payload.len() {
            self.wait_for(PollFlags::OUT, timeout_ms)?;
            let len = write(&self.fd, &payload[written..])
                .map_err(|_| device_error("uart_send", NFC_EIO))?;
            if len == 0 {
                return Err(device_error("uart_send", NFC_EIO));
            }
            written += len;
        }
        Ok(())
    }

    fn fill_read_buffer(&mut self, timeout_ms: i32) -> Result<(), Error> {
        self.wait_for(PollFlags::IN, timeout_ms)?;
        let available = ioctl_fionread(&self.fd).unwrap_or(1).max(1) as usize;
        let mut chunk = vec![0u8; available.min(512)];
        let len = read(&self.fd, chunk.as_mut_slice())
            .map_err(|_| device_error("uart_receive", NFC_EIO))?;
        if len == 0 {
            return Err(device_error("uart_receive", NFC_EIO));
        }
        self.read_buffer.extend_from_slice(&chunk[..len]);
        Ok(())
    }

    pub(crate) fn read_exact(&mut self, buffer: &mut [u8], timeout_ms: i32) -> Result<(), Error> {
        let mut filled = 0usize;
        while filled < buffer.len() {
            if !self.read_buffer.is_empty() {
                let available = (buffer.len() - filled).min(self.read_buffer.len());
                buffer[filled..filled + available].copy_from_slice(&self.read_buffer[..available]);
                self.read_buffer.drain(..available);
                filled += available;
                continue;
            }
            self.fill_read_buffer(timeout_ms)?;
        }
        Ok(())
    }

    pub(crate) fn read_frame_into(
        &mut self,
        buffer: &mut [u8],
        timeout_ms: i32,
    ) -> Result<usize, Error> {
        loop {
            if let Some(frame_len) = expected_frame_len(&self.read_buffer)?
                && self.read_buffer.len() >= frame_len
            {
                if frame_len > buffer.len() {
                    return Err(Error::BufferTooSmall {
                        needed: frame_len,
                        available: buffer.len(),
                    });
                }
                buffer[..frame_len].copy_from_slice(&self.read_buffer[..frame_len]);
                self.read_buffer.drain(..frame_len);
                return Ok(frame_len);
            }

            self.fill_read_buffer(timeout_ms)?;
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for UartPort {
    fn drop(&mut self) {
        let _ = tcsetattr(&self.fd, OptionalActions::Now, &self.original_termios);
    }
}

#[cfg(target_os = "linux")]
impl Pn53xTransport for UartPort {
    fn send(&mut self, payload: &[u8], timeout_ms: i32) -> Result<(), Error> {
        self.abort_requested.store(false, Ordering::SeqCst);
        self.flush_input()?;
        self.write_all(payload, timeout_ms)
    }

    fn receive(&mut self, buffer: &mut [u8], timeout_ms: i32) -> Result<usize, Error> {
        self.read_frame_into(buffer, timeout_ms)
    }

    fn abort_command(&mut self) -> Result<(), Error> {
        self.abort_requested.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn wake_up(&mut self) -> Result<(), Error> {
        self.write_all(&WAKEUP_FRAME, 0)?;
        std::thread::sleep(Duration::from_millis(1));
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
pub struct UartPort;

#[cfg(not(target_os = "linux"))]
impl UartPort {
    pub fn open(_path: &str, _speed: u32) -> Result<Self, Error> {
        Err(Error::DriverOpenFailed(
            "pn532_uart is only available on Linux in this phase".into(),
        ))
    }
}

fn device_error(operation: &'static str, code: i32) -> Error {
    Error::DeviceOperationFailed { operation, code }
}

#[cfg(target_os = "linux")]
fn timeout_spec(timeout_ms: i32) -> Option<Timespec> {
    if timeout_ms < 0 {
        None
    } else {
        Some(Timespec {
            tv_sec: (timeout_ms / 1000) as i64,
            tv_nsec: ((timeout_ms % 1000) as i64) * 1_000_000,
        })
    }
}

#[cfg(target_os = "linux")]
fn expected_frame_len(frame: &[u8]) -> Result<Option<usize>, Error> {
    if frame.len() >= 6 && is_ack_frame(frame) {
        return Ok(Some(6));
    }
    if frame.len() < 5 {
        return Ok(None);
    }
    if !frame.starts_with(&[0x00, 0x00, 0xff]) {
        return Err(device_error("uart_receive", NFC_EIO));
    }
    if frame[3] == 0xff && frame[4] == 0xff {
        if frame.len() < 8 {
            return Ok(None);
        }
        if frame[5].wrapping_add(frame[6]).wrapping_add(frame[7]) != 0 {
            return Err(device_error("uart_receive", NFC_EIO));
        }
        let body_len = ((frame[5] as usize) << 8) | frame[6] as usize;
        return Ok(Some(8 + body_len + 2));
    }
    if frame[3].wrapping_add(frame[4]) != 0 {
        return Err(device_error("uart_receive", NFC_EIO));
    }
    Ok(Some(5 + frame[3] as usize + 2))
}

#[cfg(test)]
mod tests {
    use super::super::pn53x::build_response_frame;
    use super::*;
    use proximate_driver::Context;

    #[test]
    fn candidate_port_filter_uses_expected_linux_prefixes() {
        assert!(serial_name_prefixes().contains(&"ttyUSB"));
    }

    #[test]
    fn uart_frame_length_recognizes_ack_and_response() {
        #[cfg(target_os = "linux")]
        {
            assert_eq!(
                expected_frame_len(&[0x00, 0x00, 0xff, 0x00, 0xff, 0x00]).unwrap(),
                Some(6)
            );
            let frame = build_response_frame(0x02, &[0x32, 0x01, 0x06, 0x07]).unwrap();
            assert_eq!(expected_frame_len(&frame).unwrap(), Some(frame.len()));
        }
    }

    #[test]
    fn uart_driver_metadata_and_open_error_are_stable() {
        let driver = Pn532UartDriver::new();
        assert_eq!(driver.name(), DRIVER_NAME);
        assert_eq!(driver.scan_type(), ScanType::Intrusive);
        assert!(
            list_candidate_paths()
                .iter()
                .all(|path| path.starts_with("/dev/"))
        );

        let connstring = ConnectionString::new("pn532_uart:/definitely/missing").unwrap();
        let error = match driver.open(&Context::new(), &connstring) {
            Ok(_) => panic!("expected missing UART path to fail"),
            Err(error) => error,
        };
        assert!(matches!(error, Error::DriverOpenFailed(_)));
    }
}
