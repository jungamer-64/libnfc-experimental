use super::acr122;
use super::connstring::{build_path_speed_connstring, decode_path_speed_descriptor};
use super::pn53x::{
    PN53X_ACK_FRAME, Pn53xDevice, Pn53xProfile, Pn53xTransport, build_response_frame,
    payload_from_host_frame,
};
use super::uart::{UartPort, list_candidate_paths};
use proximate_driver::{
    ConnectionString, Context, DeviceBackend, Driver, Error, Property, PropertyBackend, ScanType,
};
use std::collections::VecDeque;
#[cfg(test)]
use std::sync::{Arc, Mutex};

const DRIVER_NAME: &str = "ACR122S";
const DEFAULT_SPEED: u32 = 9_600;
const PROBE_TIMEOUT_MS: i32 = 250;
const CONTROL_TIMEOUT_MS: i32 = 1_000;

const STX: u8 = 0x02;
const ETX: u8 = 0x03;
const FRAME_OVERHEAD: usize = 13;
const MAX_FRAME_SIZE: usize = FRAME_OVERHEAD + 5 + 255;

const NFC_EIO: i32 = -1;
#[cfg_attr(not(test), allow(dead_code))]
const NFC_EINVARG: i32 = -2;

const ICC_POWER_ON_REQ_MSG: u8 = 0x62;
const ICC_POWER_OFF_REQ_MSG: u8 = 0x63;
const XFR_BLOCK_REQ_MSG: u8 = 0x6f;

pub(crate) struct Acr122sDriver;

impl Acr122sDriver {
    pub(crate) const fn new() -> Self {
        Self
    }
}

impl Driver for Acr122sDriver {
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
                let Ok(mut port) = UartPort::open(&path, DEFAULT_SPEED) else {
                    continue;
                };
                port.flush_input()?;
                let mut seq = 0u8;
                let Ok(firmware) = fetch_firmware_version(&mut port, &mut seq) else {
                    continue;
                };
                if acr122::is_acr122s_firmware(&firmware) {
                    devices.push(connstring);
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
    ) -> Result<Box<dyn DeviceBackend>, Error> {
        let descriptor = decode_path_speed_descriptor(connstring, DRIVER_NAME, DEFAULT_SPEED)?;

        #[cfg(target_os = "linux")]
        {
            let mut port = UartPort::open(&descriptor.path, descriptor.speed)?;
            port.flush_input()?;

            let mut seq = 0u8;
            let firmware = fetch_firmware_version(&mut port, &mut seq)?;
            if !acr122::is_acr122s_firmware(&firmware) {
                return Err(Error::DriverOpenFailed(format!(
                    "invalid ACR122S firmware '{firmware}'"
                )));
            }

            power_command(&mut port, &mut seq, ICC_POWER_ON_REQ_MSG, 0)?;

            let transport = Acr122sTransport::new(port, seq);
            let mut device = Pn53xDevice::probe_with_profile(
                firmware,
                connstring.clone(),
                Pn53xProfile::acr122s(),
                transport,
                PROBE_TIMEOUT_MS,
            )?;
            device.set_property_int(Property::TimeoutCommand, CONTROL_TIMEOUT_MS)?;
            Ok(Box::new(device))
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = descriptor;
            Err(Error::DriverOpenFailed(
                "ACR122S is only available on Linux in this phase".into(),
            ))
        }
    }
}

trait Acr122sIo: Send {
    fn flush_input(&mut self) -> Result<(), Error>;
    fn write_all(&mut self, payload: &[u8], timeout_ms: i32) -> Result<(), Error>;
    fn read_exact(&mut self, buffer: &mut [u8], timeout_ms: i32) -> Result<(), Error>;
    fn abort_command(&mut self) -> Result<(), Error>;
}

#[cfg(target_os = "linux")]
impl Acr122sIo for UartPort {
    fn flush_input(&mut self) -> Result<(), Error> {
        UartPort::flush_input(self)
    }

