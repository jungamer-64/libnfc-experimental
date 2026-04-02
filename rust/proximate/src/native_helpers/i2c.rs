use std::fs;

#[cfg(target_os = "linux")]
use rustix::fd::OwnedFd;
#[cfg(target_os = "linux")]
use rustix::fs::{Mode, OFlags, open};
#[cfg(target_os = "linux")]
use rustix::io::{read, write};
#[cfg(target_os = "linux")]
use rustix::ioctl::{self, Ioctl, IoctlOutput, Opcode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum I2cOpenError {
    InvalidBus,
    InvalidAddress,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum I2cIoError {
    Io,
    InvalidArgument,
}

pub fn list_ports() -> Vec<String> {
    let mut ports = Vec::new();
    let Ok(entries) = fs::read_dir("/dev") else {
        return ports;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("i2c-") {
            ports.push(format!("/dev/{name}"));
        }
    }

    ports.sort();
    ports
}

#[cfg(target_os = "linux")]
struct I2cSetSlave {
    address: u32,
}

#[cfg(target_os = "linux")]
unsafe impl Ioctl for I2cSetSlave {
    type Output = ();
    const IS_MUTATING: bool = false;

    fn opcode(&self) -> Opcode {
        0x0703
    }

    fn as_ptr(&mut self) -> *mut std::ffi::c_void {
        self.address as usize as *mut std::ffi::c_void
    }

    unsafe fn output_from_ptr(
        _out: IoctlOutput,
        _extract_output: *mut std::ffi::c_void,
    ) -> rustix::io::Result<Self::Output> {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
pub struct I2cHandle {
    fd: OwnedFd,
}

#[cfg(target_os = "linux")]
impl I2cHandle {
    pub fn open(path: &str, address: u32) -> Result<Self, I2cOpenError> {
        let fd = open(
            path,
            OFlags::RDWR | OFlags::NONBLOCK | OFlags::NOCTTY,
            Mode::empty(),
        )
        .map_err(|_| I2cOpenError::InvalidBus)?;

        unsafe { ioctl::ioctl(&fd, I2cSetSlave { address }) }
            .map_err(|_| I2cOpenError::InvalidAddress)?;

        Ok(Self { fd })
    }

    pub fn read(&self, rx: &mut [u8]) -> Result<(), I2cIoError> {
        let len = read(&self.fd, &mut *rx).map_err(|_| I2cIoError::Io)?;
        if len < rx.len() {
            Err(I2cIoError::InvalidArgument)
        } else {
            Ok(())
        }
    }

    pub fn write(&self, tx: &[u8]) -> Result<(), I2cIoError> {
        let len = write(&self.fd, tx).map_err(|_| I2cIoError::Io)?;
        if len == tx.len() {
            Ok(())
        } else {
            Err(I2cIoError::Io)
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub struct I2cHandle;

#[cfg(not(target_os = "linux"))]
impl I2cHandle {
    pub fn open(_path: &str, _address: u32) -> Result<Self, I2cOpenError> {
        Err(I2cOpenError::InvalidBus)
    }

    pub fn read(&self, _rx: &mut [u8]) -> Result<(), I2cIoError> {
        Err(I2cIoError::Io)
    }

    pub fn write(&self, _tx: &[u8]) -> Result<(), I2cIoError> {
        Err(I2cIoError::Io)
    }
}
