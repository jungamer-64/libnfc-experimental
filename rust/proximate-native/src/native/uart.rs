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

use super::connstring::{build_path_speed_connstring, decode_path_speed_descriptor};
use super::pn53x::{
    PN53X_ACK_FRAME, Pn53xDevice, Pn53xProfile, Pn53xTransport, TransportSendError, is_ack_frame,
    probe_timeout,
};
#[cfg(all(test, unix))]
use crate::serial::serial_name_prefixes;
use crate::serial::{SerialPort, list_candidate_paths as platform_candidate_paths};
use proximate_driver::{
    CommandAbortHandle, ConnectionString, Context, DeviceHandle, Driver, Error, OperationTimeout,
    ScanType,
};
use std::time::Duration;

const DRIVER_NAME: &str = "pn532_uart";
const DEFAULT_SPEED: u32 = 115_200;
const WAKEUP_FRAME: [u8; 16] = [
    0x55, 0x55, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

pub(crate) struct Pn532UartDriver;

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

    fn scan(&self, _context: &Context) -> Result<proximate_driver::DriverScan, Error> {
        let mut devices = Vec::new();

        for path in list_candidate_paths()? {
            let Ok(connstring) = build_path_speed_connstring(DRIVER_NAME, &path, DEFAULT_SPEED)
            else {
                continue;
            };

            let Ok(port) = UartPort::open(&path, DEFAULT_SPEED) else {
                continue;
            };
            if Pn53xDevice::probe_with_profile(
                format!("PN532 UART ({path})"),
                connstring.clone(),
                Pn53xProfile::pn532(DRIVER_NAME),
                port,
                probe_timeout(),
            )
            .is_ok()
            {
                devices.push(self.describe_discovered(format!("PN532 UART ({path})"), connstring));
            }
        }

        Ok(proximate_driver::DriverScan::Complete(devices))
    }

    fn open(
        &self,
        _context: &Context,
        connstring: &ConnectionString,
    ) -> Result<Box<dyn DeviceHandle>, Error> {
        let descriptor = decode_path_speed_descriptor(connstring, DRIVER_NAME, DEFAULT_SPEED)?;
        let port = UartPort::open(&descriptor.path, descriptor.speed)?;
        let device = Pn53xDevice::probe_with_profile(
            format!("PN532 UART ({})", descriptor.path),
            connstring.clone(),
            Pn53xProfile::pn532(DRIVER_NAME),
            port,
            probe_timeout(),
        )?;
        Ok(Box::new(device))
    }
}

pub(crate) fn list_candidate_paths() -> Result<Vec<String>, Error> {
    platform_candidate_paths()
}

pub struct UartPort {
    serial: SerialPort,
    read_buffer: Vec<u8>,
}

impl UartPort {
    pub fn open(path: &str, speed: u32) -> Result<Self, Error> {
        Ok(Self {
            serial: SerialPort::open(path, speed)?,
            read_buffer: Vec::new(),
        })
    }

    pub(crate) fn command_abort_handle(&self) -> CommandAbortHandle {
        self.serial.command_abort_handle()
    }

    pub(crate) fn flush_input(&mut self) -> Result<(), Error> {
        self.serial.begin_command()?;
        self.serial.flush_input()?;
        self.read_buffer.clear();
        Ok(())
    }

    pub(crate) fn write_all(
        &mut self,
        payload: &[u8],
        timeout: OperationTimeout,
    ) -> Result<(), Error> {
        self.serial.write_all(payload, timeout)
    }

    pub(crate) fn read_exact(
        &mut self,
        buffer: &mut [u8],
        timeout: OperationTimeout,
    ) -> Result<(), Error> {
        let mut filled = 0usize;
        while filled < buffer.len() {
            if self.read_buffer.is_empty() {
                self.fill_read_buffer(timeout)?;
            }
            let available = (buffer.len() - filled).min(self.read_buffer.len());
            buffer[filled..filled + available].copy_from_slice(&self.read_buffer[..available]);
            self.read_buffer.drain(..available);
            filled += available;
        }
        Ok(())
    }

    pub(crate) fn read_frame_into(
        &mut self,
        buffer: &mut [u8],
        timeout: OperationTimeout,
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

            self.fill_read_buffer(timeout)?;
        }
    }

    pub(crate) fn abort_command(&self) -> Result<(), Error> {
        self.serial.abort_command()
    }

    fn fill_read_buffer(&mut self, timeout: OperationTimeout) -> Result<(), Error> {
        let mut chunk = [0u8; 512];
        let len = self.serial.read_some(&mut chunk, timeout)?;
        if len == 0 {
            return Err(Error::Io("uart_receive"));
        }
        self.read_buffer.extend_from_slice(&chunk[..len]);
        Ok(())
    }
}