    fn write_all(&mut self, payload: &[u8], timeout_ms: i32) -> Result<(), Error> {
        UartPort::write_all(self, payload, timeout_ms)
    }

    fn read_exact(&mut self, buffer: &mut [u8], timeout_ms: i32) -> Result<(), Error> {
        UartPort::read_exact(self, buffer, timeout_ms)
    }

    fn abort_command(&mut self) -> Result<(), Error> {
        <UartPort as Pn53xTransport>::abort_command(self)
    }
}

#[cfg(not(target_os = "linux"))]
impl Acr122sIo for UartPort {
    fn flush_input(&mut self) -> Result<(), Error> {
        Err(Error::DriverOpenFailed(
            "ACR122S is only available on Linux in this phase".into(),
        ))
    }

    fn write_all(&mut self, _payload: &[u8], _timeout_ms: i32) -> Result<(), Error> {
        self.flush_input()
    }

    fn read_exact(&mut self, _buffer: &mut [u8], _timeout_ms: i32) -> Result<(), Error> {
        self.flush_input()
    }

    fn abort_command(&mut self) -> Result<(), Error> {
        self.flush_input()
    }
}

struct Acr122sTransport<IO: Acr122sIo> {
    io: IO,
    seq: u8,
    pending: VecDeque<Vec<u8>>,
    deactivate_on_drop: bool,
}

impl<IO: Acr122sIo> Acr122sTransport<IO> {
    fn new(io: IO, seq: u8) -> Self {
        Self {
            io,
            seq,
            pending: VecDeque::new(),
            deactivate_on_drop: true,
        }
    }

    #[cfg(test)]
    fn without_power_off(io: IO, seq: u8) -> Self {
        Self {
            io,
            seq,
            pending: VecDeque::new(),
            deactivate_on_drop: false,
        }
    }
}

impl<IO: Acr122sIo> Drop for Acr122sTransport<IO> {
    fn drop(&mut self) {
        if self.deactivate_on_drop {
            let _ = power_command(&mut self.io, &mut self.seq, ICC_POWER_OFF_REQ_MSG, 0);
        }
    }
}

impl<IO: Acr122sIo> Pn53xTransport for Acr122sTransport<IO> {
    fn send(&mut self, payload: &[u8], timeout_ms: i32) -> Result<(), Error> {
        self.io.flush_input()?;
        let host_payload = payload_from_host_frame(payload)?;
        let command = *host_payload
            .first()
            .ok_or_else(|| device_error("acr122s_send", NFC_EIO))?;
        let response = direct_transmit(
            &mut self.io,
            &mut self.seq,
            command,
            &host_payload,
            timeout_ms,
        )?;
        let frame = build_response_frame(command, &response)?;
        self.pending.clear();
        self.pending.push_back(PN53X_ACK_FRAME.to_vec());
        self.pending.push_back(frame);
        Ok(())
    }

    fn receive(&mut self, buffer: &mut [u8], _timeout_ms: i32) -> Result<usize, Error> {
        let payload = self
            .pending
            .pop_front()
            .ok_or_else(|| device_error("acr122s_pending_receive", NFC_EIO))?;
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
        self.pending.clear();
        self.io.abort_command()
    }
}

fn device_error(operation: &'static str, code: i32) -> Error {
    Error::DeviceOperationFailed { operation, code }
}

fn build_frame(message_type: u8, seq: u8, message_specific: [u8; 3], payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(FRAME_OVERHEAD + payload.len());
    frame.push(STX);
    frame.push(message_type);
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.push(0x00);
    frame.push(seq);
    frame.extend_from_slice(&message_specific);
    frame.extend_from_slice(payload);
    let checksum = frame[1..].iter().fold(0u8, |acc, byte| acc ^ *byte);
    frame.push(checksum);
    frame.push(ETX);
    frame
}

fn validate_ack(ack: &[u8]) -> Result<(), Error> {
    if ack == [STX, 0x00, 0x00, ETX] {
        Ok(())
    } else {
        Err(device_error("acr122s_ack", NFC_EIO))
    }
}

