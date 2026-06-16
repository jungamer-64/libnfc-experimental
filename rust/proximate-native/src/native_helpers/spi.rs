use std::ffi::c_void;
use std::fs;

use rustix::fd::OwnedFd;
use rustix::fs::{Mode, OFlags, open};
use rustix::ioctl::{self, Direction, Ioctl, IoctlOutput, Opcode, Setter, opcode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpiOpenError {
    InvalidPort,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpiIoError {
    Io,
}

const SPI_IOC_WR_MODE: Opcode = opcode::write::<u8>(b'k', 1);
const SPI_IOC_WR_MAX_SPEED_HZ: Opcode = opcode::write::<u32>(b'k', 4);

pub fn list_ports() -> Vec<String> {
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
        opcode::from_components(
            Direction::ReadWrite,
            b'k',
            0,
            std::mem::size_of_val(self.transfers),
        )
    }

    fn as_ptr(&mut self) -> *mut c_void {
        self.transfers.as_mut_ptr().cast()
    }

    unsafe fn output_from_ptr(
        out: IoctlOutput,
        _extract_output: *mut c_void,
    ) -> rustix::io::Result<Self::Output> {
        Ok(out)
    }
}

pub struct SpiHandle {
    fd: OwnedFd,
    speed: u32,
    mode: u32,
}

impl SpiHandle {
    pub fn open(path: &str) -> Result<Self, SpiOpenError> {
        let fd = open(
            path,
            OFlags::RDWR | OFlags::NONBLOCK | OFlags::NOCTTY,
            Mode::empty(),
        )
        .map_err(|_| SpiOpenError::InvalidPort)?;

        Ok(Self {
            fd,
            speed: 0,
            mode: 0,
        })
    }

    pub fn set_speed(&mut self, speed: u32) -> Result<(), SpiIoError> {
        unsafe { ioctl::ioctl(&self.fd, Setter::<SPI_IOC_WR_MAX_SPEED_HZ, u32>::new(speed)) }
            .map_err(|_| SpiIoError::Io)?;
        self.speed = speed;
        Ok(())
    }

    pub fn get_speed(&self) -> u32 {
        self.speed
    }

    pub fn set_mode(&mut self, mode: u32) -> Result<(), SpiIoError> {
        unsafe { ioctl::ioctl(&self.fd, Setter::<SPI_IOC_WR_MODE, u8>::new(mode as u8)) }
            .map_err(|_| SpiIoError::Io)?;
        self.mode = mode;
        Ok(())
    }

    pub fn send(&self, tx: &[u8], lsb_first: bool) -> Result<(), SpiIoError> {
        self.send_receive(tx, &mut [], lsb_first)
    }

    pub fn receive(&self, rx: &mut [u8], lsb_first: bool) -> Result<(), SpiIoError> {
        self.send_receive(&[], rx, lsb_first)
    }

    pub fn send_receive(
        &self,
        tx: &[u8],
        rx: &mut [u8],
        lsb_first: bool,
    ) -> Result<(), SpiIoError> {
        let mut tx_storage = tx.to_vec();
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
        if !rx.is_empty() {
            transfers.push(SpiTransfer {
                tx_buf: 0,
                rx_buf: rx.as_mut_ptr() as usize as u64,
                len: rx.len() as u32,
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

        let transferred = unsafe {
            ioctl::ioctl(
                &self.fd,
                SpiMessage {
                    transfers: &mut transfers,
                },
            )
        }
        .map_err(|_| SpiIoError::Io)?;
        if transferred as usize != tx.len() + rx.len() {
            return Err(SpiIoError::Io);
        }

        if lsb_first {
            for byte in rx {
                *byte = bit_reverse(*byte);
            }
        }

        Ok(())
    }
}

fn bit_reverse(byte: u8) -> u8 {
    let mut value = byte;
    value = ((value & 0xaa) >> 1) | ((value & 0x55) << 1);
    value = ((value & 0xcc) >> 2) | ((value & 0x33) << 2);
    ((value & 0xf0) >> 4) | ((value & 0x0f) << 4)
}

#[cfg(test)]
mod tests {
    use super::bit_reverse;

    #[test]
    fn bit_reverse_matches_expected_pattern() {
        assert_eq!(bit_reverse(0b0001_0110), 0b0110_1000);
    }
}
