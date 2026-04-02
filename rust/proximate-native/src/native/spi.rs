use super::connstring::{build_path_speed_connstring, decode_path_speed_descriptor};
use super::pn53x::{
    Pn53xDevice, Pn53xProfile, Pn53xTransport, command_from_host_frame, is_ack_frame,
};
use crate::spi::{SpiHandle, SpiOpenError};
use proximate_driver::{ConnectionString, Context, DeviceHandle, Driver, Error, ScanType};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

const DRIVER_NAME: &str = "pn532_spi";
const DEFAULT_SPEED: u32 = 1_000_000;
const SPI_MODE_0: u8 = 0;
const PROBE_TIMEOUT_MS: i32 = 250;
const NFC_EIO: i32 = -1;
const NFC_ETIMEOUT: i32 = -6;
const NFC_EOPABORTED: i32 = -7;
const DATAREAD: u8 = 0x03;
const DATAWRITE: u8 = 0x01;
const STATREAD: u8 = 0x02;
const ACK_FRAME: [u8; 6] = [0x00, 0x00, 0xff, 0x00, 0xff, 0x00];
const STATUS_POLL_INTERVAL_MS: u64 = 10;

pub(crate) struct Pn532SpiDriver;

impl Pn532SpiDriver {
    pub(crate) const fn new() -> Self {
        Self
    }
}

impl Driver for Pn532SpiDriver {
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
            let Ok(transport) = SpiTransport::open(&path, DEFAULT_SPEED) else {
                continue;
            };
            if Pn53xDevice::probe_with_profile(
                format!("PN532 SPI ({path})"),
                connstring.clone(),
                Pn53xProfile::pn532(DRIVER_NAME),
                transport,
                PROBE_TIMEOUT_MS,
            )
            .is_ok()
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
        let transport = SpiTransport::open(&descriptor.path, descriptor.speed)?;
        let device = Pn53xDevice::probe_with_profile(
            format!("PN532 SPI ({})", descriptor.path),
            connstring.clone(),
            Pn53xProfile::pn532(DRIVER_NAME),
            transport,
            PROBE_TIMEOUT_MS,
        )?;
        Ok(Box::new(device))
    }
}

fn list_candidate_paths() -> Vec<String> {
    crate::spi::list_ports()
}

pub struct SpiTransport {
    handle: SpiHandle,
    abort_requested: Arc<AtomicBool>,
}

impl SpiTransport {
    pub fn open(path: &str, speed: u32) -> Result<Self, Error> {
        let mut handle = match SpiHandle::open(path) {
            Ok(handle) => handle,
            Err(SpiOpenError::InvalidPort) => {
                return Err(Error::DriverOpenFailed(format!("failed to open {path}")));
            }
        };
        handle
            .set_mode(u32::from(SPI_MODE_0))
            .map_err(|_| Error::DriverOpenFailed(format!("failed to set SPI mode on {path}")))?;
        handle
            .set_speed(speed)
            .map_err(|_| Error::DriverOpenFailed(format!("failed to set SPI speed on {path}")))?;

        Ok(Self {
            handle,
            abort_requested: Arc::new(AtomicBool::new(false)),
        })
    }

    fn read_status(&self) -> Result<u8, Error> {
        let mut status = [0u8; 1];
        self.handle
            .send_receive(&[STATREAD], &mut status, true)
            .map_err(|_| device_error("spi_transfer", NFC_EIO))?;
        Ok(status[0])
    }

    fn wait_ready(&mut self, timeout_ms: i32) -> Result<(), Error> {
        let start = Instant::now();
        loop {
            if self.abort_requested.swap(false, Ordering::SeqCst) {
                return Err(device_error("spi_abort", NFC_EOPABORTED));
            }

            if self.read_status()? == 0x01 {
                return Ok(());
            }

            if timeout_ms >= 0 && start.elapsed() > Duration::from_millis(timeout_ms as u64) {
                return Err(device_error("spi_wait_ready", NFC_ETIMEOUT));
            }

            thread::sleep(Duration::from_millis(STATUS_POLL_INTERVAL_MS));
        }
    }
}

impl Pn53xTransport for SpiTransport {
    fn send(&mut self, payload: &[u8], _timeout_ms: i32) -> Result<(), Error> {
        self.abort_requested.store(false, Ordering::SeqCst);
        let _ = command_from_host_frame(payload);
        let mut tx = Vec::with_capacity(payload.len() + 1);
        tx.push(DATAWRITE);
        tx.extend_from_slice(payload);
        self.handle
            .send(&tx, true)
            .map_err(|_| device_error("spi_transfer", NFC_EIO))
    }

