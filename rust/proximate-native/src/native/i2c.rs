/*-
 * Free/Libre Near Field Communication (NFC) library
 *
 * Libnfc historical contributors:
 * Copyright (C) 2009      Roel Verdult
 * Copyright (C) 2009-2013 Romuald Conty
 * Copyright (C) 2010-2012 Romain Tartière
 * Copyright (C) 2010-2013 Philippe Teuwen
 * Copyright (C) 2012-2013 Ludovic Rousseau
 * See AUTHORS file for a more comprehensive list of contributors.
 * Additional contributors of this file:
 * Copyright (C) 2013      Laurent Latil
 *
 * This program is free software: you can redistribute it and/or modify it
 * under the terms of the GNU Lesser General Public License as published by the
 * Free Software Foundation, either version 3 of the License, or (at your
 * option) any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
 * more details.
 *
 * You should have received a copy of the GNU Lesser General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>
 */

// Derived from libnfc's PN532 I2C driver. The bus protocol is implemented
// here in Rust.

use super::connstring::{build_path_connstring, decode_path_descriptor};
use super::pn53x::{Pn53xDevice, Pn53xProfile, Pn53xTransport, probe_timeout};
use crate::command_abort::AtomicCommandAbort;
use proximate_driver::{
    CommandAbort, CommandAbortHandle, ConnectionString, Context, DeviceHandle, Driver, Error,
    OperationTimeout, ScanType,
};
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use crate::i2c::{I2cHandle, I2cIoError, I2cOpenError};

const DRIVER_NAME: &str = "pn532_i2c";
const PN532_I2C_ADDR: u16 = 0x24;
const PN532_SEND_RETRIES: u8 = 3;
const PN532_BUS_FREE_TIME_MS: u64 = 5;
const NFC_EIO: i32 = -1;
const NFC_ETIMEOUT: i32 = -6;

pub(crate) struct Pn532I2cDriver;

impl Pn532I2cDriver {
    pub(crate) const fn new() -> Self {
        Self
    }
}

impl Driver for Pn532I2cDriver {
    fn name(&self) -> &str {
        DRIVER_NAME
    }

    fn scan_type(&self) -> ScanType {
        ScanType::Intrusive
    }

    fn scan(&self, _context: &Context) -> Result<Vec<proximate_driver::DiscoveredDevice>, Error> {
        let mut devices = Vec::new();
        for path in list_candidate_paths() {
            let Ok(connstring) = build_path_connstring(DRIVER_NAME, &path) else {
                continue;
            };

            #[cfg(target_os = "linux")]
            {
                let Ok(transport) = I2cTransport::open(&path) else {
                    continue;
                };
                if Pn53xDevice::probe_with_profile(
                    format!("PN532 I2C ({path})"),
                    connstring.clone(),
                    Pn53xProfile::pn532(DRIVER_NAME),
                    transport,
                    probe_timeout(),
                )
                .is_ok()
                {
                    devices
                        .push(self.describe_discovered(format!("PN532 I2C ({path})"), connstring));
                }
            }

            #[cfg(not(target_os = "linux"))]
            {
                devices.push(self.describe_discovered(format!("PN532 I2C ({path})"), connstring));
            }
        }
        Ok(devices)
    }

    fn open(
        &self,
        _context: &Context,
        connstring: &ConnectionString,
    ) -> Result<Box<dyn DeviceHandle>, Error> {
        let descriptor = decode_path_descriptor(connstring, DRIVER_NAME)?;

        #[cfg(target_os = "linux")]
        {
            let transport = I2cTransport::open(&descriptor.path)?;
            let device = Pn53xDevice::probe_with_profile(
                format!("PN532 I2C ({})", descriptor.path),
                connstring.clone(),
                Pn53xProfile::pn532(DRIVER_NAME),
                transport,
                probe_timeout(),
            )?;
            return Ok(Box::new(device));
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = descriptor;
            Err(Error::DriverOpenFailed(
                "pn532_i2c is only available on Linux in this phase".into(),
            ))
        }
    }
}

fn list_candidate_paths() -> Vec<String> {
    crate::i2c::list_ports()
}

#[cfg(target_os = "linux")]
pub struct I2cTransport {
    handle: I2cHandle,
    command_abort: std::sync::Arc<AtomicCommandAbort>,
    last_transaction_stop: Option<Instant>,
}

