#![allow(dead_code)]

use crate::rust_api::{
    BaudRate, ConnectionString, Error, Mode, ModulationType, OpenedDevice, Property,
};

const NFC_EIO: i32 = -1;
const NFC_ETIMEOUT: i32 = -6;
const NFC_EOPABORTED: i32 = -7;
const NFC_ENOTIMPL: i32 = -8;

const HOST_TO_PN53X_TFI: u8 = 0xD4;
const PN53X_TO_HOST_TFI: u8 = 0xD5;
const PN53X_GET_FIRMWARE_VERSION: u8 = 0x02;

pub(crate) const PN53X_ACK_FRAME: [u8; 6] = [0x00, 0x00, 0xff, 0x00, 0xff, 0x00];
const PN53X_EXTENDED_FRAME_DATA_MAX_LEN: usize = 264;
const PN53X_EXTENDED_FRAME_OVERHEAD: usize = 11;
const PN532_BUFFER_LEN: usize = PN53X_EXTENDED_FRAME_DATA_MAX_LEN + PN53X_EXTENDED_FRAME_OVERHEAD;

fn status_error(operation: &'static str, code: i32) -> Error {
    Error::DeviceOperationFailed { operation, code }
}

fn status_code(error: &Error) -> i32 {
    match error {
        Error::InvalidArgument(_) | Error::InvalidEncoding(_) | Error::InvalidConnectionString(_) => -2,
        Error::BufferTooSmall { .. } => -5,
        Error::DriverNotFound(_) => -4,
        Error::DriverOpenFailed(_) => -80,
        Error::UnsupportedOperation(_) => NFC_ENOTIMPL,
        Error::DeviceOperationFailed { code, .. } => *code,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Pn53xType {
    Unknown,
    Pn531,
    Pn532,
    Pn533,
    Rcs360,
}

impl Pn53xType {
    fn from_ic_byte(ic: u8) -> Self {
        match ic {
            0x31 => Self::Pn531,
            0x32 => Self::Pn532,
            0x33 => Self::Pn533,
            0x88 => Self::Rcs360,
            _ => Self::Unknown,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Unknown => "PN53x",
            Self::Pn531 => "PN531",
            Self::Pn532 => "PN532",
            Self::Pn533 => "PN533",
            Self::Rcs360 => "RCS360",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Pn53xPowerMode {
    Normal,
    PowerDown,
    LowVbat,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Pn53xFirmwareVersion {
    pub ic: u8,
    pub version: u8,
    pub revision: u8,
    pub support: u8,
}

impl Pn53xFirmwareVersion {
    fn chip_type(&self) -> Pn53xType {
        Pn53xType::from_ic_byte(self.ic)
    }

    fn text(&self) -> String {
        format!(
            "{} firmware v{}.{} support=0x{:02x}",
            self.chip_type().label(),
            self.version,
            self.revision,
            self.support
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PropertyState {
    handle_crc: bool,
    handle_parity: bool,
    activate_field: bool,
    activate_crypto1: bool,
    infinite_select: bool,
    accept_invalid_frames: bool,
    accept_multiple_frames: bool,
    auto_iso14443_4: bool,
    easy_framing: bool,
    force_iso14443_a: bool,
    force_iso14443_b: bool,
    force_speed_106: bool,
}

impl Default for PropertyState {
    fn default() -> Self {
        Self {
            handle_crc: true,
            handle_parity: true,
            activate_field: true,
            activate_crypto1: false,
            infinite_select: false,
            accept_invalid_frames: false,
            accept_multiple_frames: false,
            auto_iso14443_4: true,
            easy_framing: true,
            force_iso14443_a: false,
            force_iso14443_b: false,
            force_speed_106: false,
        }
    }
}

impl PropertyState {
    fn get(self, property: Property) -> Option<bool> {
        Some(match property {
            Property::HandleCrc => self.handle_crc,
            Property::HandleParity => self.handle_parity,
            Property::ActivateField => self.activate_field,
            Property::ActivateCrypto1 => self.activate_crypto1,
            Property::InfiniteSelect => self.infinite_select,
            Property::AcceptInvalidFrames => self.accept_invalid_frames,
            Property::AcceptMultipleFrames => self.accept_multiple_frames,
            Property::AutoIso14443_4 => self.auto_iso14443_4,
            Property::EasyFraming => self.easy_framing,
            Property::ForceIso14443A => self.force_iso14443_a,
            Property::ForceIso14443B => self.force_iso14443_b,
            Property::ForceSpeed106 => self.force_speed_106,
            Property::TimeoutCommand | Property::TimeoutAtr | Property::TimeoutCom => return None,
        })
    }

    fn set(&mut self, property: Property, value: bool) -> Result<(), Error> {
        match property {
            Property::HandleCrc => self.handle_crc = value,
            Property::HandleParity => self.handle_parity = value,
            Property::ActivateField => self.activate_field = value,
            Property::ActivateCrypto1 => self.activate_crypto1 = value,
            Property::InfiniteSelect => self.infinite_select = value,
            Property::AcceptInvalidFrames => self.accept_invalid_frames = value,
            Property::AcceptMultipleFrames => self.accept_multiple_frames = value,
            Property::AutoIso14443_4 => self.auto_iso14443_4 = value,
            Property::EasyFraming => self.easy_framing = value,
            Property::ForceIso14443A => self.force_iso14443_a = value,
            Property::ForceIso14443B => self.force_iso14443_b = value,
            Property::ForceSpeed106 => self.force_speed_106 = value,
            Property::TimeoutCommand | Property::TimeoutAtr | Property::TimeoutCom => {
                return Err(Error::InvalidArgument("property"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Pn53xCore {
    chip_type: Pn53xType,
    firmware: Option<Pn53xFirmwareVersion>,
    power_mode: Pn53xPowerMode,
    last_command: Option<u8>,
    last_status_byte: u8,
    timeout_command_ms: i32,
    timeout_atr_ms: i32,
    timeout_communication_ms: i32,
    properties: PropertyState,
}

impl Default for Pn53xCore {
    fn default() -> Self {
        Self {
            chip_type: Pn53xType::Unknown,
            firmware: None,
            power_mode: Pn53xPowerMode::LowVbat,
            last_command: None,
            last_status_byte: 0,
            timeout_command_ms: 500,
            timeout_atr_ms: 103,
            timeout_communication_ms: 52,
            properties: PropertyState::default(),
        }
    }
}

impl Pn53xCore {
    pub(crate) fn chip_type(&self) -> Pn53xType {
        self.chip_type
    }

    pub(crate) fn firmware(&self) -> Option<&Pn53xFirmwareVersion> {
        self.firmware.as_ref()
    }

    pub(crate) fn power_mode(&self) -> Pn53xPowerMode {
        self.power_mode
    }

    pub(crate) fn last_command(&self) -> Option<u8> {
        self.last_command
    }

    pub(crate) fn property_bool_state(&self, property: Property) -> Option<bool> {
        self.properties.get(property)
    }

    pub(crate) fn set_property_bool(&mut self, property: Property, enable: bool) -> Result<(), Error> {
        self.properties.set(property, enable)
    }

    pub(crate) fn set_property_int(&mut self, property: Property, value: i32) -> Result<(), Error> {
        match property {
            Property::TimeoutCommand => self.timeout_command_ms = value,
            Property::TimeoutAtr => self.timeout_atr_ms = value,
            Property::TimeoutCom => self.timeout_communication_ms = value,
            _ => return Err(Error::InvalidArgument("property")),
        }
        Ok(())
    }

    pub(crate) fn exchange_command<T: Pn53xTransport>(
        &mut self,
        transport: &mut T,
        command: u8,
        payload: &[u8],
        timeout_ms: i32,
    ) -> Result<Vec<u8>, Error> {
        if self.power_mode != Pn53xPowerMode::Normal {
            transport.wake_up()?;
            self.power_mode = Pn53xPowerMode::Normal;
        }

        let mut command_payload = Vec::with_capacity(payload.len() + 1);
        command_payload.push(command);
        command_payload.extend_from_slice(payload);

        let frame = build_frame(&command_payload)?;
        transport.send(&frame, timeout_ms)?;

        let mut ack = [0u8; PN53X_ACK_FRAME.len()];
        let ack_len = transport.receive(&mut ack, timeout_ms)?;
        if !is_ack_frame(&ack[..ack_len]) {
            return Err(status_error("pn53x_wait_for_ack", NFC_EIO));
        }

        let mut response = [0u8; PN532_BUFFER_LEN];
        let response_len = transport.receive(&mut response, timeout_ms)?;
        let payload = parse_response_frame(&response[..response_len], command)?;
        self.last_command = Some(command);
        Ok(payload)
    }

    pub(crate) fn get_firmware_version<T: Pn53xTransport>(
        &mut self,
        transport: &mut T,
        timeout_ms: i32,
    ) -> Result<Pn53xFirmwareVersion, Error> {
        let payload = self.exchange_command(transport, PN53X_GET_FIRMWARE_VERSION, &[], timeout_ms)?;
        if payload.len() < 4 {
            return Err(status_error("pn53x_get_firmware_version", NFC_EIO));
        }

        let firmware = Pn53xFirmwareVersion {
            ic: payload[0],
            version: payload[1],
            revision: payload[2],
            support: payload[3],
        };
        self.chip_type = firmware.chip_type();
        self.last_status_byte = payload.get(4).copied().unwrap_or(0);
        self.firmware = Some(firmware.clone());
        Ok(firmware)
    }
}

pub(crate) trait Pn53xTransport {
    fn send(&mut self, payload: &[u8], timeout_ms: i32) -> Result<(), Error>;
    fn receive(&mut self, buffer: &mut [u8], timeout_ms: i32) -> Result<usize, Error>;
    fn abort_command(&mut self) -> Result<(), Error>;

    fn wake_up(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

pub(crate) struct Pn53xDevice<T> {
    name: String,
    connstring: ConnectionString,
    transport: T,
    core: Pn53xCore,
    last_error: i32,
}

impl<T: Pn53xTransport + Send + 'static> Pn53xDevice<T> {
    pub(crate) fn probe_pn532(
        name: impl Into<String>,
        connstring: ConnectionString,
        mut transport: T,
        timeout_ms: i32,
    ) -> Result<Self, Error> {
        let mut core = Pn53xCore::default();
        core.get_firmware_version(&mut transport, timeout_ms)?;
        Ok(Self {
            name: name.into(),
            connstring,
            transport,
            core,
            last_error: 0,
        })
    }

    #[allow(dead_code)]
    pub(crate) fn core(&self) -> &Pn53xCore {
        &self.core
    }

    fn remember<TValue>(&mut self, result: Result<TValue, Error>) -> Result<TValue, Error> {
        match &result {
            Ok(_) => self.last_error = 0,
            Err(error) => self.last_error = status_code(error),
        }
        result
    }

    fn firmware_text(&self) -> String {
        self.core
            .firmware()
            .map(Pn53xFirmwareVersion::text)
            .unwrap_or_else(|| format!("{} firmware unknown", self.core.chip_type().label()))
    }
}

impl<T: Pn53xTransport + Send + 'static> OpenedDevice for Pn53xDevice<T> {
    fn name(&self) -> &str {
        &self.name
    }

    fn connstring(&self) -> &ConnectionString {
        &self.connstring
    }

    fn last_error(&self) -> i32 {
        self.last_error
    }

    fn information_about(&mut self) -> Result<String, Error> {
        let message = format!("{} via {}", self.firmware_text(), self.connstring);
        self.last_error = 0;
        Ok(message)
    }

    fn set_property_bool(&mut self, property: Property, enable: bool) -> Result<(), Error> {
        let result = self.core.set_property_bool(property, enable);
        self.remember(result)
    }

    fn set_property_int(&mut self, property: Property, value: i32) -> Result<(), Error> {
        let result = self.core.set_property_int(property, value);
        self.remember(result)
    }

    fn supported_modulations(&mut self, mode: Mode) -> Result<Vec<ModulationType>, Error> {
        self.last_error = 0;
        Ok(match mode {
            Mode::Initiator => vec![
                ModulationType::Iso14443A,
                ModulationType::Jewel,
                ModulationType::Iso14443B,
                ModulationType::Felica,
                ModulationType::Dep,
            ],
            Mode::Target => vec![
                ModulationType::Iso14443A,
                ModulationType::Felica,
                ModulationType::Dep,
            ],
        })
    }

    fn supported_baud_rates(
        &mut self,
        mode: Mode,
        modulation_type: ModulationType,
    ) -> Result<Vec<BaudRate>, Error> {
        self.last_error = 0;
        Ok(match (mode, modulation_type) {
            (_, ModulationType::Iso14443A)
            | (_, ModulationType::Iso14443B)
            | (_, ModulationType::Jewel) => vec![BaudRate::Br106],
            (_, ModulationType::Felica) => vec![BaudRate::Br212, BaudRate::Br424],
            (_, ModulationType::Dep) => {
                vec![BaudRate::Br106, BaudRate::Br212, BaudRate::Br424]
            }
            _ => Vec::new(),
        })
    }

    fn property_bool_state(&self, property: Property) -> Option<bool> {
        self.core.property_bool_state(property)
    }

    fn initiator_init_driver(&mut self) -> Result<i32, Error> {
        self.last_error = 0;
        Ok(0)
    }

    fn abort_command_driver(&mut self) -> Result<(), Error> {
        let result = self.transport.abort_command();
        self.remember(result)
    }

    fn idle_driver(&mut self) -> Result<(), Error> {
        self.last_error = 0;
        Ok(())
    }

    fn powerdown_driver(&mut self) -> Result<(), Error> {
        self.core.power_mode = Pn53xPowerMode::PowerDown;
        self.last_error = 0;
        Ok(())
    }
}

pub(crate) fn build_frame(payload: &[u8]) -> Result<Vec<u8>, Error> {
    if payload.is_empty() {
        return Err(Error::InvalidArgument("payload"));
    }

    if payload.len() > PN53X_EXTENDED_FRAME_DATA_MAX_LEN {
        return Err(Error::BufferTooSmall {
            needed: payload.len(),
            available: PN53X_EXTENDED_FRAME_DATA_MAX_LEN,
        });
    }

    let mut frame = Vec::with_capacity(PN532_BUFFER_LEN);
    if payload.len() <= 254 {
        let len = payload.len() as u8 + 1;
        frame.extend_from_slice(&[
            0x00,
            0x00,
            0xff,
            len,
            (!len).wrapping_add(1),
            HOST_TO_PN53X_TFI,
        ]);
        frame.extend_from_slice(payload);
    } else {
        let high = ((payload.len() + 1) >> 8) as u8;
        let low = ((payload.len() + 1) & 0xff) as u8;
        frame.extend_from_slice(&[
            0x00,
            0x00,
            0xff,
            0xff,
            0xff,
            high,
            low,
            (0u8).wrapping_sub(high.wrapping_add(low)),
            HOST_TO_PN53X_TFI,
        ]);
        frame.extend_from_slice(payload);
    }

    let dcs = payload
        .iter()
        .fold(0u8.wrapping_sub(HOST_TO_PN53X_TFI), |acc, byte| {
            acc.wrapping_sub(*byte)
        });
    frame.push(dcs);
    frame.push(0x00);
    Ok(frame)
}

pub(crate) fn is_ack_frame(frame: &[u8]) -> bool {
    frame.starts_with(&PN53X_ACK_FRAME)
}

fn parse_response_frame(frame: &[u8], expected_command: u8) -> Result<Vec<u8>, Error> {
    if frame.len() < 8 {
        return Err(status_error("pn53x_parse_response_frame", NFC_EIO));
    }
    if is_ack_frame(frame) {
        return Err(status_error("pn53x_parse_response_frame", NFC_EIO));
    }
    if !frame.starts_with(&[0x00, 0x00, 0xff]) {
        return Err(status_error("pn53x_parse_response_frame", NFC_EIO));
    }

    let (body_offset, body_len) = if frame[3] == 0xff && frame[4] == 0xff {
        if frame.len() < 11 {
            return Err(status_error("pn53x_parse_response_frame", NFC_EIO));
        }
        let length = ((frame[5] as usize) << 8) | frame[6] as usize;
        let checksum = frame[5].wrapping_add(frame[6]).wrapping_add(frame[7]);
        if checksum != 0 {
            return Err(status_error("pn53x_parse_response_frame", NFC_EIO));
        }
        (8usize, length)
    } else {
        let length = frame[3] as usize;
        let checksum = frame[3].wrapping_add(frame[4]);
        if checksum != 0 {
            return Err(status_error("pn53x_parse_response_frame", NFC_EIO));
        }
        (5usize, length)
    };

    let trailer_offset = body_offset + body_len;
    if frame.len() < trailer_offset + 2 || body_len < 2 {
        return Err(status_error("pn53x_parse_response_frame", NFC_EIO));
    }

    let body = &frame[body_offset..trailer_offset];
    if body[0] != PN53X_TO_HOST_TFI || body[1] != expected_command.wrapping_add(1) {
        return Err(status_error("pn53x_parse_response_frame", NFC_EIO));
    }

    let expected_dcs = body
        .iter()
        .fold(0u8, |sum, byte| sum.wrapping_add(*byte))
        .wrapping_neg();
    if frame[trailer_offset] != expected_dcs || frame[trailer_offset + 1] != 0x00 {
        return Err(status_error("pn53x_parse_response_frame", NFC_EIO));
    }

    Ok(body[2..].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[derive(Default)]
    struct FakeTransport {
        sent: Vec<Vec<u8>>,
        received: VecDeque<Vec<u8>>,
        wake_up_calls: usize,
        abort_calls: usize,
    }

    impl Pn53xTransport for FakeTransport {
        fn send(&mut self, payload: &[u8], _timeout_ms: i32) -> Result<(), Error> {
            self.sent.push(payload.to_vec());
            Ok(())
        }

        fn receive(&mut self, buffer: &mut [u8], _timeout_ms: i32) -> Result<usize, Error> {
            let payload = self
                .received
                .pop_front()
                .ok_or_else(|| status_error("receive", NFC_ETIMEOUT))?;
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
            self.abort_calls += 1;
            Ok(())
        }

        fn wake_up(&mut self) -> Result<(), Error> {
            self.wake_up_calls += 1;
            Ok(())
        }
    }

    fn response_frame(command: u8, payload: &[u8]) -> Vec<u8> {
        let body_len = payload.len() + 2;
        let len = body_len as u8;
        let mut frame = vec![
            0x00,
            0x00,
            0xff,
            len,
            (!len).wrapping_add(1),
            PN53X_TO_HOST_TFI,
            command.wrapping_add(1),
        ];
        frame.extend_from_slice(payload);
        let dcs = frame[5..]
            .iter()
            .fold(0u8, |sum, byte| sum.wrapping_add(*byte))
            .wrapping_neg();
        frame.push(dcs);
        frame.push(0x00);
        frame
    }

    #[test]
    fn build_frame_supports_standard_frames() {
        let frame = build_frame(&[0x02, 0x03, 0x04]).unwrap();
        assert_eq!(
            frame,
            vec![0x00, 0x00, 0xff, 0x04, 0xfc, 0xD4, 0x02, 0x03, 0x04, 0x23, 0x00]
        );
    }

    #[test]
    fn build_frame_supports_extended_frames() {
        let payload = vec![0xAB; 255];
        let frame = build_frame(&payload).unwrap();
        assert_eq!(&frame[..9], &[0x00, 0x00, 0xff, 0xff, 0xff, 0x01, 0x00, 0xff, 0xD4]);
        assert_eq!(frame.len(), payload.len() + 11);
        assert_eq!(*frame.last().unwrap(), 0x00);
    }

    #[test]
    fn build_frame_rejects_empty_payloads() {
        assert_eq!(build_frame(&[]), Err(Error::InvalidArgument("payload")));
    }

    #[test]
    fn build_frame_rejects_oversized_payloads() {
        let payload = vec![0xAA; PN53X_EXTENDED_FRAME_DATA_MAX_LEN + 1];
        assert_eq!(
            build_frame(&payload),
            Err(Error::BufferTooSmall {
                needed: payload.len(),
                available: PN53X_EXTENDED_FRAME_DATA_MAX_LEN,
            })
        );
    }

    #[test]
    fn ack_frame_helper_matches_prefix() {
        assert!(is_ack_frame(&PN53X_ACK_FRAME));
        assert!(is_ack_frame(&[0x00, 0x00, 0xff, 0x00, 0xff, 0x00, 0x90]));
        assert!(!is_ack_frame(&[0x00, 0x00, 0xff, 0x01, 0xff, 0x00]));
    }

    #[test]
    fn parse_response_frame_validates_payload_and_command() {
        let frame = response_frame(0x02, &[0x32, 0x01, 0x06, 0x07]);
        assert_eq!(
            parse_response_frame(&frame, 0x02).unwrap(),
            vec![0x32, 0x01, 0x06, 0x07]
        );
    }

    #[test]
    fn exchange_command_wakes_up_and_tracks_last_command() {
        let mut transport = FakeTransport::default();
        transport.received.push_back(PN53X_ACK_FRAME.to_vec());
        transport
            .received
            .push_back(response_frame(0x02, &[0x32, 0x01, 0x06, 0x07]));

        let mut core = Pn53xCore::default();
        let payload = core.get_firmware_version(&mut transport, 25).unwrap();

        assert_eq!(transport.wake_up_calls, 1);
        assert_eq!(transport.sent.len(), 1);
        assert_eq!(payload.ic, 0x32);
        assert_eq!(core.chip_type(), Pn53xType::Pn532);
        assert_eq!(core.last_command(), Some(0x02));
        assert_eq!(core.power_mode(), Pn53xPowerMode::Normal);
    }

    #[test]
    fn probe_builds_pure_rust_device_and_reports_information() {
        let mut transport = FakeTransport::default();
        transport.received.push_back(PN53X_ACK_FRAME.to_vec());
        transport
            .received
            .push_back(response_frame(0x02, &[0x32, 0x01, 0x06, 0x07]));

        let connstring = ConnectionString::new("pn532_uart:/dev/ttyUSB0:115200").unwrap();
        let mut device = Pn53xDevice::probe_pn532("PN532", connstring, transport, 25).unwrap();

        assert_eq!(device.name(), "PN532");
        assert_eq!(device.last_error(), 0);
        assert_eq!(
            device.information_about().unwrap(),
            "PN532 firmware v1.6 support=0x07 via pn532_uart:/dev/ttyUSB0:115200"
        );
    }

    #[test]
    fn device_property_state_and_initiator_defaults_are_pure_rust() {
        let mut transport = FakeTransport::default();
        transport.received.push_back(PN53X_ACK_FRAME.to_vec());
        transport
            .received
            .push_back(response_frame(0x02, &[0x32, 0x01, 0x06, 0x07]));

        let connstring = ConnectionString::new("pn532_uart:/dev/null:115200").unwrap();
        let mut device = Pn53xDevice::probe_pn532("PN532", connstring, transport, 25).unwrap();

        assert_eq!(device.property_bool_state(Property::EasyFraming), Some(true));
        device.set_property_bool(Property::EasyFraming, false).unwrap();
        device.set_property_int(Property::TimeoutCommand, 900).unwrap();
        device.initiator_init().unwrap();

        assert_eq!(device.property_bool_state(Property::EasyFraming), Some(false));
        assert_eq!(device.property_bool_state(Property::InfiniteSelect), Some(true));
        assert_eq!(device.property_bool_state(Property::ForceSpeed106), Some(true));
        assert_eq!(device.last_error(), 0);
    }

    #[test]
    fn abort_command_delegates_to_transport() {
        let mut transport = FakeTransport::default();
        transport.received.push_back(PN53X_ACK_FRAME.to_vec());
        transport
            .received
            .push_back(response_frame(0x02, &[0x32, 0x01, 0x06, 0x07]));

        let connstring = ConnectionString::new("pn532_uart:/dev/null:115200").unwrap();
        let mut device = Pn53xDevice::probe_pn532("PN532", connstring, transport, 25).unwrap();
        device.abort_command().unwrap();

        assert_eq!(device.transport.abort_calls, 1);
    }

    #[test]
    fn transport_timeout_is_preserved_as_device_error() {
        let mut transport = FakeTransport::default();
        transport.received.push_back(PN53X_ACK_FRAME.to_vec());

        let mut core = Pn53xCore::default();
        let error = core.get_firmware_version(&mut transport, 25).unwrap_err();

        assert_eq!(
            error,
            Error::DeviceOperationFailed {
                operation: "receive",
                code: NFC_ETIMEOUT,
            }
        );
    }

    #[test]
    fn status_constants_match_expected_negative_codes() {
        assert_eq!(NFC_EOPABORTED, -7);
    }
}