fn validate_frame(frame: &[u8], expected_seq: u8) -> Result<(), Error> {
    if frame.len() < FRAME_OVERHEAD || frame[0] != STX || *frame.last().unwrap_or(&0) != ETX {
        return Err(device_error("acr122s_frame", NFC_EIO));
    }
    let checksum = frame[1..frame.len() - 2]
        .iter()
        .fold(0u8, |acc, byte| acc ^ *byte);
    if checksum != frame[frame.len() - 2] {
        return Err(device_error("acr122s_frame", NFC_EIO));
    }
    if frame[7] != expected_seq {
        return Err(device_error("acr122s_frame", NFC_EIO));
    }
    Ok(())
}

fn transact<IO: Acr122sIo>(
    io: &mut IO,
    seq: &mut u8,
    message_type: u8,
    message_specific: [u8; 3],
    payload: &[u8],
    timeout_ms: i32,
) -> Result<Vec<u8>, Error> {
    let sent_seq = *seq;
    let frame = build_frame(message_type, sent_seq, message_specific, payload);
    io.write_all(&frame, timeout_ms)?;

    let mut ack = [0u8; 4];
    io.read_exact(&mut ack, timeout_ms)?;
    validate_ack(&ack)?;

    *seq = sent_seq.wrapping_add(1);

    let mut header = [0u8; 11];
    io.read_exact(&mut header, timeout_ms)?;
    let payload_len = u32::from_le_bytes([header[2], header[3], header[4], header[5]]) as usize;
    let frame_len = FRAME_OVERHEAD + payload_len;
    if !(FRAME_OVERHEAD..=MAX_FRAME_SIZE).contains(&frame_len) {
        return Err(device_error("acr122s_frame", NFC_EIO));
    }

    let mut frame = vec![0u8; frame_len];
    frame[..header.len()].copy_from_slice(&header);
    io.read_exact(&mut frame[header.len()..], timeout_ms)?;
    validate_frame(&frame, sent_seq)?;
    Ok(frame[11..frame.len() - 2].to_vec())
}

fn extract_direct_transmit_payload(command: u8, response: &[u8]) -> Result<Option<Vec<u8>>, Error> {
    if response.is_empty() || response[0] != 0xD5 {
        return Ok(None);
    }
    if response.len() < 4 || response[1] != command.wrapping_add(1) {
        return Err(device_error("acr122s_receive", NFC_EIO));
    }
    let status = acr122::parse_status_words(&response[response.len() - 2..])
        .ok_or_else(|| device_error("acr122s_receive", NFC_EIO))?;
    if !status.ok {
        return Err(device_error("acr122s_receive", NFC_EIO));
    }
    Ok(Some(response[2..response.len() - 2].to_vec()))
}

fn complete_direct_transmit<IO: Acr122sIo>(
    io: &mut IO,
    seq: &mut u8,
    command: u8,
    response: Vec<u8>,
    timeout_ms: i32,
) -> Result<Vec<u8>, Error> {
    if let Some(payload) = extract_direct_transmit_payload(command, &response)? {
        return Ok(payload);
    }

    let status = acr122::parse_status_words(&response)
        .ok_or_else(|| device_error("acr122s_receive", NFC_EIO))?;
    if !status.has_more_data {
        return Err(device_error("acr122s_receive", NFC_EIO));
    }

    let follow_up = acr122::build_get_additional_data_apdu(status.more_data_length)?;
    let follow_up_response = transact(
        io,
        seq,
        XFR_BLOCK_REQ_MSG,
        [0x00, 0x00, 0x00],
        &follow_up,
        timeout_ms,
    )?;
    extract_direct_transmit_payload(command, &follow_up_response)?
        .ok_or_else(|| device_error("acr122s_receive", NFC_EIO))
}

