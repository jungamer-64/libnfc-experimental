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

// Derived from libnfc's ARYGON driver. The serial reader protocol is
// implemented here in Rust.

use super::connstring::{build_path_speed_connstring, decode_path_speed_descriptor};
use super::pn53x::{
    PN53X_ACK_FRAME, Pn53xDevice, Pn53xProfile, Pn53xTransport, TransportSendError, is_ack_frame,
    probe_timeout,
};
use super::uart::{UartPort, list_candidate_paths};
use proximate_driver::{
    CommandAbortHandle, ConnectionString, Context, DeviceHandle, Driver, Error, OperationTimeout,
    ScanType,
};
use std::borrow::Cow;
use std::collections::VecDeque;

const DRIVER_NAME: &str = "arygon";
const DEFAULT_SPEED: u32 = 9_600;
const FIRMWARE_BUFFER_LEN: usize = 16;
const RESET_BUFFER_LEN: usize = 10;

const NFC_EIO: i32 = -1;

const PROTOCOL_ARYGON_ASCII: u8 = b'0';
const PROTOCOL_TAMA: u8 = b'2';

const ERROR_NONE: &[u8] = b"FF000000\r\n";
const ERROR_UNKNOWN_MODE_PREFIX: &[u8] = b"FF0600";
const ABORT_FRAME: [u8; 17] = [
    0x32, 0x00, 0x00, 0xff, 0x09, 0xf7, 0xd4, 0x00, 0x00, 0x6c, 0x69, 0x62, 0x6e, 0x66, 0x63, 0xbe,
    0x00,
];

fn control_timeout() -> OperationTimeout {
    OperationTimeout::try_milliseconds(1_000).expect("ARYGON control timeout is representable")
}
const RESET_TAMA_COMMAND: &[u8] = &[PROTOCOL_ARYGON_ASCII, b'a', b'r'];
const FIRMWARE_COMMAND: &[u8] = &[PROTOCOL_ARYGON_ASCII, b'a', b'v'];

pub(crate) struct ArygonDriver;

impl ArygonDriver {
    pub(crate) const fn new() -> Self {
        Self
    }
}

impl Driver for ArygonDriver {
    fn name(&self) -> &str {
        DRIVER_NAME
    }

    fn scan_type(&self) -> ScanType {
        ScanType::Intrusive
    }

