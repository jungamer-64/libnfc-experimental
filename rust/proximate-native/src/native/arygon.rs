use super::connstring::{build_path_speed_connstring, decode_path_speed_descriptor};
use super::pn53x::{Pn53xDevice, Pn53xProfile, Pn53xTransport, is_ack_frame};
use super::uart::{UartPort, list_candidate_paths};
use proximate_driver::{ConnectionString, Context, DeviceHandle, Driver, Error, ScanType};
use std::borrow::Cow;

const DRIVER_NAME: &str = "arygon";
const DEFAULT_SPEED: u32 = 9_600;
const PROBE_TIMEOUT_MS: i32 = 250;
const CONTROL_TIMEOUT_MS: i32 = 1_000;
const FIRMWARE_BUFFER_LEN: usize = 16;
const RESET_BUFFER_LEN: usize = 10;

const NFC_EIO: i32 = -1;

const PROTOCOL_ARYGON_ASCII: u8 = b'0';
const PROTOCOL_TAMA: u8 = b'2';

const ERROR_NONE: &[u8] = b"FF000000\r\n";
const ERROR_UNKNOWN_MODE_PREFIX: &[u8] = b"FF0600";
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

            #[cfg(target_os = "linux")]
            {
                let Ok(mut port) = UartPort::open(&path, DEFAULT_SPEED) else {
                    continue;
                };
                if reset_tama(&mut port).is_ok() {
                    devices.push(self.describe_discovered(
                        format!("{DRIVER_NAME}:{path}"),
                        connstring,
                        Some(super::pn53x::scan_caps(Pn53xProfile::arygon())),
                    ));
                }
            }

            #[cfg(not(target_os = "linux"))]
            {
                let _ = connstring;
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
                PROBE_TIMEOUT_MS,
            )?;
            Ok(Box::new(device))
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = descriptor;
            Err(Error::DriverOpenFailed(
                "arygon is only available on Linux in this phase".into(),
            ))
        }
    }
}

struct ArygonTransport {
    port: UartPort,
}

impl ArygonTransport {
    fn new(port: UartPort) -> Self {
        Self { port }
    }
}

impl Pn53xTransport for ArygonTransport {
    fn send(&mut self, payload: &[u8], timeout_ms: i32) -> Result<(), Error> {
        self.port.flush_input()?;

        let mut prefixed = Vec::with_capacity(payload.len() + 1);
        prefixed.push(PROTOCOL_TAMA);
        prefixed.extend_from_slice(payload);
        self.port.write_all(&prefixed, timeout_ms)?;

        let mut ack = [0u8; 16];
        let ack_len = self.port.read_frame_into(&mut ack, timeout_ms)?;
        if is_ack_frame(&ack[..ack_len]) {
            return Ok(());
        }

        if ack[..ack_len].starts_with(ERROR_UNKNOWN_MODE_PREFIX) {
            let mut rest = [0u8; 4];
            let _ = self.port.read_exact(&mut rest, timeout_ms);
        }

        Err(device_error("arygon_send", NFC_EIO))
    }

    fn receive(&mut self, buffer: &mut [u8], timeout_ms: i32) -> Result<usize, Error> {
        self.port.read_frame_into(buffer, timeout_ms)
    }

    fn abort_command(&mut self) -> Result<(), Error> {
        self.port.abort_command()
    }
}

fn device_error(operation: &'static str, code: i32) -> Error {
    Error::DeviceOperationFailed { operation, code }
}

fn query_ascii_command(
    port: &mut UartPort,
    command: &[u8],
    response_len: usize,
    timeout_ms: i32,
) -> Result<Vec<u8>, Error> {
    port.flush_input()?;
    port.write_all(command, timeout_ms)?;
    let mut response = vec![0u8; response_len];
    port.read_exact(&mut response, timeout_ms)?;
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
        CONTROL_TIMEOUT_MS,
    )?;
    parse_firmware(&response)
}

fn reset_tama(port: &mut UartPort) -> Result<(), Error> {
    let response = query_ascii_command(
        port,
        RESET_TAMA_COMMAND,
        RESET_BUFFER_LEN,
        CONTROL_TIMEOUT_MS,
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
}