fn direct_transmit<IO: Acr122sIo>(
    io: &mut IO,
    seq: &mut u8,
    command: u8,
    host_payload: &[u8],
    timeout_ms: i32,
) -> Result<Vec<u8>, Error> {
    let apdu = acr122::build_direct_transmit_apdu(host_payload)?;
    let response = transact(
        io,
        seq,
        XFR_BLOCK_REQ_MSG,
        [0x00, 0x00, 0x00],
        &apdu,
        timeout_ms,
    )?;
    complete_direct_transmit(io, seq, command, response, timeout_ms)
}

fn power_command<IO: Acr122sIo>(
    io: &mut IO,
    seq: &mut u8,
    message_type: u8,
    timeout_ms: i32,
) -> Result<(), Error> {
    let _ = transact(io, seq, message_type, [0x00, 0x00, 0x00], &[], timeout_ms)?;
    Ok(())
}

fn fetch_firmware_version<IO: Acr122sIo>(io: &mut IO, seq: &mut u8) -> Result<String, Error> {
    let apdu = acr122::build_get_firmware_version_apdu()?;
    let response = transact(
        io,
        seq,
        XFR_BLOCK_REQ_MSG,
        [0x00, 0x00, 0x00],
        &apdu,
        CONTROL_TIMEOUT_MS,
    )?;
    Ok(String::from_utf8_lossy(&response)
        .trim_end_matches('\0')
        .to_string())
}

#[cfg(test)]
#[derive(Clone, Default)]
struct FakeIo {
    state: Arc<Mutex<FakeIoState>>,
}

#[cfg(test)]
#[derive(Default)]
struct FakeIoState {
    writes: Vec<Vec<u8>>,
    reads: VecDeque<Vec<u8>>,
    aborts: usize,
}

#[cfg(test)]
impl FakeIo {
    fn with_reads(reads: impl IntoIterator<Item = Vec<u8>>) -> Self {
        let state = FakeIoState {
            writes: Vec::new(),
            reads: reads.into_iter().collect(),
            aborts: 0,
        };
        Self {
            state: Arc::new(Mutex::new(state)),
        }
    }

    fn writes(&self) -> Vec<Vec<u8>> {
        self.state.lock().expect("poisoned fake io").writes.clone()
    }

    fn aborts(&self) -> usize {
        self.state.lock().expect("poisoned fake io").aborts
    }
}