impl Pn53xTransport for UartPort {
    fn send(
        &mut self,
        payload: &[u8],
        timeout: OperationTimeout,
    ) -> Result<(), TransportSendError> {
        timeout
            .configured_millis()
            .map_err(TransportSendError::ProtocolStable)?;
        self.flush_input()
            .map_err(TransportSendError::ProtocolStable)?;
        self.write_all(payload, timeout)
            .map_err(TransportSendError::OutcomeUnknown)
    }

    fn receive(&mut self, buffer: &mut [u8], timeout: OperationTimeout) -> Result<usize, Error> {
        match self.read_frame_into(buffer, timeout) {
            Err(operation @ Error::Aborted(_)) => {
                if let Err(recovery) = self.write_all(&PN53X_ACK_FRAME, OperationTimeout::INFINITE)
                {
                    return Err(Error::RecoveryFailed {
                        operation: Box::new(operation),
                        recovery: Box::new(recovery),
                    });
                }
                Err(operation)
            }
            result => result,
        }
    }

    fn abort_command(&mut self) -> Result<(), Error> {
        UartPort::abort_command(self)
    }

    fn command_abort_handle(&self) -> Option<CommandAbortHandle> {
        Some(UartPort::command_abort_handle(self))
    }

    fn wake_up(&mut self) -> Result<(), Error> {
        self.write_all(&WAKEUP_FRAME, OperationTimeout::INFINITE)?;
        std::thread::sleep(Duration::from_millis(1));
        Ok(())
    }
}

fn expected_frame_len(frame: &[u8]) -> Result<Option<usize>, Error> {
    if frame.len() >= 6 && is_ack_frame(frame) {
        return Ok(Some(6));
    }
    if frame.len() < 5 {
        return Ok(None);
    }
    if !frame.starts_with(&[0x00, 0x00, 0xff]) {
        return Err(Error::Io("uart_receive"));
    }
    if frame[3] == 0xff && frame[4] == 0xff {
        if frame.len() < 8 {
            return Ok(None);
        }
        if frame[5].wrapping_add(frame[6]).wrapping_add(frame[7]) != 0 {
            return Err(Error::Io("uart_receive"));
        }
        let body_len = ((frame[5] as usize) << 8) | frame[6] as usize;
        return Ok(Some(8 + body_len + 2));
    }
    if frame[3].wrapping_add(frame[4]) != 0 {
        return Err(Error::Io("uart_receive"));
    }
    Ok(Some(5 + frame[3] as usize + 2))
}

#[cfg(test)]
mod tests {
    use super::super::pn53x::build_response_frame;
    use super::*;
    use proximate_driver::Context;

    #[test]
    fn candidate_port_filter_uses_platform_names() {
        #[cfg(unix)]
        assert!(serial_name_prefixes().contains(&"ttyUSB"));
        #[cfg(windows)]
        assert!(list_candidate_paths().is_ok());
    }

    #[test]
    fn uart_frame_length_recognizes_ack_and_response() {
        assert_eq!(
            expected_frame_len(&[0x00, 0x00, 0xff, 0x00, 0xff, 0x00]).unwrap(),
            Some(6)
        );
        let frame = build_response_frame(0x02, &[0x32, 0x01, 0x06, 0x07]).unwrap();
        assert_eq!(expected_frame_len(&frame).unwrap(), Some(frame.len()));
    }

    #[test]
    fn uart_driver_metadata_and_open_error_are_stable() {
        let driver = Pn532UartDriver::new();
        assert_eq!(driver.name(), DRIVER_NAME);
        assert_eq!(driver.scan_type(), ScanType::Intrusive);
        #[cfg(unix)]
        assert!(
            list_candidate_paths()
                .unwrap()
                .iter()
                .all(|path| path.starts_with("/dev/"))
        );
        #[cfg(windows)]
        assert!(
            list_candidate_paths()
                .unwrap()
                .iter()
                .all(|path| path.starts_with(r"\\.\COM"))
        );

        let connstring = ConnectionString::new("pn532_uart:/definitely/missing").unwrap();
        let error = match driver.open(&Context::new(), &connstring) {
            Ok(_) => panic!("expected missing UART path to fail"),
            Err(error) => error,
        };
        assert!(matches!(error, Error::DriverOpenFailed(_)));
    }
}