    fn scan(&self, _context: &Context) -> Result<Vec<proximate_driver::DiscoveredDevice>, Error> {
        let mut devices = Vec::new();

        for path in list_candidate_paths() {
            let Ok(connstring) = build_path_speed_connstring(DRIVER_NAME, &path, DEFAULT_SPEED)
            else {
                continue;
            };

            let Ok(mut port) = UartPort::open(&path, DEFAULT_SPEED) else {
                continue;
            };
            if reset_tama(&mut port).is_ok() {
                devices.push(self.describe_discovered(format!("{DRIVER_NAME}:{path}"), connstring));
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
        let mut port = UartPort::open(&descriptor.path, descriptor.speed)?;
        reset_tama(&mut port)?;
        let firmware = query_firmware(&mut port)?;
        let display_name = if firmware.is_empty() {
            format!("{DRIVER_NAME}:{}", descriptor.path)
        } else {
            format!("{DRIVER_NAME}:{} {}", descriptor.path, firmware)
        };

        let transport = ArygonTransport::new(port);
        let device = Pn53xDevice::probe_with_profile(
            display_name,
            connstring.clone(),
            Pn53xProfile::arygon(),
            transport,
            probe_timeout(),
        )?;
        Ok(Box::new(device))
    }
}

trait ArygonIo: Send {
    fn flush_input(&mut self) -> Result<(), Error>;
    fn write_all(&mut self, payload: &[u8], timeout: OperationTimeout) -> Result<(), Error>;
    fn read_exact(&mut self, buffer: &mut [u8], timeout: OperationTimeout) -> Result<(), Error>;
    fn read_frame_into(
        &mut self,
        buffer: &mut [u8],
        timeout: OperationTimeout,
    ) -> Result<usize, Error>;
    fn abort_command(&mut self) -> Result<(), Error>;
    fn command_abort_handle(&self) -> Option<CommandAbortHandle>;
}

impl ArygonIo for UartPort {
    fn flush_input(&mut self) -> Result<(), Error> {
        UartPort::flush_input(self)
    }

    fn write_all(&mut self, payload: &[u8], timeout: OperationTimeout) -> Result<(), Error> {
        UartPort::write_all(self, payload, timeout)
    }

    fn read_exact(&mut self, buffer: &mut [u8], timeout: OperationTimeout) -> Result<(), Error> {
        UartPort::read_exact(self, buffer, timeout)
    }

    fn read_frame_into(
        &mut self,
        buffer: &mut [u8],
        timeout: OperationTimeout,
    ) -> Result<usize, Error> {
        UartPort::read_frame_into(self, buffer, timeout)
    }

    fn abort_command(&mut self) -> Result<(), Error> {
        UartPort::abort_command(self)
    }

    fn command_abort_handle(&self) -> Option<CommandAbortHandle> {
        Some(UartPort::command_abort_handle(self))
    }
}

struct ArygonTransport<IO = UartPort> {
    port: IO,
    pending: VecDeque<Vec<u8>>,
}

impl<IO> ArygonTransport<IO> {
    fn new(port: IO) -> Self {
        Self {
            port,
            pending: VecDeque::new(),
        }
    }
}

impl<IO: ArygonIo> Pn53xTransport for ArygonTransport<IO> {
    fn send(
        &mut self,
        payload: &[u8],
        timeout: OperationTimeout,
    ) -> Result<(), TransportSendError> {
        timeout
            .configured_millis()
            .map_err(TransportSendError::ProtocolStable)?;
        self.port
            .flush_input()
            .map_err(TransportSendError::ProtocolStable)?;

        let mut prefixed = Vec::with_capacity(payload.len() + 1);
        prefixed.push(PROTOCOL_TAMA);
        prefixed.extend_from_slice(payload);
        self.port
            .write_all(&prefixed, timeout)
            .map_err(TransportSendError::OutcomeUnknown)?;

        let mut ack = [0u8; PN53X_ACK_FRAME.len()];
        self.port
            .read_exact(&mut ack, timeout)
            .map_err(TransportSendError::OutcomeUnknown)?;
        if is_ack_frame(&ack) {
            self.pending.push_back(ack.to_vec());
            return Ok(());
        }

        if ack.starts_with(ERROR_UNKNOWN_MODE_PREFIX) {
            let mut rest = [0u8; 4];
            let _ = self.port.read_exact(&mut rest, timeout);
        }

        Err(TransportSendError::OutcomeUnknown(device_error(
            "arygon_send",
            NFC_EIO,
        )))
    }

    fn receive(&mut self, buffer: &mut [u8], timeout: OperationTimeout) -> Result<usize, Error> {
        if let Some(frame) = self.pending.pop_front() {
            if frame.len() > buffer.len() {
                return Err(Error::BufferTooSmall {
                    needed: frame.len(),
                    available: buffer.len(),
                });
            }
            buffer[..frame.len()].copy_from_slice(&frame);
            return Ok(frame.len());
        }
        match self.port.read_frame_into(buffer, timeout) {
            Err(operation @ Error::Aborted(_)) => {
                if let Err(recovery) = self
                    .port
                    .write_all(&ABORT_FRAME, OperationTimeout::INFINITE)
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
        self.pending.clear();
        self.port.abort_command()
    }

    fn command_abort_handle(&self) -> Option<CommandAbortHandle> {
        self.port.command_abort_handle()
    }
}

fn device_error(operation: &'static str, code: i32) -> Error {
    Error::DeviceOperationFailed { operation, code }
}

fn query_ascii_command(
    port: &mut UartPort,
    command: &[u8],
    response_len: usize,
    timeout: OperationTimeout,
) -> Result<Vec<u8>, Error> {
    port.flush_input()?;
    port.write_all(command, timeout)?;
    let mut response = vec![0u8; response_len];
    port.read_exact(&mut response, timeout)?;
    Ok(response)
}

fn parse_firmware(response: &[u8]) -> Result<String, Error> {
    if !response.starts_with(&ERROR_NONE[..6]) {
        return Err(device_error("arygon_firmware", NFC_EIO));
    }
    let size_hex = std::str::from_utf8(&response[6..8])
        .map_err(|_| device_error("arygon_firmware", NFC_EIO))?;
    let size = usize::from_str_radix(size_hex, 16)
        .map_err(|_| device_error("arygon_firmware", NFC_EIO))?;
    if response.len() < 8 + size {
        return Err(device_error("arygon_firmware", NFC_EIO));
    }
    let firmware = match String::from_utf8(response[8..8 + size].to_vec()) {
        Ok(text) => Cow::Owned(text),
        Err(_) => String::from_utf8_lossy(&response[8..8 + size]),
    };
    Ok(firmware.trim_end_matches('\0').to_string())
}

fn query_firmware(port: &mut UartPort) -> Result<String, Error> {
    let response = query_ascii_command(
        port,
        FIRMWARE_COMMAND,
        FIRMWARE_BUFFER_LEN,
        control_timeout(),
    )?;
    parse_firmware(&response)
}

fn reset_tama(port: &mut UartPort) -> Result<(), Error> {
    let response = query_ascii_command(
        port,
        RESET_TAMA_COMMAND,
        RESET_BUFFER_LEN,
        control_timeout(),
    )?;
    if response == ERROR_NONE {
        Ok(())
    } else {
        Err(device_error("arygon_reset_tama", NFC_EIO))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proximate_driver::Context;

    #[derive(Default)]
    struct FakeArygonIo {
        reads: VecDeque<Vec<u8>>,
        writes: Vec<Vec<u8>>,
    }

    impl ArygonIo for FakeArygonIo {
        fn flush_input(&mut self) -> Result<(), Error> {
            Ok(())
        }

        fn write_all(&mut self, payload: &[u8], _timeout: OperationTimeout) -> Result<(), Error> {
            self.writes.push(payload.to_vec());
            Ok(())
        }

        fn read_exact(
            &mut self,
            buffer: &mut [u8],
            _timeout: OperationTimeout,
        ) -> Result<(), Error> {
            let payload = self
                .reads
                .pop_front()
                .ok_or(Error::Io("fake_arygon_read"))?;
            if payload.len() != buffer.len() {
                return Err(Error::InvalidEncoding("fake ARYGON read length"));
            }
            buffer.copy_from_slice(&payload);
            Ok(())
        }

        fn read_frame_into(
            &mut self,
            buffer: &mut [u8],
            _timeout: OperationTimeout,
        ) -> Result<usize, Error> {
            let payload = self
                .reads
                .pop_front()
                .ok_or(Error::Io("fake_arygon_frame"))?;
            if payload.len() > buffer.len() {
                return Err(Error::BufferTooSmall {
                    needed: payload.len(),
                    available: buffer.len(),
                });
            }
            buffer[..payload.len()].copy_from_slice(&payload);
            Ok(payload.len())
        }

        fn abort_command(&mut self) -> Result<(), Error> {
            Ok(())
        }

        fn command_abort_handle(&self) -> Option<CommandAbortHandle> {
            None
        }
    }

    fn test_timeout() -> OperationTimeout {
        OperationTimeout::try_milliseconds(25).expect("test timeout is representable")
    }

    #[test]
    fn reset_response_matches_existing_c_behavior() {
        assert_eq!(ERROR_NONE, b"FF000000\r\n");
    }

    #[test]
    fn parse_firmware_matches_existing_ascii_protocol() {
        let response = b"FF000006ARYGON";
        assert_eq!(parse_firmware(response).unwrap(), "ARYGON");
    }

    #[test]
    fn driver_metadata_and_missing_port_error_are_stable() {
        let driver = ArygonDriver::new();
        assert_eq!(driver.name(), DRIVER_NAME);
        assert_eq!(driver.scan_type(), ScanType::Intrusive);

        let connstring = ConnectionString::new("arygon:/definitely/missing").unwrap();
        let error = match driver.open(&Context::new(), &connstring) {
            Ok(_) => panic!("expected missing serial path to fail"),
            Err(error) => error,
        };
        assert!(matches!(error, Error::DriverOpenFailed(_)));
    }

    #[test]
    fn transport_constants_match_expected_sizes() {
        assert_eq!(RESET_BUFFER_LEN, ERROR_NONE.len());
    }

    #[test]
    fn tama_transport_replays_consumed_ack_before_the_response() {
        let response =
            super::super::pn53x::build_response_frame(0x02, &[0x32, 0x01, 0x06, 0x07]).unwrap();
        let io = FakeArygonIo {
            reads: [PN53X_ACK_FRAME.to_vec(), response.clone()].into(),
            ..FakeArygonIo::default()
        };
        let mut transport = ArygonTransport::new(io);
        let frame = super::super::pn53x::build_frame(&[0x02]).unwrap();

        transport.send(&frame, test_timeout()).unwrap();
        assert_eq!(
            transport.port.writes,
            [[vec![PROTOCOL_TAMA], frame.clone()].concat()]
        );

        let mut buffer = [0u8; 32];
        assert_eq!(transport.receive(&mut buffer, test_timeout()).unwrap(), 6);
        assert_eq!(&buffer[..6], &PN53X_ACK_FRAME);
        let response_len = transport.receive(&mut buffer, test_timeout()).unwrap();
        assert_eq!(&buffer[..response_len], response.as_slice());
    }

    #[test]
    fn tama_unknown_mode_response_is_drained_and_marks_the_send_uncertain() {
        let io = FakeArygonIo {
            reads: [ERROR_UNKNOWN_MODE_PREFIX.to_vec(), vec![b'0'; 4]].into(),
            ..FakeArygonIo::default()
        };
        let mut transport = ArygonTransport::new(io);
        let frame = super::super::pn53x::build_frame(&[0x02]).unwrap();

        assert_eq!(
            transport.send(&frame, test_timeout()),
            Err(TransportSendError::OutcomeUnknown(device_error(
                "arygon_send",
                NFC_EIO,
            )))
        );
        assert!(transport.port.reads.is_empty());
    }
}