#[cfg(test)]
impl Acr122sIo for FakeIo {
    fn flush_input(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn write_all(&mut self, payload: &[u8], _timeout_ms: i32) -> Result<(), Error> {
        self.state
            .lock()
            .expect("poisoned fake io")
            .writes
            .push(payload.to_vec());
        Ok(())
    }

    fn read_exact(&mut self, buffer: &mut [u8], _timeout_ms: i32) -> Result<(), Error> {
        let payload = self
            .state
            .lock()
            .expect("poisoned fake io")
            .reads
            .pop_front()
            .ok_or_else(|| device_error("fake_read_exact", NFC_EIO))?;
        if payload.len() != buffer.len() {
            return Err(device_error("fake_read_exact", NFC_EINVARG));
        }
        buffer.copy_from_slice(&payload);
        Ok(())
    }

    fn abort_command(&mut self) -> Result<(), Error> {
        self.state.lock().expect("poisoned fake io").aborts += 1;
        Ok(())
    }
}

#[cfg(test)]
fn split_frame_for_reads(frame: Vec<u8>) -> [Vec<u8>; 2] {
    [frame[..11].to_vec(), frame[11..].to_vec()]
}

#[cfg(test)]
fn build_response_frame_payload(message_type: u8, seq: u8, payload: &[u8]) -> [Vec<u8>; 2] {
    split_frame_for_reads(build_frame(message_type, seq, [0x00, 0x00, 0x00], payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firmware_probe_and_power_on_use_expected_frames() {
        let [firmware_header, firmware_tail] =
            build_response_frame_payload(0x80, 0x00, b"ACR122S101");
        let [power_header, power_tail] = build_response_frame_payload(0x80, 0x01, &[]);
        let io = FakeIo::with_reads([
            vec![STX, 0x00, 0x00, ETX],
            firmware_header,
            firmware_tail,
            vec![STX, 0x00, 0x00, ETX],
            power_header,
            power_tail,
        ]);
        let mut io_clone = io.clone();
        let mut seq = 0u8;

        let firmware = fetch_firmware_version(&mut io_clone, &mut seq).unwrap();
        assert_eq!(firmware, "ACR122S101");
        power_command(&mut io_clone, &mut seq, ICC_POWER_ON_REQ_MSG, 0).unwrap();

        let writes = io.writes();
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0][1], XFR_BLOCK_REQ_MSG);
        assert_eq!(writes[1][1], ICC_POWER_ON_REQ_MSG);
        assert_eq!(seq, 2);
    }

    #[test]
    fn transport_builds_pending_ack_and_response_frames() {
        let [header, tail] = build_response_frame_payload(
            0x80,
            0x00,
            &[0xD5, 0x03, 0x32, 0x01, 0x06, 0x07, 0x90, 0x00],
        );
        let io = FakeIo::with_reads([vec![STX, 0x00, 0x00, ETX], header, tail]);
        let mut transport = Acr122sTransport::without_power_off(io.clone(), 0);

        let frame = crate::native::pn53x::build_frame(&[0x02]).unwrap();
        transport.send(&frame, 25).unwrap();

        let mut ack = [0u8; 6];
        assert_eq!(transport.receive(&mut ack, 25).unwrap(), 6);
        assert_eq!(ack, PN53X_ACK_FRAME);

        let mut response = [0u8; 32];
        let size = transport.receive(&mut response, 25).unwrap();
        assert!(size > 0);

        let writes = io.writes();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0][1], XFR_BLOCK_REQ_MSG);
    }

    #[test]
    fn transport_fetches_additional_data_when_reader_requests_follow_up() {
        let [first_header, first_tail] = build_response_frame_payload(0x80, 0x00, &[0x61, 0x04]);
        let [second_header, second_tail] = build_response_frame_payload(
            0x80,
            0x01,
            &[0xD5, 0x03, 0x32, 0x01, 0x06, 0x07, 0x90, 0x00],
        );
        let io = FakeIo::with_reads([
            vec![STX, 0x00, 0x00, ETX],
            first_header,
            first_tail,
            vec![STX, 0x00, 0x00, ETX],
            second_header,
            second_tail,
        ]);
        let mut transport = Acr122sTransport::without_power_off(io.clone(), 0);

        let frame = crate::native::pn53x::build_frame(&[0x02]).unwrap();
        transport.send(&frame, 25).unwrap();

        let writes = io.writes();
        assert_eq!(writes.len(), 2);
        let follow_up_payload = &writes[1][11..writes[1].len() - 2];
        assert_eq!(follow_up_payload, [0xFF, 0xC0, 0x00, 0x00, 0x04].as_slice());
    }

    #[test]
    fn transport_abort_clears_pending_frames_and_forwards_abort() {
        let io = FakeIo::default();
        let mut transport = Acr122sTransport::without_power_off(io.clone(), 0);
        transport.pending.push_back(vec![1, 2, 3]);
        transport.abort_command().unwrap();
        assert!(transport.pending.is_empty());
        assert_eq!(io.aborts(), 1);
    }

    #[test]
    fn acr122s_driver_metadata_and_open_error_are_stable() {
        let driver = Acr122sDriver::new();
        assert_eq!(driver.name(), DRIVER_NAME);
        assert_eq!(driver.scan_type(), ScanType::Intrusive);

        let connstring = ConnectionString::new("ACR122S:/definitely/missing").unwrap();
        let error = match driver.open(&Context::new(), &connstring) {
            Ok(_) => panic!("expected missing serial path to fail"),
            Err(error) => error,
        };
        assert!(matches!(error, Error::DriverOpenFailed(_)));
    }
}
