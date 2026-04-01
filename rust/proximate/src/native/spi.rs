use super::connstring::{build_path_speed_connstring, decode_path_speed_descriptor};
use super::pn53x::{
    Pn53xDevice, Pn53xProfile, Pn53xTransport, command_from_host_frame, is_ack_frame,
};
use crate::rust_api::{ConnectionString, Context, Driver, Error, OpenedDevice, ScanType};
use rustix::fd::OwnedFd;
use rustix::fs::{Mode, OFlags, open};
use rustix::ioctl::{self, Direction, Ioctl, IoctlOutput, Opcode, Setter, opcode};
use std::ffi::c_void;
use std::fs;
use std::mem::size_of_val;
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

const SPI_IOC_WR_MODE: Opcode = opcode::write::<u8>(b'k', 1);
const SPI_IOC_WR_MAX_SPEED_HZ: Opcode = opcode::write::<u32>(b'k', 4);

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
    ) -> Result<Box<dyn OpenedDevice>, Error> {
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
    let mut ports = Vec::new();
    let Ok(entries) = fs::read_dir("/dev") else {
        return ports;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("spidev")
            && name
                .bytes()
                .last()
                .is_some_and(|byte| byte.is_ascii_digit())
        {
            ports.push(format!("/dev/{name}"));
        }
    }

    ports.sort();
    ports
}

#[repr(C)]
struct SpiTransfer {
    tx_buf: u64,
    rx_buf: u64,
    len: u32,
    speed_hz: u32,
    delay_usecs: u16,
    bits_per_word: u8,
    cs_change: u8,
    tx_nbits: u8,
    rx_nbits: u8,
    word_delay_usecs: u8,
    pad: u8,
}

struct SpiMessage<'a> {
    transfers: &'a mut [SpiTransfer],
}

unsafe impl Ioctl for SpiMessage<'_> {
    type Output = IoctlOutput;
    const IS_MUTATING: bool = true;

    fn opcode(&self) -> Opcode {
        opcode::from_components(Direction::ReadWrite, b'k', 0, size_of_val(self.transfers))
    }

    fn as_ptr(&mut self) -> *mut c_void {
        self.transfers.as_mut_ptr().cast::<c_void>()
    }

    unsafe fn output_from_ptr(
        out: IoctlOutput,
        _extract_output: *mut c_void,
    ) -> rustix::io::Result<Self::Output> {
        Ok(out)
    }
}

pub struct SpiTransport {
    fd: OwnedFd,
    abort_requested: Arc<AtomicBool>,
}

impl SpiTransport {
    pub fn open(path: &str, speed: u32) -> Result<Self, Error> {
        let fd = open(
            path,
            OFlags::RDWR | OFlags::NONBLOCK | OFlags::NOCTTY,
            Mode::empty(),
        )
        .map_err(|error| {
            Error::DriverOpenFailed(format!("failed to open {path}: {}", error.raw_os_error()))
        })?;

        unsafe { ioctl::ioctl(&fd, Setter::<SPI_IOC_WR_MODE, u8>::new(SPI_MODE_0)) }
            .map_err(|_| Error::DriverOpenFailed(format!("failed to set SPI mode on {path}")))?;
        unsafe { ioctl::ioctl(&fd, Setter::<SPI_IOC_WR_MAX_SPEED_HZ, u32>::new(speed)) }
            .map_err(|_| Error::DriverOpenFailed(format!("failed to set SPI speed on {path}")))?;

        Ok(Self {
            fd,
            abort_requested: Arc::new(AtomicBool::new(false)),
        })
    }

    fn transfer(
        &self,
        tx: Option<&[u8]>,
        rx: Option<&mut [u8]>,
        lsb_first: bool,
    ) -> Result<(), Error> {
        let mut tx_storage = tx.unwrap_or_default().to_vec();
        if lsb_first {
            for byte in &mut tx_storage {
                *byte = bit_reverse(*byte);
            }
        }

        let mut transfers = Vec::with_capacity(2);
        if !tx_storage.is_empty() {
            transfers.push(SpiTransfer {
                tx_buf: tx_storage.as_ptr() as u64,
                rx_buf: 0,
                len: tx_storage.len() as u32,
                speed_hz: 0,
                delay_usecs: 0,
                bits_per_word: 0,
                cs_change: 0,
                tx_nbits: 0,
                rx_nbits: 0,
                word_delay_usecs: 0,
                pad: 0,
            });
        }
        if let Some(rx_buf) = rx.as_ref() {
            transfers.push(SpiTransfer {
                tx_buf: 0,
                rx_buf: rx_buf.as_ptr() as usize as u64,
                len: rx_buf.len() as u32,
                speed_hz: 0,
                delay_usecs: 0,
                bits_per_word: 0,
                cs_change: 0,
                tx_nbits: 0,
                rx_nbits: 0,
                word_delay_usecs: 0,
                pad: 0,
            });
        }

        if transfers.is_empty() {
            return Ok(());
        }

        let result = unsafe {
            ioctl::ioctl(
                &self.fd,
                SpiMessage {
                    transfers: &mut transfers,
                },
            )
        }
        .map_err(|_| device_error("spi_transfer", NFC_EIO))?;
        let expected = tx_storage.len() + rx.as_ref().map_or(0, |buf| buf.len());
        if result < 0 || result as usize != expected {
            return Err(device_error("spi_transfer", NFC_EIO));
        }

        if lsb_first && let Some(rx_buf) = rx {
            for byte in rx_buf {
                *byte = bit_reverse(*byte);
            }
        }

        Ok(())
    }

    fn read_status(&self) -> Result<u8, Error> {
        let mut status = [0u8; 1];
        self.transfer(Some(&[STATREAD]), Some(&mut status), true)?;
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
        self.transfer(Some(&tx), None, true)
    }

    fn receive(&mut self, buffer: &mut [u8], timeout_ms: i32) -> Result<usize, Error> {
        self.wait_ready(timeout_ms)?;

        let read_len = buffer.len().saturating_add(4).min(buffer.len().max(16));
        let mut scratch = vec![0u8; read_len];
        self.transfer(Some(&[DATAREAD]), Some(&mut scratch), true)?;

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
        self.transfer(None, Some(&mut byte), true)?;
        thread::sleep(Duration::from_millis(1));
        Ok(())
    }
}

fn bit_reverse(byte: u8) -> u8 {
    let mut value = byte;
    value = ((value & 0xaa) >> 1) | ((value & 0x55) << 1);
    value = ((value & 0xcc) >> 2) | ((value & 0x33) << 2);
    ((value & 0xf0) >> 4) | ((value & 0x0f) << 4)
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