#[cfg(target_os = "linux")]
impl I2cTransport {
    pub fn open(path: &str) -> Result<Self, Error> {
        let handle = match I2cHandle::open(path, u32::from(PN532_I2C_ADDR)) {
            Ok(handle) => handle,
            Err(I2cOpenError::InvalidBus) => {
                return Err(Error::DriverOpenFailed(format!("failed to open {path}")));
            }
            Err(I2cOpenError::InvalidAddress) => {
                return Err(Error::DriverOpenFailed(format!(
                    "failed to bind PN532 I2C address on {path}"
                )));
            }
        };

        Ok(Self {
            handle,
            command_abort: AtomicCommandAbort::new(),
            last_transaction_stop: None,
        })
    }

    fn respect_bus_free_time(&mut self) {
        if let Some(last) = self.last_transaction_stop {
            let required = Duration::from_millis(PN532_BUS_FREE_TIME_MS);
            let elapsed = last.elapsed();
            if elapsed < required {
                std::thread::sleep(required - elapsed);
            }
        }
    }

    fn note_transaction_stop(&mut self) {
        self.last_transaction_stop = Some(Instant::now());
    }
}

#[cfg(target_os = "linux")]
impl Pn53xTransport for I2cTransport {
    fn send(&mut self, payload: &[u8], _timeout: OperationTimeout) -> Result<(), Error> {
        self.command_abort.begin_command();
        let mut last_error = None;

        for _ in 0..PN532_SEND_RETRIES {
            self.respect_bus_free_time();
            match self.handle.write(payload) {
                Ok(()) => {
                    self.note_transaction_stop();
                    return Ok(());
                }
                Err(I2cIoError::Io | I2cIoError::InvalidArgument) => {
                    last_error = Some(device_error("i2c_send", NFC_EIO));
                }
            }
            self.note_transaction_stop();
        }

        Err(last_error.unwrap_or_else(|| device_error("i2c_send", NFC_EIO)))
    }

    fn receive(&mut self, buffer: &mut [u8], timeout: OperationTimeout) -> Result<usize, Error> {
        timeout.configured_millis()?;
        let timeout = timeout
            .finite_millis()
            .map(|millis| Duration::from_millis(u64::from(millis)));
        let start = Instant::now();
        loop {
            if self.command_abort.take_requested() {
                return Err(Error::Aborted("i2c_receive"));
            }

            self.respect_bus_free_time();
            let mut scratch = vec![0u8; buffer.len() + 1];
            self.handle
                .read(scratch.as_mut_slice())
                .map_err(|_| device_error("i2c_receive", NFC_EIO))?;
            let len = scratch.len();
            self.note_transaction_stop();

            if len == 0 {
                return Err(device_error("i2c_receive", NFC_EIO));
            }

            if (scratch[0] & 1) != 0 {
                let payload_len = len.saturating_sub(1);
                if payload_len > buffer.len() {
                    return Err(Error::BufferTooSmall {
                        needed: payload_len,
                        available: buffer.len(),
                    });
                }
                buffer[..payload_len].copy_from_slice(&scratch[1..1 + payload_len]);
                return Ok(payload_len);
            }

            if timeout.is_some_and(|timeout| start.elapsed() > timeout) {
                return Err(device_error("i2c_receive", NFC_ETIMEOUT));
            }

            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn abort_command(&mut self) -> Result<(), Error> {
        self.command_abort.abort()
    }

    fn command_abort_handle(&self) -> Option<CommandAbortHandle> {
        Some(self.command_abort.clone())
    }
}

#[cfg(target_os = "linux")]
impl Drop for I2cTransport {
    fn drop(&mut self) {
        self.command_abort.revoke();
    }
}

#[cfg(not(target_os = "linux"))]
pub struct I2cTransport;

#[cfg(not(target_os = "linux"))]
impl I2cTransport {
    pub fn open(_path: &str) -> Result<Self, Error> {
        Err(Error::DriverOpenFailed(
            "pn532_i2c is only available on Linux in this phase".into(),
        ))
    }
}

fn device_error(operation: &'static str, code: i32) -> Error {
    Error::DeviceOperationFailed { operation, code }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proximate_driver::Context;

    #[test]
    fn candidate_port_filter_uses_expected_linux_prefix() {
        assert!(
            list_candidate_paths()
                .iter()
                .all(|path| path.starts_with("/dev/i2c-"))
        );
    }

    #[test]
    fn i2c_driver_metadata_and_open_error_are_stable() {
        let driver = Pn532I2cDriver::new();
        assert_eq!(driver.name(), DRIVER_NAME);
        assert_eq!(driver.scan_type(), ScanType::Intrusive);

        let connstring = ConnectionString::new("pn532_i2c:/definitely/missing").unwrap();
        let error = match driver.open(&Context::new(), &connstring) {
            Ok(_) => panic!("expected missing I2C path to fail"),
            Err(error) => error,
        };
        assert!(matches!(error, Error::DriverOpenFailed(_)));
    }
}