    fn receive(&mut self, buffer: &mut [u8], timeout_ms: i32) -> Result<usize, Error> {
        self.wait_ready(timeout_ms)?;

        let read_len = buffer.len().saturating_add(4).min(buffer.len().max(16));
        let mut scratch = vec![0u8; read_len];
        self.handle
            .send_receive(&[DATAREAD], &mut scratch, true)
            .map_err(|_| device_error("spi_transfer", NFC_EIO))?;

        if let Some(position) = find_subslice(&scratch, &ACK_FRAME) {
            buffer[..ACK_FRAME.len()]
                .copy_from_slice(&scratch[position..position + ACK_FRAME.len()]);
            return Ok(ACK_FRAME.len());
        }

        let Some(start) = find_subslice(&scratch, &[0x00, 0x00, 0xff]) else {
            return Err(device_error("spi_receive", NFC_EIO));
        };
        let frame = &scratch[start..];
        let Some(frame_len) = expected_frame_len(frame)? else {
            return Err(device_error("spi_receive", NFC_EIO));
        };
        if frame_len > frame.len() || frame_len > buffer.len() {
            return Err(device_error("spi_receive", NFC_EIO));
        }

        buffer[..frame_len].copy_from_slice(&frame[..frame_len]);
        Ok(frame_len)
    }

    fn abort_command(&mut self) -> Result<(), Error> {
        self.abort_requested.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn wake_up(&mut self) -> Result<(), Error> {
        let mut byte = [0u8; 1];
        self.handle
            .receive(&mut byte, true)
            .map_err(|_| device_error("spi_transfer", NFC_EIO))?;
        thread::sleep(Duration::from_millis(1));
        Ok(())
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn expected_frame_len(frame: &[u8]) -> Result<Option<usize>, Error> {
    if frame.len() >= ACK_FRAME.len() && is_ack_frame(frame) {
        return Ok(Some(ACK_FRAME.len()));
    }
    if frame.len() < 5 {
        return Ok(None);
    }
    if !frame.starts_with(&[0x00, 0x00, 0xff]) {
        return Err(device_error("spi_receive", NFC_EIO));
    }
    if frame[3] == 0xff && frame[4] == 0xff {
        if frame.len() < 8 {
            return Ok(None);
        }
        if frame[5].wrapping_add(frame[6]).wrapping_add(frame[7]) != 0 {
            return Err(device_error("spi_receive", NFC_EIO));
        }
        let body_len = ((frame[5] as usize) << 8) | frame[6] as usize;
        return Ok(Some(8 + body_len + 2));
    }
    if frame[3].wrapping_add(frame[4]) != 0 {
        return Err(device_error("spi_receive", NFC_EIO));
    }
    Ok(Some(5 + frame[3] as usize + 2))
}

fn device_error(operation: &'static str, code: i32) -> Error {
    Error::DeviceOperationFailed { operation, code }
}

#[cfg(test)]
mod tests {
    use super::super::pn53x::build_response_frame;
    use super::*;
    use proximate_driver::Context;

    #[test]
    fn find_subslice_locates_ack_sequence_inside_status_prefix() {
        let frame = [0xaa, 0xbb, 0x00, 0x00, 0xff, 0x00, 0xff, 0x00, 0xcc];
        assert_eq!(find_subslice(&frame, &ACK_FRAME), Some(2));
    }

    #[test]
    fn expected_frame_len_recognizes_ack_and_response() {
        assert_eq!(
            expected_frame_len(&ACK_FRAME).unwrap(),
            Some(ACK_FRAME.len())
        );

        let frame = build_response_frame(0x02, &[0x32, 0x01, 0x06, 0x07]).unwrap();
        assert_eq!(expected_frame_len(&frame).unwrap(), Some(frame.len()));
    }

    #[test]
    fn spi_driver_metadata_and_open_error_are_stable() {
        let driver = Pn532SpiDriver::new();
        assert_eq!(driver.name(), DRIVER_NAME);
        assert_eq!(driver.scan_type(), ScanType::Intrusive);
        assert!(
            list_candidate_paths()
                .iter()
                .all(|path| path.starts_with("/dev/"))
        );

        let connstring = ConnectionString::new("pn532_spi:/definitely/missing:1000000").unwrap();
        let error = match driver.open(&Context::new(), &connstring) {
            Ok(_) => panic!("expected missing SPI path to fail"),
            Err(error) => error,
        };
        assert!(matches!(error, Error::DriverOpenFailed(_)));
    }
}
