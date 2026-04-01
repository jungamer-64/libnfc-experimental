#![allow(dead_code)]

use crate::rust_api::{
    BaudRate, ConnectionString, DepInfo, DepMode, Error, Mode, Modulation, ModulationType,
    OpenedDevice, Property, Target, TargetInfo,
};
use std::thread;
use std::time::Duration;

const NFC_EIO: i32 = -1;
const NFC_EINVARG: i32 = -2;
const NFC_EDEVNOTSUPP: i32 = -3;
const NFC_ENOTSUCHDEV: i32 = -4;
const NFC_EOVFLOW: i32 = -5;
const NFC_ETIMEOUT: i32 = -6;
const NFC_EOPABORTED: i32 = -7;
const NFC_ENOTIMPL: i32 = -8;
const NFC_ETGRELEASED: i32 = -10;
const NFC_ERFTRANS: i32 = -20;

const HOST_TO_PN53X_TFI: u8 = 0xD4;
const PN53X_TO_HOST_TFI: u8 = 0xD5;
const PN53X_GET_FIRMWARE_VERSION: u8 = 0x02;
const PN53X_READ_REGISTER: u8 = 0x06;
const PN53X_WRITE_REGISTER: u8 = 0x08;
const PN532_SAM_CONFIGURATION: u8 = 0x14;
const PN53X_IN_DATA_EXCHANGE: u8 = 0x40;
const PN53X_IN_COMMUNICATE_THRU: u8 = 0x42;
const PN53X_IN_DESELECT: u8 = 0x44;
const PN53X_IN_LIST_PASSIVE_TARGET: u8 = 0x4A;
const PN53X_IN_JUMP_FOR_DEP: u8 = 0x56;
const PN53X_TG_GET_DATA: u8 = 0x86;
const PN53X_TG_INIT_AS_TARGET: u8 = 0x8C;
const PN53X_TG_SET_DATA: u8 = 0x8E;
const PN53X_TG_GET_INITIATOR_COMMAND: u8 = 0x88;
const PN53X_TG_RESPONSE_TO_INITIATOR: u8 = 0x90;

const PN53X_STATUS_TIMEOUT: u8 = 0x01;
const PN53X_STATUS_CRC: u8 = 0x02;
const PN53X_STATUS_PARITY: u8 = 0x03;
const PN53X_STATUS_BITCOUNT: u8 = 0x04;
const PN53X_STATUS_FRAMING: u8 = 0x05;
const PN53X_STATUS_BITCOLL: u8 = 0x06;
const PN53X_STATUS_SMALLBUF: u8 = 0x07;
const PN53X_STATUS_BUFOVF: u8 = 0x09;
const PN53X_STATUS_RFTIMEOUT: u8 = 0x0a;
const PN53X_STATUS_RFPROTO: u8 = 0x0b;
const PN53X_STATUS_OVHEAT: u8 = 0x0d;
const PN53X_STATUS_INBUFOVF: u8 = 0x0e;
const PN53X_STATUS_INVPARAM: u8 = 0x10;
const PN53X_STATUS_DEPUNKCMD: u8 = 0x12;
const PN53X_STATUS_INVRXFRAM: u8 = 0x13;
const PN53X_STATUS_MFAUTH: u8 = 0x14;
const PN53X_STATUS_SECNOTSUPP: u8 = 0x18;
const PN53X_STATUS_BCC: u8 = 0x23;
const PN53X_STATUS_DEPINVSTATE: u8 = 0x25;
const PN53X_STATUS_OPNOTALL: u8 = 0x26;
const PN53X_STATUS_CMD: u8 = 0x27;
const PN53X_STATUS_TGREL: u8 = 0x29;
const PN53X_STATUS_CID: u8 = 0x2a;
const PN53X_STATUS_CDISCARDED: u8 = 0x2b;
const PN53X_STATUS_NFCID3: u8 = 0x2c;
const PN53X_STATUS_OVCURRENT: u8 = 0x2d;
const PN53X_STATUS_NAD: u8 = 0x2e;

const PN53X_TARGET_MODE_NORMAL: u8 = 0x00;
const PN53X_TARGET_MODE_PASSIVE_ONLY: u8 = 0x01;
const PN53X_TARGET_MODE_DEP_ONLY: u8 = 0x02;
const PN53X_TARGET_MODE_ISO14443_4_PICC_ONLY: u8 = 0x04;
const SAK_ISO14443_4_COMPLIANT: u8 = 0x20;
const SAK_MIFARE_CLASSIC_MASK: u8 = 0x08;

const PN53X_REG_CIU_TX_MODE: u16 = 0x6302;
const PN53X_REG_CIU_TMODE: u16 = 0x631a;
const PN53X_REG_CIU_TPRESCALER: u16 = 0x631b;
const PN53X_REG_CIU_TRELOAD_VAL_HI: u16 = 0x631c;
const PN53X_REG_CIU_TRELOAD_VAL_LO: u16 = 0x631d;
const PN53X_REG_CIU_TCOUNTER_VAL_HI: u16 = 0x631e;
const PN53X_REG_CIU_TCOUNTER_VAL_LO: u16 = 0x631f;
const PN53X_REG_CIU_COMMAND: u16 = 0x6331;
const PN53X_REG_CIU_FIFO_DATA: u16 = 0x6339;
const PN53X_REG_CIU_FIFO_LEVEL: u16 = 0x633a;
const PN53X_REG_CIU_CONTROL: u16 = 0x633c;
const PN53X_REG_CIU_BIT_FRAMING: u16 = 0x633d;
const SYMBOL_TX_CRC_ENABLE: u8 = 0x80;
const SYMBOL_TX_FRAMING: u8 = 0x03;
const SYMBOL_TAUTO: u8 = 0x80;
const SYMBOL_TPRESCALERHI: u8 = 0x0f;
const SYMBOL_TPRESCALERLO: u8 = 0xff;
const SYMBOL_COMMAND: u8 = 0x0f;
const SYMBOL_COMMAND_TRANSCEIVE: u8 = 0x0c;
const SYMBOL_FLUSH_BUFFER: u8 = 0x80;
const SYMBOL_FIFO_LEVEL: u8 = 0x7f;
const SYMBOL_START_SEND: u8 = 0x80;
const SYMBOL_RX_LAST_BITS: u8 = 0x07;
const SYMBOL_TX_LAST_BITS: u8 = 0x07;

pub(crate) const PN53X_ACK_FRAME: [u8; 6] = [0x00, 0x00, 0xff, 0x00, 0xff, 0x00];
const PN53X_EXTENDED_FRAME_DATA_MAX_LEN: usize = 264;
const PN53X_EXTENDED_FRAME_OVERHEAD: usize = 11;
const PN532_BUFFER_LEN: usize = PN53X_EXTENDED_FRAME_DATA_MAX_LEN + PN53X_EXTENDED_FRAME_OVERHEAD;

fn status_error(operation: &'static str, code: i32) -> Error {
    Error::DeviceOperationFailed { operation, code }
}

fn status_code(error: &Error) -> i32 {
    match error {
        Error::InvalidArgument(_)
        | Error::InvalidEncoding(_)
        | Error::InvalidConnectionString(_) => -2,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Pn532SamMode {
    Normal = 0x01,
    WiredCard = 0x03,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Pn53xUsbModel {
    Unknown,
    NxpPn531,
    NxpPn533,
    ScmScl3711,
    ScmScl3712,
    SonyPn531,
    AskLogo,
    SonyRcs360,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Pn53xProfile {
    pub driver_name: &'static str,
    pub initial_power_mode: Pn53xPowerMode,
    pub sam_mode_on_low_vbat: Option<Pn532SamMode>,
    pub secure_element_mode: Option<Pn532SamMode>,
    pub timer_correction: u32,
    pub usb_model: Option<Pn53xUsbModel>,
}

impl Pn53xProfile {
    pub(crate) const fn pn532(driver_name: &'static str) -> Self {
        Self {
            driver_name,
            initial_power_mode: Pn53xPowerMode::LowVbat,
            sam_mode_on_low_vbat: Some(Pn532SamMode::Normal),
            secure_element_mode: Some(Pn532SamMode::WiredCard),
            timer_correction: 48,
            usb_model: None,
        }
    }

    pub(crate) const fn pn53x_usb(model: Pn53xUsbModel) -> Self {
        Self {
            driver_name: "pn53x_usb",
            initial_power_mode: Pn53xPowerMode::Normal,
            sam_mode_on_low_vbat: None,
            secure_element_mode: None,
            timer_correction: match model {
                Pn53xUsbModel::ScmScl3711 | Pn53xUsbModel::ScmScl3712 | Pn53xUsbModel::NxpPn533 => {
                    46
                }
                Pn53xUsbModel::SonyPn531 => 54,
                Pn53xUsbModel::AskLogo | Pn53xUsbModel::NxpPn531 => 50,
                Pn53xUsbModel::SonyRcs360 | Pn53xUsbModel::Unknown => 0,
            },
            usb_model: Some(model),
        }
    }

    pub(crate) const fn acr122_pcsc() -> Self {
        Self {
            driver_name: "acr122_pcsc",
            initial_power_mode: Pn53xPowerMode::Normal,
            sam_mode_on_low_vbat: None,
            secure_element_mode: None,
            timer_correction: 50,
            usb_model: None,
        }
    }

    fn supported_modulations(self, mode: Mode) -> Vec<ModulationType> {
        match (self.usb_model, mode) {
            (Some(Pn53xUsbModel::AskLogo), Mode::Target) => Vec::new(),
            (_, Mode::Initiator) => vec![
                ModulationType::Iso14443A,
                ModulationType::Jewel,
                ModulationType::Iso14443B,
                ModulationType::Felica,
                ModulationType::Dep,
            ],
            (_, Mode::Target) => vec![
                ModulationType::Iso14443A,
                ModulationType::Felica,
                ModulationType::Dep,
            ],
        }
    }
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
    tx_bits: u8,
    timer_prescaler: u16,
    timeout_command_ms: i32,
    timeout_atr_ms: i32,
    timeout_communication_ms: i32,
    properties: PropertyState,
    current_target: Option<Target>,
}

impl Default for Pn53xCore {
    fn default() -> Self {
        Self {
            chip_type: Pn53xType::Unknown,
            firmware: None,
            power_mode: Pn53xPowerMode::LowVbat,
            last_command: None,
            last_status_byte: 0,
            tx_bits: 0,
            timer_prescaler: 0,
            timeout_command_ms: 500,
            timeout_atr_ms: 103,
            timeout_communication_ms: 52,
            properties: PropertyState::default(),
            current_target: None,
        }
    }
}

impl Pn53xCore {
    fn exchange_prepared_command<T: Pn53xTransport>(
        &mut self,
        transport: &mut T,
        command: u8,
        payload: &[u8],
        timeout_ms: i32,
    ) -> Result<Vec<u8>, Error> {
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

    fn ensure_ready<T: Pn53xTransport>(
        &mut self,
        profile: Pn53xProfile,
        transport: &mut T,
        timeout_ms: i32,
    ) -> Result<(), Error> {
        if self.power_mode == Pn53xPowerMode::Normal {
            return Ok(());
        }

        let previous_mode = self.power_mode;
        transport.wake_up()?;
        self.power_mode = Pn53xPowerMode::Normal;

        if previous_mode == Pn53xPowerMode::LowVbat {
            if let Some(mode) = profile.sam_mode_on_low_vbat {
                let payload = match mode {
                    Pn532SamMode::Normal => [mode as u8, 0x00],
                    Pn532SamMode::WiredCard => [mode as u8, 0x00],
                };
                let _ = self.exchange_prepared_command(
                    transport,
                    PN532_SAM_CONFIGURATION,
                    &payload,
                    timeout_ms,
                )?;
            }
        }

        Ok(())
    }

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

    pub(crate) fn current_target(&self) -> Option<&Target> {
        self.current_target.as_ref()
    }

    fn remember_target(&mut self, target: Target) {
        self.current_target = Some(target);
    }

    fn clear_target(&mut self) {
        self.current_target = None;
    }

    pub(crate) fn set_property_bool(
        &mut self,
        property: Property,
        enable: bool,
    ) -> Result<(), Error> {
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
        profile: Pn53xProfile,
        transport: &mut T,
        command: u8,
        payload: &[u8],
        timeout_ms: i32,
    ) -> Result<Vec<u8>, Error> {
        self.ensure_ready(profile, transport, timeout_ms)?;
        self.exchange_prepared_command(transport, command, payload, timeout_ms)
    }

    pub(crate) fn get_firmware_version<T: Pn53xTransport>(
        &mut self,
        profile: Pn53xProfile,
        transport: &mut T,
        timeout_ms: i32,
    ) -> Result<Pn53xFirmwareVersion, Error> {
        let payload = self.exchange_command(
            profile,
            transport,
            PN53X_GET_FIRMWARE_VERSION,
            &[],
            timeout_ms,
        )?;
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
    profile: Pn53xProfile,
    transport: T,
    core: Pn53xCore,
    last_error: i32,
}

impl<T: Pn53xTransport + Send + 'static> Pn53xDevice<T> {
    pub(crate) fn probe_with_profile(
        name: impl Into<String>,
        connstring: ConnectionString,
        profile: Pn53xProfile,
        mut transport: T,
        timeout_ms: i32,
    ) -> Result<Self, Error> {
        let mut core = Pn53xCore {
            power_mode: profile.initial_power_mode,
            ..Pn53xCore::default()
        };
        core.get_firmware_version(profile, &mut transport, timeout_ms)?;
        Ok(Self {
            name: name.into(),
            connstring,
            profile,
            transport,
            core,
            last_error: 0,
        })
    }

    pub(crate) fn probe_pn532(
        name: impl Into<String>,
        connstring: ConnectionString,
        transport: T,
        timeout_ms: i32,
    ) -> Result<Self, Error> {
        Self::probe_with_profile(
            name,
            connstring,
            Pn53xProfile::pn532("pn532"),
            transport,
            timeout_ms,
        )
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

    fn sam_configuration(&mut self, mode: Pn532SamMode, timeout_ms: i32) -> Result<i32, Error> {
        let payload = match mode {
            Pn532SamMode::Normal => [mode as u8, 0x00],
            Pn532SamMode::WiredCard => [mode as u8, 0x00],
        };
        let result = self
            .core
            .exchange_command(
                self.profile,
                &mut self.transport,
                PN532_SAM_CONFIGURATION,
                &payload,
                timeout_ms,
            )
            .map(|_| 0);
        self.remember(result)
    }

    fn exchange_raw(
        &mut self,
        command: u8,
        payload: &[u8],
        timeout_ms: i32,
    ) -> Result<Vec<u8>, Error> {
        let result = self.core.exchange_command(
            self.profile,
            &mut self.transport,
            command,
            payload,
            timeout_ms,
        );
        self.remember(result)
    }

    fn exchange_with_status(
        &mut self,
        operation: &'static str,
        command: u8,
        payload: &[u8],
        timeout_ms: i32,
    ) -> Result<Vec<u8>, Error> {
        let response = self.exchange_raw(command, payload, timeout_ms)?;
        let (status, data) = split_status_response(command, &response)?;
        self.core.last_status_byte = status;
        let mapped = pn53x_translate_status(status);
        if mapped < 0 {
            self.last_error = mapped;
            return Err(status_error(operation, mapped));
        }
        self.last_error = 0;
        Ok(data)
    }

    fn copy_into(
        operation: &'static str,
        source: &[u8],
        destination: &mut [u8],
    ) -> Result<usize, Error> {
        if source.len() > destination.len() {
            return Err(Error::DeviceOperationFailed {
                operation,
                code: NFC_EOVFLOW,
            });
        }
        destination[..source.len()].copy_from_slice(source);
        Ok(source.len())
    }

    fn read_register(&mut self, register: u16) -> Result<u8, Error> {
        let values = self.read_registers(&[register])?;
        values
            .into_iter()
            .next()
            .ok_or_else(|| status_error("read_register", NFC_EIO))
    }

    fn write_register(&mut self, register: u16, value: u8) -> Result<(), Error> {
        self.write_registers(&[(register, value)])
    }

    fn read_registers(&mut self, registers: &[u16]) -> Result<Vec<u8>, Error> {
        if registers.is_empty() {
            return Ok(Vec::new());
        }
        let mut payload = Vec::with_capacity(registers.len() * 2);
        for register in registers {
            payload.push((register >> 8) as u8);
            payload.push(*register as u8);
        }
        let response = self.exchange_raw(
            PN53X_READ_REGISTER,
            &payload,
            self.core.timeout_command_ms,
        )?;
        let values = if self.core.chip_type() == Pn53xType::Pn533 {
            let (status, data) = split_status_response(PN53X_READ_REGISTER, &response)?;
            self.core.last_status_byte = status;
            let mapped = pn53x_translate_status(status);
            if mapped < 0 {
                return self.remember(Err(status_error("read_register", mapped)));
            }
            data
        } else {
            response
        };
        if values.len() < registers.len() {
            return self.remember(Err(status_error("read_register", NFC_EIO)));
        }
        Ok(values[..registers.len()].to_vec())
    }

    fn write_registers(&mut self, writes: &[(u16, u8)]) -> Result<(), Error> {
        if writes.is_empty() {
            return Ok(());
        }
        let mut payload = Vec::with_capacity(writes.len() * 3);
        for (register, value) in writes {
            payload.push((register >> 8) as u8);
            payload.push(*register as u8);
            payload.push(*value);
        }
        let _ = self.exchange_raw(
            PN53X_WRITE_REGISTER,
            &payload,
            self.core.timeout_command_ms,
        )?;
        Ok(())
    }

    fn update_register_bits(&mut self, register: u16, mask: u8, value: u8) -> Result<(), Error> {
        let current = self.read_register(register)?;
        let next = (current & !mask) | (value & mask);
        if current != next {
            self.write_register(register, next)?;
        }
        Ok(())
    }

    fn set_tx_bits(&mut self, bits: u8) -> Result<(), Error> {
        let bits = bits & SYMBOL_TX_LAST_BITS;
        if self.core.tx_bits == bits {
            return Ok(());
        }
        self.update_register_bits(PN53X_REG_CIU_BIT_FRAMING, SYMBOL_TX_LAST_BITS, bits)?;
        self.core.tx_bits = bits;
        Ok(())
    }

    fn rx_last_bits(&mut self) -> Result<u8, Error> {
        Ok(self.read_register(PN53X_REG_CIU_CONTROL)? & SYMBOL_RX_LAST_BITS)
    }

    fn init_timer(&mut self, max_cycles: u32) -> Result<(), Error> {
        self.core.timer_prescaler = if max_cycles > 0xFFFF {
            (((max_cycles / 0xFFFF).saturating_sub(1)) / 2) as u16
        } else {
            0
        };
        self.write_registers(&[
            (
                PN53X_REG_CIU_TMODE,
                SYMBOL_TAUTO
                    | (((self.core.timer_prescaler >> 8) as u8) & SYMBOL_TPRESCALERHI),
            ),
            (
                PN53X_REG_CIU_TPRESCALER,
                (self.core.timer_prescaler as u8) & SYMBOL_TPRESCALERLO,
            ),
            (PN53X_REG_CIU_TRELOAD_VAL_HI, 0xff),
            (PN53X_REG_CIU_TRELOAD_VAL_LO, 0xff),
        ])
    }

    fn timer_cycles(&mut self, last_cmd_byte: u8) -> Result<u32, Error> {
        let values = self.read_registers(&[
            PN53X_REG_CIU_TCOUNTER_VAL_HI,
            PN53X_REG_CIU_TCOUNTER_VAL_LO,
        ])?;
        let counter = u16::from(values[0]) << 8 | u16::from(values[1]);
        if counter == 0 {
            return Ok(u32::MAX);
        }

        let mut cycles = u32::from(0xFFFFu16 - counter);
        cycles = cycles
            .saturating_mul(u32::from(self.core.timer_prescaler) * 2 + 1)
            .saturating_add(1);
        let rx_detection_correction = match self.core.chip_type() {
            Pn53xType::Pn531 => 2 * 128,
            _ => 5 * 128,
        };
        cycles = cycles.saturating_sub(rx_detection_correction);
        if even_parity_bit(last_cmd_byte) == 1 {
            cycles = cycles.saturating_add(64);
        }
        Ok(cycles.saturating_add(self.profile.timer_correction))
    }

    fn timed_send_fifo(&mut self, tx: &[u8], tx_last_bits: u8) -> Result<(), Error> {
        let mut writes = Vec::with_capacity((tx.len() + 3) * 2);
        writes.push((
            PN53X_REG_CIU_COMMAND,
            SYMBOL_COMMAND & SYMBOL_COMMAND_TRANSCEIVE,
        ));
        writes.push((PN53X_REG_CIU_FIFO_LEVEL, SYMBOL_FLUSH_BUFFER));
        for byte in tx {
            writes.push((PN53X_REG_CIU_FIFO_DATA, *byte));
        }
        writes.push((
            PN53X_REG_CIU_BIT_FRAMING,
            SYMBOL_START_SEND | (tx_last_bits & SYMBOL_TX_LAST_BITS),
        ));
        self.write_registers(&writes)?;
        self.core.tx_bits = tx_last_bits & SYMBOL_TX_LAST_BITS;
        Ok(())
    }

    fn timed_wait_fifo_level(&mut self) -> Result<u8, Error> {
        let attempts = usize::from(3u16.saturating_mul(self.core.timer_prescaler * 2 + 1)).max(1);
        let mut level = 0u8;
        for _ in 0..attempts {
            level = self.read_register(PN53X_REG_CIU_FIFO_LEVEL)?;
            if level & SYMBOL_FIFO_LEVEL != 0 {
                return Ok(level);
            }
        }
        Ok(level)
    }

    fn timed_receive_fifo(
        &mut self,
        rx: &mut [u8],
        read_last_bits: bool,
    ) -> Result<(usize, u8), Error> {
        let mut fifo_level = self.timed_wait_fifo_level()?;
        let mut total = 0usize;
        while fifo_level & SYMBOL_FIFO_LEVEL != 0 {
            let chunk_len = usize::from(fifo_level & SYMBOL_FIFO_LEVEL);
            let mut registers = vec![PN53X_REG_CIU_FIFO_DATA; chunk_len];
            registers.push(PN53X_REG_CIU_FIFO_LEVEL);
            let values = self.read_registers(&registers)?;
            if total + chunk_len > rx.len() {
                return Err(status_error("transceive_timed", NFC_EOVFLOW));
            }
            rx[total..total + chunk_len].copy_from_slice(&values[..chunk_len]);
            total += chunk_len;
            fifo_level = values[chunk_len];
        }
        let last_bits = if read_last_bits && total != 0 {
            self.rx_last_bits()?
        } else {
            0
        };
        Ok((total, last_bits))
    }

    fn transceive_bytes_timed_shared(
        &mut self,
        operation: &'static str,
        tx: &[u8],
        rx: &mut [u8],
    ) -> Result<(usize, u32), Error> {
        if !self.core.properties.handle_parity {
            return self.remember(Err(status_error(operation, NFC_EINVARG)));
        }
        if self.core.properties.easy_framing {
            return self.remember(Err(Error::UnsupportedOperation(operation)));
        }
        if tx.is_empty() {
            return self.remember(Err(status_error(operation, NFC_EINVARG)));
        }

        let txmode = if self.core.properties.handle_crc {
            Some(self.read_register(PN53X_REG_CIU_TX_MODE)?)
        } else {
            None
        };
        self.init_timer(0)?;
        self.timed_send_fifo(tx, 0)?;
        let (written, _) = self.timed_receive_fifo(rx, false)?;
        let last_cmd_byte = timer_last_command_byte(tx, txmode)?;
        let cycles = self.timer_cycles(last_cmd_byte)?;
        self.last_error = 0;
        Ok((written, cycles))
    }

    fn transceive_bits_timed_shared(
        &mut self,
        operation: &'static str,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), Error> {
        if self.core.properties.easy_framing {
            return self.remember(Err(Error::UnsupportedOperation(operation)));
        }
        if self.core.properties.handle_crc {
            return self.remember(Err(Error::UnsupportedOperation(operation)));
        }

        let (payload, payload_bits_len) = if self.core.properties.handle_parity {
            if tx_parity.is_some() || rx_parity.is_some() {
                return self.remember(Err(Error::UnsupportedOperation(operation)));
            }
            let byte_len = bits_to_bytes_len(tx_bits_len);
            if tx.len() < byte_len {
                return self.remember(Err(status_error(operation, NFC_EINVARG)));
            }
            (tx[..byte_len].to_vec(), tx_bits_len)
        } else if tx_bits_len == 0 {
            (Vec::new(), 0)
        } else {
            (
                pn53x_wrap_frame(tx, tx_bits_len, tx_parity)?,
                tx_bits_len + (tx_bits_len / 8),
            )
        };

        self.init_timer(0)?;
        self.timed_send_fifo(&payload, (payload_bits_len % 8) as u8)?;
        let mut raw_rx = vec![0u8; rx.len().max(1)];
        let (raw_len, last_bits) = self.timed_receive_fifo(&mut raw_rx, true)?;
        let response_bits_len = raw_frame_bits_len(raw_len, last_bits);
        let written = if self.core.properties.handle_parity {
            let byte_len = bits_to_bytes_len(response_bits_len);
            Self::copy_into(operation, &raw_rx[..byte_len], rx)?;
            response_bits_len
        } else {
            pn53x_unwrap_frame(&raw_rx[..raw_len], response_bits_len, rx, rx_parity)?
        };
        let last_cmd_byte = payload.last().copied().unwrap_or(0);
        let cycles = self.timer_cycles(last_cmd_byte)?;
        self.last_error = 0;
        Ok((written, cycles))
    }

    fn transceive_bits_shared(
        &mut self,
        operation: &'static str,
        command: u8,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
        timeout_ms: i32,
    ) -> Result<usize, Error> {
        let (payload, payload_bits_len) = if self.core.properties.handle_parity {
            if tx_parity.is_some() || rx_parity.is_some() {
                return self.remember(Err(Error::UnsupportedOperation(operation)));
            }
            let byte_len = bits_to_bytes_len(tx_bits_len);
            if tx.len() < byte_len {
                return self.remember(Err(status_error(operation, NFC_EINVARG)));
            }
            (tx[..byte_len].to_vec(), tx_bits_len)
        } else if tx_bits_len == 0 {
            (Vec::new(), 0)
        } else {
            (pn53x_wrap_frame(tx, tx_bits_len, tx_parity)?, tx_bits_len + (tx_bits_len / 8))
        };

        self.set_tx_bits((payload_bits_len % 8) as u8)?;
        let response = self.exchange_with_status(operation, command, &payload, timeout_ms)?;
        let response_bits_len = raw_frame_bits_len(response.len(), self.rx_last_bits()?);
        let result_bits = if self.core.properties.handle_parity {
            let byte_len = bits_to_bytes_len(response_bits_len);
            Self::copy_into(operation, &response[..byte_len], rx)?;
            response_bits_len
        } else {
            pn53x_unwrap_frame(&response, response_bits_len, rx, rx_parity)?
        };
        self.last_error = 0;
        Ok(result_bits)
    }

    fn with_temporary_bool_property<R>(
        &mut self,
        property: Property,
        value: bool,
        f: impl FnOnce(&mut Self) -> Result<R, Error>,
    ) -> Result<R, Error> {
        let previous = self.core.property_bool_state(property).unwrap_or(false);
        self.core.set_property_bool(property, value)?;
        let result = f(self);
        let restore = self.core.set_property_bool(property, previous);
        match (result, restore) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) | (Err(_), Err(error)) => Err(error),
        }
    }

    fn presence_transceive_bytes(
        &mut self,
        tx: &[u8],
        timeout_ms: i32,
        easy_framing: bool,
    ) -> Result<bool, Error> {
        self.with_temporary_bool_property(Property::EasyFraming, easy_framing, |device| {
            let mut rx = [0u8; PN53X_EXTENDED_FRAME_DATA_MAX_LEN];
            let len = device.transceive_bytes_driver(tx, &mut rx, timeout_ms)?;
            Ok(len > 0)
        })
    }

    fn presence_transceive_bits(&mut self, _timeout_ms: i32) -> Result<bool, Error> {
        self.with_temporary_bool_property(Property::HandleParity, false, |device| {
            let mut rx = [0u8; PN53X_EXTENDED_FRAME_DATA_MAX_LEN];
            let mut parity = [0u8; PN53X_EXTENDED_FRAME_DATA_MAX_LEN];
            let len = device.transceive_bits_driver(&[], 0, None, &mut rx, Some(&mut parity))?;
            Ok(len > 0)
        })
    }

    fn diagnose_card_presence(&mut self) -> Result<bool, Error> {
        const PN53X_DIAGNOSE: u8 = 0x00;
        let response = self.exchange_raw(PN53X_DIAGNOSE, &[0x06], 1000)?;
        let Some(&status) = response.first() else {
            return Err(status_error("target_is_present", NFC_EIO));
        };
        self.core.last_status_byte = status & 0x3f;
        let mapped = pn53x_translate_status(self.core.last_status_byte);
        if mapped < 0 {
            return Err(status_error("target_is_present", mapped));
        }
        Ok(true)
    }

    fn check_iso14443a_presence(&mut self, target: &Target) -> Result<bool, Error> {
        match &target.info {
            TargetInfo::Iso14443A { atqa, sak, uid, .. } if sak & SAK_ISO14443_4_COMPLIANT != 0 => {
                self.presence_transceive_bytes(&[0xb2], 300, false)
            }
            TargetInfo::Iso14443A { atqa, sak, .. }
                if *sak == 0x00 && *atqa == [0x00, 0x44] =>
            {
                self.presence_transceive_bytes(&[0x30, 0x00], 300, true)
            }
            TargetInfo::Iso14443A { sak, uid, .. } if *sak & SAK_MIFARE_CLASSIC_MASK != 0 => {
                let init_data = cascade_iso14443a_uid(uid);
                self.with_temporary_bool_property(Property::InfiniteSelect, false, |device| {
                    device
                        .select_passive_target_driver(target.modulation, &init_data)
                        .map(|found| found.is_some())
                })
            }
            _ => Err(status_error("target_is_present", NFC_EDEVNOTSUPP)),
        }
    }

    fn check_current_target_presence(&mut self, target: &Target) -> Result<bool, Error> {
        match target.modulation.modulation_type {
            ModulationType::Iso14443A => self.check_iso14443a_presence(target),
            ModulationType::Iso14443B => self.presence_transceive_bytes(&[0xba, 0x01], 300, false),
            ModulationType::Jewel => self.presence_transceive_bytes(&[0x78], -1, true),
            ModulationType::Felica => match &target.info {
                TargetInfo::Felica { id, .. } => {
                    let mut command = vec![0x0a, 0x04];
                    command.extend_from_slice(id);
                    self.presence_transceive_bytes(&command, 300, true)
                }
                _ => Err(status_error("target_is_present", NFC_EDEVNOTSUPP)),
            },
            ModulationType::Dep => self.diagnose_card_presence(),
            ModulationType::Barcode => self.presence_transceive_bits(300),
            _ => Err(status_error("target_is_present", NFC_EDEVNOTSUPP)),
        }
    }
}

fn command_uses_status_byte(command: u8) -> bool {
    matches!(
        command,
        PN53X_READ_REGISTER
            | PN53X_IN_DATA_EXCHANGE
            | PN53X_IN_COMMUNICATE_THRU
            | PN53X_IN_JUMP_FOR_DEP
            | PN53X_TG_GET_DATA
            | PN53X_TG_SET_DATA
            | PN53X_TG_GET_INITIATOR_COMMAND
            | PN53X_TG_RESPONSE_TO_INITIATOR
            | PN53X_IN_DESELECT
    )
}

fn split_status_response(command: u8, response: &[u8]) -> Result<(u8, Vec<u8>), Error> {
    if !command_uses_status_byte(command) {
        return Ok((0, response.to_vec()));
    }
    let Some((&status_flags, data)) = response.split_first() else {
        return Err(status_error("pn53x_status_response", NFC_EIO));
    };
    if status_flags & 0x80 != 0 {
        return Ok((PN53X_STATUS_NAD, data.to_vec()));
    }
    Ok((status_flags & 0x3f, data.to_vec()))
}

fn bits_to_bytes_len(bits_len: usize) -> usize {
    (bits_len / 8) + usize::from(bits_len % 8 != 0)
}

fn raw_frame_bits_len(bytes_len: usize, last_bits: u8) -> usize {
    if bytes_len == 0 {
        0
    } else if last_bits == 0 {
        bytes_len * 8
    } else {
        (bytes_len.saturating_sub(1) * 8) + usize::from(last_bits)
    }
}

fn mirror(byte: u8) -> u8 {
    byte.reverse_bits()
}

fn pn53x_wrap_frame(
    tx: &[u8],
    tx_bits_len: usize,
    tx_parity: Option<&[u8]>,
) -> Result<Vec<u8>, Error> {
    if tx_bits_len == 0 {
        return Ok(Vec::new());
    }
    let tx_bytes_len = bits_to_bytes_len(tx_bits_len);
    if tx.len() < tx_bytes_len {
        return Err(status_error("pn53x_wrap_frame", NFC_EINVARG));
    }
    if tx_bits_len < 9 {
        return Ok(vec![tx[0]]);
    }

    let parity = tx_parity.ok_or(Error::InvalidArgument("tx_parity"))?;
    let full_bytes = tx_bits_len / 8;
    if parity.len() < full_bytes {
        return Err(status_error("pn53x_wrap_frame", NFC_EINVARG));
    }

    let frame_bits_len = tx_bits_len + full_bytes;
    let frame_bytes_len = bits_to_bytes_len(frame_bits_len);
    let mut frame = vec![0u8; frame_bytes_len];
    let mut bits_left = tx_bits_len;
    let mut data_pos = 0usize;
    let mut frame_pos = 0usize;
    loop {
        let mut frame_byte = 0u8;
        for bit_pos in 0..8 {
            let data = mirror(tx[data_pos]);
            frame_byte |= data >> bit_pos;
            frame[frame_pos] = mirror(frame_byte);
            frame_byte = ((u16::from(data)) << (8 - bit_pos)) as u8;
            frame_byte |= (parity[data_pos] & 0x01) << (7 - bit_pos);
            frame_pos += 1;
            if frame_pos >= frame.len() {
                return Ok(frame);
            }
            frame[frame_pos] = mirror(frame_byte);
            data_pos += 1;
            if bits_left < 9 {
                return Ok(frame);
            }
            bits_left -= 8;
        }
        frame_pos += 1;
        if frame_pos >= frame.len() {
            return Ok(frame);
        }
    }
}

fn pn53x_unwrap_frame(
    frame: &[u8],
    frame_bits_len: usize,
    rx: &mut [u8],
    mut rx_parity: Option<&mut [u8]>,
) -> Result<usize, Error> {
    if frame_bits_len == 0 {
        return Ok(0);
    }
    let frame_bytes_len = bits_to_bytes_len(frame_bits_len);
    if frame.len() < frame_bytes_len {
        return Err(status_error("pn53x_unwrap_frame", NFC_EIO));
    }
    if frame_bits_len < 9 {
        if rx.is_empty() {
            return Err(status_error("pn53x_unwrap_frame", NFC_EOVFLOW));
        }
        rx[0] = frame[0];
        return Ok(frame_bits_len);
    }

    let rx_bits_len = frame_bits_len - (frame_bits_len / 9);
    let rx_bytes_len = bits_to_bytes_len(rx_bits_len);
    if rx.len() < rx_bytes_len {
        return Err(status_error("pn53x_unwrap_frame", NFC_EOVFLOW));
    }
    if let Some(parity) = rx_parity.as_ref()
        && parity.len() < rx_bits_len / 8
    {
        return Err(status_error("pn53x_unwrap_frame", NFC_EOVFLOW));
    }

    let mut bits_left = frame_bits_len;
    let mut data_pos = 0usize;
    let mut frame_pos = 0usize;
    loop {
        for bit_pos in 0..8 {
            let first = mirror(frame[frame_pos + data_pos]);
            let second = mirror(frame[frame_pos + data_pos + 1]);
            let mut data = ((u16::from(first)) << bit_pos) as u8;
            data |= (u16::from(second) >> (8 - bit_pos)) as u8;
            rx[data_pos] = mirror(data);
            if let Some(parity) = rx_parity.as_deref_mut() {
                parity[data_pos] = (second >> (7 - bit_pos)) & 0x01;
            }
            data_pos += 1;
            if bits_left <= 9 {
                return Ok(rx_bits_len);
            }
            bits_left -= 9;
        }
        frame_pos += 1;
    }
}

fn even_parity_bit(byte: u8) -> u8 {
    u8::from(byte.count_ones() % 2 == 0)
}

fn iso14443a_crc_append(data: &[u8]) -> [u8; 2] {
    let mut crc = 0x6363u16;
    for byte in data {
        let mut value = *byte ^ (crc as u8);
        value ^= value << 4;
        crc = (crc >> 8)
            ^ (u16::from(value) << 8)
            ^ (u16::from(value) << 3)
            ^ (u16::from(value) >> 4);
    }
    [crc as u8, (crc >> 8) as u8]
}

fn iso14443b_crc_append(data: &[u8]) -> [u8; 2] {
    let mut crc = 0xFFFFu16;
    for byte in data {
        let mut value = *byte ^ (crc as u8);
        value ^= value << 4;
        crc = (crc >> 8)
            ^ (u16::from(value) << 8)
            ^ (u16::from(value) << 3)
            ^ (u16::from(value) >> 4);
    }
    crc = !crc;
    [crc as u8, (crc >> 8) as u8]
}

fn timer_last_command_byte(tx: &[u8], txmode: Option<u8>) -> Result<u8, Error> {
    let Some(&last) = tx.last() else {
        return Err(status_error("pn53x_timer_last_byte", NFC_EINVARG));
    };
    let Some(txmode) = txmode else {
        return Ok(last);
    };
    if txmode & SYMBOL_TX_CRC_ENABLE == 0 {
        return Ok(last);
    }
    let crc = match txmode & SYMBOL_TX_FRAMING {
        0x00 => iso14443a_crc_append(tx),
        0x03 => iso14443b_crc_append(tx),
        _ => return Ok(last),
    };
    Ok(crc[1])
}

fn cascade_iso14443a_uid(uid: &[u8]) -> Vec<u8> {
    match uid.len() {
        4 => uid.to_vec(),
        7 => {
            let mut cascaded = Vec::with_capacity(8);
            cascaded.extend_from_slice(&[0x88, uid[0], uid[1], uid[2]]);
            cascaded.extend_from_slice(&uid[3..]);
            cascaded
        }
        10 => {
            let mut cascaded = Vec::with_capacity(12);
            cascaded.extend_from_slice(&[0x88, uid[0], uid[1], uid[2]]);
            cascaded.extend_from_slice(&[0x88, uid[3], uid[4], uid[5]]);
            cascaded.extend_from_slice(&uid[6..]);
            cascaded
        }
        _ => Vec::new(),
    }
}

fn pn53x_translate_status(status: u8) -> i32 {
    match status {
        0 => 0,
        PN53X_STATUS_TIMEOUT
        | PN53X_STATUS_CRC
        | PN53X_STATUS_PARITY
        | PN53X_STATUS_BITCOUNT
        | PN53X_STATUS_FRAMING
        | PN53X_STATUS_BITCOLL
        | PN53X_STATUS_RFPROTO
        | PN53X_STATUS_RFTIMEOUT
        | PN53X_STATUS_DEPUNKCMD
        | PN53X_STATUS_DEPINVSTATE
        | PN53X_STATUS_NAD
        | PN53X_STATUS_NFCID3
        | PN53X_STATUS_INVRXFRAM
        | PN53X_STATUS_BCC
        | PN53X_STATUS_CID => NFC_ERFTRANS,
        PN53X_STATUS_SMALLBUF
        | PN53X_STATUS_OVCURRENT
        | PN53X_STATUS_BUFOVF
        | PN53X_STATUS_OVHEAT
        | PN53X_STATUS_INBUFOVF => NFC_EIO,
        PN53X_STATUS_INVPARAM
        | PN53X_STATUS_OPNOTALL
        | PN53X_STATUS_CMD
        | PN53X_STATUS_SECNOTSUPP => NFC_EINVARG,
        PN53X_STATUS_TGREL | PN53X_STATUS_CDISCARDED => NFC_ETGRELEASED,
        PN53X_STATUS_MFAUTH => -30,
        _ => NFC_EIO,
    }
}

fn default_initiator_payload(modulation: Modulation) -> &'static [u8] {
    match modulation.modulation_type {
        ModulationType::Iso14443B => &[0x00],
        ModulationType::Felica => &[0x00, 0xff, 0xff, 0x01, 0x00],
        _ => &[],
    }
}

fn nm_to_pm(modulation: Modulation) -> Option<u8> {
    match (modulation.modulation_type, modulation.baud_rate) {
        (ModulationType::Iso14443A, _) => Some(0x00),
        (ModulationType::Felica, BaudRate::Br212) => Some(0x01),
        (ModulationType::Felica, BaudRate::Br424) => Some(0x02),
        (ModulationType::Iso14443B, BaudRate::Br106) => Some(0x03),
        (ModulationType::Jewel, _) => Some(0x04),
        _ => None,
    }
}

fn nm_to_ptt(modulation: Modulation) -> Option<u8> {
    match (modulation.modulation_type, modulation.baud_rate) {
        (ModulationType::Iso14443A, _) => Some(0x10),
        (ModulationType::Iso14443B, BaudRate::Br106) => Some(0x03),
        (ModulationType::Jewel, _) => Some(0x04),
        (ModulationType::Felica, BaudRate::Br212) => Some(0x11),
        (ModulationType::Felica, BaudRate::Br424) => Some(0x12),
        _ => None,
    }
}

fn ptt_to_modulation(value: u8) -> Modulation {
    match value {
        0x03 | 0x23 => Modulation {
            modulation_type: ModulationType::Iso14443B,
            baud_rate: BaudRate::Br106,
        },
        0x04 => Modulation {
            modulation_type: ModulationType::Jewel,
            baud_rate: BaudRate::Br106,
        },
        0x11 => Modulation {
            modulation_type: ModulationType::Felica,
            baud_rate: BaudRate::Br212,
        },
        0x12 => Modulation {
            modulation_type: ModulationType::Felica,
            baud_rate: BaudRate::Br424,
        },
        0x40 | 0x80 => Modulation {
            modulation_type: ModulationType::Dep,
            baud_rate: BaudRate::Br106,
        },
        0x41 | 0x81 => Modulation {
            modulation_type: ModulationType::Dep,
            baud_rate: BaudRate::Br212,
        },
        0x42 | 0x82 => Modulation {
            modulation_type: ModulationType::Dep,
            baud_rate: BaudRate::Br424,
        },
        _ => Modulation {
            modulation_type: ModulationType::Iso14443A,
            baud_rate: BaudRate::Br106,
        },
    }
}

fn process_cascade_uid(uid: &[u8]) -> Vec<u8> {
    match uid {
        [0x88, a, b, c, tail @ ..] if tail.len() == 4 => {
            let mut real = vec![*a, *b, *c];
            real.extend_from_slice(tail);
            real
        }
        [0x88, a, b, c, 0x88, d, e, f, tail @ ..] if tail.len() == 4 => {
            let mut real = vec![*a, *b, *c, *d, *e, *f];
            real.extend_from_slice(tail);
            real
        }
        value => value.to_vec(),
    }
}

fn decode_target_data(
    chip_type: Pn53xType,
    modulation: Modulation,
    raw: &[u8],
) -> Result<Target, Error> {
    let info = match modulation.modulation_type {
        ModulationType::Iso14443A => decode_iso14443a_target(chip_type, raw)?,
        ModulationType::Iso14443B => decode_iso14443b_target(raw)?,
        ModulationType::Felica => decode_felica_target(raw)?,
        ModulationType::Jewel => decode_jewel_target(raw)?,
        _ => return Err(Error::UnsupportedOperation("decode_target_data")),
    };
    Ok(Target { modulation, info })
}

fn decode_iso14443a_target(chip_type: Pn53xType, raw: &[u8]) -> Result<TargetInfo, Error> {
    if raw.len() < 5 {
        return Err(status_error("decode_iso14443a_target", NFC_EIO));
    }

    let mut offset = 1;
    let atqa = if chip_type == Pn53xType::Pn531 {
        let value = [raw[offset + 1], raw[offset]];
        offset += 2;
        value
    } else {
        let value = [raw[offset], raw[offset + 1]];
        offset += 2;
        value
    };
    let sak = raw[offset];
    offset += 1;
    let uid_len = raw[offset] as usize;
    offset += 1;
    if raw.len() < offset + uid_len {
        return Err(status_error("decode_iso14443a_target", NFC_EIO));
    }
    let uid = process_cascade_uid(&raw[offset..offset + uid_len]);
    offset += uid_len;

    let mut ats = Vec::new();
    if let Some(&ats_header) = raw.get(offset) {
        offset += 1;
        if ats_header > 1 {
            let ats_len = usize::from(ats_header - 1);
            if raw.len() < offset + ats_len {
                return Err(status_error("decode_iso14443a_target", NFC_EIO));
            }
            ats.extend_from_slice(&raw[offset..offset + ats_len]);
        }
    }

    Ok(TargetInfo::Iso14443A {
        atqa,
        sak,
        uid,
        ats,
    })
}

fn decode_iso14443b_target(raw: &[u8]) -> Result<TargetInfo, Error> {
    if raw.len() < 13 {
        return Err(status_error("decode_iso14443b_target", NFC_EIO));
    }
    let mut offset = 2;
    let mut pupi = [0u8; 4];
    pupi.copy_from_slice(&raw[offset..offset + 4]);
    offset += 4;
    let mut application_data = [0u8; 4];
    application_data.copy_from_slice(&raw[offset..offset + 4]);
    offset += 4;
    let mut protocol_info = [0u8; 3];
    protocol_info.copy_from_slice(&raw[offset..offset + 3]);
    offset += 3;
    let card_identifier = if raw.len() > offset + 1 && raw[offset] > 0 {
        raw[offset + 1]
    } else {
        0
    };
    Ok(TargetInfo::Iso14443B {
        pupi,
        application_data,
        protocol_info,
        card_identifier,
    })
}

fn decode_felica_target(raw: &[u8]) -> Result<TargetInfo, Error> {
    if raw.len() < 19 {
        return Err(status_error("decode_felica_target", NFC_EIO));
    }
    let len = raw[1] as usize;
    let response_code = raw[2];
    let mut id = [0u8; 8];
    id.copy_from_slice(&raw[3..11]);
    let mut pad = [0u8; 8];
    pad.copy_from_slice(&raw[11..19]);
    let mut system_code = [0u8; 2];
    if len > 18 && raw.len() >= 21 {
        system_code.copy_from_slice(&raw[19..21]);
    }
    Ok(TargetInfo::Felica {
        len,
        response_code,
        id,
        pad,
        system_code,
    })
}

fn decode_jewel_target(raw: &[u8]) -> Result<TargetInfo, Error> {
    if raw.len() < 7 {
        return Err(status_error("decode_jewel_target", NFC_EIO));
    }
    let mut sens_res = [0u8; 2];
    sens_res.copy_from_slice(&raw[1..3]);
    let mut id = [0u8; 4];
    id.copy_from_slice(&raw[3..7]);
    Ok(TargetInfo::Jewel { sens_res, id })
}

fn build_injump_for_dep_command(
    mode: DepMode,
    baud_rate: BaudRate,
    initiator: Option<&DepInfo>,
) -> Result<Vec<u8>, Error> {
    let (baud_code, passive_initiator) = match baud_rate {
        BaudRate::Br106 => (0x00, None),
        BaudRate::Br212 => (0x01, Some(&[0x00, 0xff, 0xff, 0x00, 0x0f][..])),
        BaudRate::Br424 => (0x02, Some(&[0x00, 0xff, 0xff, 0x00, 0x0f][..])),
        _ => return Err(Error::InvalidArgument("baud_rate")),
    };

    let mut payload = vec![
        if mode == DepMode::Active { 0x01 } else { 0x00 },
        baud_code,
        0x00,
    ];

    if mode == DepMode::Passive
        && let Some(passive) = passive_initiator
    {
        payload[2] |= 0x01;
        payload.extend_from_slice(passive);
    }

    if let Some(initiator) = initiator {
        payload[2] |= 0x02;
        payload.extend_from_slice(&initiator.nfcid3);
        if !initiator.general_bytes.is_empty() {
            payload[2] |= 0x04;
            payload.extend_from_slice(&initiator.general_bytes);
        }
    }

    Ok(payload)
}

fn parse_dep_target(
    payload: &[u8],
    mode: DepMode,
    baud_rate: BaudRate,
) -> Result<Option<Target>, Error> {
    if payload.is_empty() {
        return Err(status_error("parse_dep_target", NFC_EIO));
    }
    if payload[0] == 0 {
        return Ok(None);
    }
    if payload.len() < 16 {
        return Err(status_error("parse_dep_target", NFC_EIO));
    }
    let mut nfcid3 = [0u8; 10];
    nfcid3.copy_from_slice(&payload[1..11]);
    let general_bytes = if payload.len() > 16 {
        payload[16..].to_vec()
    } else {
        Vec::new()
    };
    Ok(Some(Target {
        modulation: Modulation {
            modulation_type: ModulationType::Dep,
            baud_rate,
        },
        info: TargetInfo::Dep(DepInfo {
            nfcid3,
            did: payload[11],
            bs: payload[12],
            br: payload[13],
            timeout: payload[14],
            pp: payload[15],
            general_bytes,
            mode,
        }),
    }))
}

fn is_iso14443_4_target(target: &Target) -> bool {
    matches!(
        target.info,
        TargetInfo::Iso14443A { sak, .. } if sak & SAK_ISO14443_4_COMPLIANT != 0
    )
}

fn build_target_init_command(
    chip_type: Pn53xType,
    properties: PropertyState,
    target: &Target,
) -> Result<Vec<u8>, Error> {
    let mut command = vec![0u8; 39 + 47 + 48];
    command[0] = PN53X_TG_INIT_AS_TARGET;
    let mut target_mode = PN53X_TARGET_MODE_NORMAL;
    let optional_bytes;

    match &target.info {
        TargetInfo::Iso14443A { atqa, sak, uid, .. } => {
            if uid.len() != 4 || uid[0] != 0x08 {
                return Err(Error::InvalidArgument("target.uid"));
            }
            target_mode |= PN53X_TARGET_MODE_PASSIVE_ONLY;
            if chip_type == Pn53xType::Pn532
                && properties.auto_iso14443_4
                && sak & SAK_ISO14443_4_COMPLIANT != 0
            {
                target_mode |= PN53X_TARGET_MODE_ISO14443_4_PICC_ONLY;
            }
            command[2] = atqa[1];
            command[3] = atqa[0];
            command[4] = uid[1];
            command[5] = uid[2];
            command[6] = uid[3];
            command[7] = *sak;
            command[36] = 0;
            optional_bytes = 2;
        }
        TargetInfo::Felica {
            id,
            pad,
            system_code,
            ..
        } => {
            target_mode |= PN53X_TARGET_MODE_PASSIVE_ONLY;
            command[8..16].copy_from_slice(id);
            command[16..24].copy_from_slice(pad);
            command[24..26].copy_from_slice(system_code);
            command[36] = 0;
            optional_bytes = 2;
        }
        TargetInfo::Dep(dep) => {
            target_mode |= PN53X_TARGET_MODE_DEP_ONLY;
            if dep.mode == DepMode::Passive {
                target_mode |= PN53X_TARGET_MODE_PASSIVE_ONLY;
            }
            command[2] = 0x08;
            command[3] = 0x00;
            command[4] = 0x12;
            command[5] = 0x34;
            command[6] = 0x56;
            command[7] = 0x40;
            command[8..16].copy_from_slice(&[0x01, 0xfe, 0x12, 0x34, 0x56, 0x78, 0x90, 0x12]);
            command[16..24].copy_from_slice(&[0xc0, 0xc1, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7]);
            command[24..26].copy_from_slice(&[0x0f, 0xab]);
            command[26..36].copy_from_slice(&dep.nfcid3);
            let gb_len = dep.general_bytes.len().min(47);
            command[36] = gb_len as u8;
            command[37..37 + gb_len].copy_from_slice(&dep.general_bytes[..gb_len]);
            command[37 + gb_len] = 0;
            optional_bytes = gb_len + 2;
        }
        _ => return Err(Error::UnsupportedOperation("target_init")),
    }

    command[1] = target_mode;
    command.truncate(36 + optional_bytes);
    Ok(command)
}

fn decode_activation_mode(mode: u8) -> (Modulation, DepMode) {
    let baud_rate = match mode & 0x70 {
        0x10 => BaudRate::Br212,
        0x20 => BaudRate::Br424,
        _ => BaudRate::Br106,
    };
    if mode & 0x04 != 0 {
        let dep_mode = if mode & 0x03 == 0x01 {
            DepMode::Active
        } else {
            DepMode::Passive
        };
        (
            Modulation {
                modulation_type: ModulationType::Dep,
                baud_rate,
            },
            dep_mode,
        )
    } else {
        let modulation_type = if mode & 0x03 == 0x02 {
            ModulationType::Felica
        } else {
            ModulationType::Iso14443A
        };
        (
            Modulation {
                modulation_type,
                baud_rate,
            },
            DepMode::Undefined,
        )
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
        Ok(self.profile.supported_modulations(mode))
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

    fn initiator_init_secure_element_driver(&mut self) -> Result<i32, Error> {
        let Some(mode) = self.profile.secure_element_mode else {
            return Err(Error::UnsupportedOperation("initiator_init_secure_element"));
        };
        self.sam_configuration(mode, self.core.timeout_command_ms)
    }

    fn select_passive_target_driver(
        &mut self,
        nm: Modulation,
        init_data: &[u8],
    ) -> Result<Option<Target>, Error> {
        let Some(pm) = nm_to_pm(nm) else {
            return self.remember(Err(Error::UnsupportedOperation("select_passive_target")));
        };
        let mut payload = Vec::with_capacity(init_data.len() + 3);
        payload.push(0x01);
        payload.push(pm);
        payload.extend_from_slice(init_data);

        let response = self.exchange_raw(
            PN53X_IN_LIST_PASSIVE_TARGET,
            &payload,
            self.core.timeout_command_ms,
        )?;
        let target = if response.first().copied().unwrap_or(0) == 0 {
            None
        } else {
            Some(decode_target_data(
                self.core.chip_type(),
                nm,
                &response[1..],
            )?)
        };
        if let Some(target) = &target {
            self.core.remember_target(target.clone());
        } else {
            self.core.clear_target();
        }
        self.last_error = 0;
        Ok(target)
    }

    fn poll_target_driver(
        &mut self,
        modulations: &[Modulation],
        poll_nr: u8,
        period: u8,
    ) -> Result<Option<Target>, Error> {
        if modulations.is_empty() {
            return self.remember(Err(Error::InvalidArgument("modulations")));
        }

        let delay = Duration::from_micros(u64::from(period) * 150_000);
        let mut remaining = if poll_nr == 0xff {
            usize::MAX
        } else {
            usize::from(poll_nr.max(1))
        };

        while remaining > 0 {
            for modulation in modulations {
                if let Some(target) = self.select_passive_target_driver(
                    *modulation,
                    default_initiator_payload(*modulation),
                )? {
                    self.last_error = 0;
                    return Ok(Some(target));
                }
            }
            if poll_nr != 0xff {
                remaining -= 1;
            }
            thread::sleep(delay);
        }

        self.last_error = 0;
        Ok(None)
    }

    fn select_dep_target_driver(
        &mut self,
        ndm: DepMode,
        nbr: BaudRate,
        initiator: Option<&DepInfo>,
        timeout: i32,
    ) -> Result<Option<Target>, Error> {
        let payload = build_injump_for_dep_command(ndm, nbr, initiator)?;
        let timeout = if timeout >= 0 {
            timeout
        } else {
            self.core.timeout_command_ms
        };
        let response = self.exchange_with_status(
            "select_dep_target",
            PN53X_IN_JUMP_FOR_DEP,
            &payload,
            timeout,
        )?;
        let target = parse_dep_target(&response, ndm, nbr)?;
        if let Some(target) = &target {
            self.core.remember_target(target.clone());
        } else {
            self.core.clear_target();
        }
        self.last_error = 0;
        Ok(target)
    }

    fn deselect_target_driver(&mut self) -> Result<(), Error> {
        let _ = self.exchange_with_status(
            "deselect_target",
            PN53X_IN_DESELECT,
            &[0x00],
            self.core.timeout_command_ms,
        )?;
        self.core.clear_target();
        self.last_error = 0;
        Ok(())
    }

    fn target_is_present_driver(&mut self, target: Option<&Target>) -> Result<bool, Error> {
        let Some(current) = self.core.current_target().cloned() else {
            return self.remember(Err(status_error("target_is_present", NFC_EINVARG)));
        };
        if target.is_some_and(|target| *target != current) {
            self.core.clear_target();
            return self.remember(Err(status_error("target_is_present", NFC_ETGRELEASED)));
        }
        match self.check_current_target_presence(&current) {
            Ok(found) => {
                if !found {
                    self.core.clear_target();
                }
                self.last_error = 0;
                Ok(found)
            }
            Err(error) => {
                let code = status_code(&error);
                if matches!(code, NFC_ETGRELEASED | NFC_ETIMEOUT) {
                    self.core.clear_target();
                }
                self.remember(Err(error))
            }
        }
    }

    fn target_init_driver(
        &mut self,
        target: &mut Target,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error> {
        let command =
            build_target_init_command(self.core.chip_type(), self.core.properties, target)?;
        let timeout = if timeout >= 0 {
            timeout
        } else {
            self.core.timeout_command_ms
        };
        let response = self.exchange_raw(PN53X_TG_INIT_AS_TARGET, &command[1..], timeout)?;
        let Some((&activation_mode, payload)) = response.split_first() else {
            return self.remember(Err(status_error("target_init", NFC_EIO)));
        };
        let (modulation, dep_mode) = decode_activation_mode(activation_mode);
        target.modulation = modulation;
        if let TargetInfo::Dep(info) = &mut target.info {
            info.mode = dep_mode;
        }
        let written = Self::copy_into("target_init", payload, rx)?;
        self.core.remember_target(target.clone());
        self.last_error = 0;
        Ok(written)
    }

    fn transceive_bytes_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error> {
        let timeout = if timeout >= 0 {
            timeout
        } else {
            self.core.timeout_communication_ms
        };
        self.set_tx_bits(0)?;
        let response = if self.core.properties.easy_framing {
            let mut payload = Vec::with_capacity(tx.len() + 1);
            payload.push(0x01);
            payload.extend_from_slice(tx);
            self.exchange_with_status(
                "transceive_bytes",
                PN53X_IN_DATA_EXCHANGE,
                &payload,
                timeout,
            )?
        } else {
            self.exchange_with_status("transceive_bytes", PN53X_IN_COMMUNICATE_THRU, tx, timeout)?
        };
        let written = Self::copy_into("transceive_bytes", &response, rx)?;
        self.last_error = 0;
        Ok(written)
    }

    fn transceive_bits_driver(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        self.transceive_bits_shared(
            "transceive_bits",
            PN53X_IN_COMMUNICATE_THRU,
            tx,
            tx_bits_len,
            tx_parity,
            rx,
            rx_parity,
            self.core.timeout_communication_ms,
        )
    }

    fn transceive_bytes_timed_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
    ) -> Result<(usize, u32), Error> {
        self.transceive_bytes_timed_shared("transceive_bytes_timed", tx, rx)
    }

    fn transceive_bits_timed_driver(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), Error> {
        self.transceive_bits_timed_shared(
            "transceive_bits_timed",
            tx,
            tx_bits_len,
            tx_parity,
            rx,
            rx_parity,
        )
    }

    fn target_send_bytes_driver(&mut self, tx: &[u8], timeout: i32) -> Result<usize, Error> {
        let timeout = if timeout >= 0 {
            timeout
        } else {
            self.core.timeout_communication_ms
        };
        self.set_tx_bits(0)?;
        let command = match self.core.current_target() {
            Some(target) if self.core.properties.easy_framing => {
                match target.modulation.modulation_type {
                    ModulationType::Dep => PN53X_TG_SET_DATA,
                    ModulationType::Iso14443A
                        if self.core.chip_type() == Pn53xType::Pn532
                            && self.core.properties.auto_iso14443_4
                            && is_iso14443_4_target(target) =>
                    {
                        PN53X_TG_SET_DATA
                    }
                    _ => PN53X_TG_RESPONSE_TO_INITIATOR,
                }
            }
            _ => PN53X_TG_RESPONSE_TO_INITIATOR,
        };
        let _ = self.exchange_with_status("target_send_bytes", command, tx, timeout)?;
        self.last_error = 0;
        Ok(tx.len())
    }

    fn target_receive_bytes_driver(&mut self, rx: &mut [u8], timeout: i32) -> Result<usize, Error> {
        let timeout = if timeout >= 0 {
            timeout
        } else {
            self.core.timeout_communication_ms
        };
        let command = match self.core.current_target() {
            Some(target) if self.core.properties.easy_framing => {
                match target.modulation.modulation_type {
                    ModulationType::Dep => PN53X_TG_GET_DATA,
                    ModulationType::Iso14443A
                        if self.core.chip_type() == Pn53xType::Pn532
                            && self.core.properties.auto_iso14443_4
                            && is_iso14443_4_target(target) =>
                    {
                        PN53X_TG_GET_DATA
                    }
                    _ => PN53X_TG_GET_INITIATOR_COMMAND,
                }
            }
            _ => PN53X_TG_GET_INITIATOR_COMMAND,
        };
        let response = self.exchange_with_status("target_receive_bytes", command, &[], timeout)?;
        let written = Self::copy_into("target_receive_bytes", &response, rx)?;
        self.last_error = 0;
        Ok(written)
    }

    fn target_send_bits_driver(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
    ) -> Result<usize, Error> {
        let mut sink = [];
        let _ = self.transceive_bits_shared(
            "target_send_bits",
            PN53X_TG_RESPONSE_TO_INITIATOR,
            tx,
            tx_bits_len,
            tx_parity,
            &mut sink,
            None,
            self.core.timeout_communication_ms,
        )?;
        self.last_error = 0;
        Ok(tx_bits_len)
    }

    fn target_receive_bits_driver(
        &mut self,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        self.transceive_bits_shared(
            "target_receive_bits",
            PN53X_TG_GET_INITIATOR_COMMAND,
            &[],
            0,
            None,
            rx,
            rx_parity,
            self.core.timeout_communication_ms,
        )
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

pub(crate) fn build_response_frame(command: u8, payload: &[u8]) -> Result<Vec<u8>, Error> {
    if payload.len() > PN53X_EXTENDED_FRAME_DATA_MAX_LEN {
        return Err(Error::BufferTooSmall {
            needed: payload.len(),
            available: PN53X_EXTENDED_FRAME_DATA_MAX_LEN,
        });
    }

    let mut body = Vec::with_capacity(payload.len() + 2);
    body.push(PN53X_TO_HOST_TFI);
    body.push(command.wrapping_add(1));
    body.extend_from_slice(payload);

    let mut frame = Vec::with_capacity(body.len() + PN53X_EXTENDED_FRAME_OVERHEAD);
    if body.len() <= 0xfe {
        let len = body.len() as u8;
        frame.extend_from_slice(&[0x00, 0x00, 0xff, len, (!len).wrapping_add(1)]);
    } else {
        let high = (body.len() >> 8) as u8;
        let low = (body.len() & 0xff) as u8;
        frame.extend_from_slice(&[
            0x00,
            0x00,
            0xff,
            0xff,
            0xff,
            high,
            low,
            (0u8).wrapping_sub(high.wrapping_add(low)),
        ]);
    }
    frame.extend_from_slice(&body);
    let dcs = body
        .iter()
        .fold(0u8, |sum, byte| sum.wrapping_add(*byte))
        .wrapping_neg();
    frame.push(dcs);
    frame.push(0x00);
    Ok(frame)
}

pub(crate) fn command_from_host_frame(frame: &[u8]) -> Result<u8, Error> {
    Ok(payload_from_host_frame(frame)?[0])
}

pub(crate) fn payload_from_host_frame(frame: &[u8]) -> Result<Vec<u8>, Error> {
    if frame.len() < 8 || !frame.starts_with(&[0x00, 0x00, 0xff]) {
        return Err(status_error("pn53x_command_from_host_frame", NFC_EIO));
    }

    let (body_offset, body_len) = if frame[3] == 0xff && frame[4] == 0xff {
        if frame.len() < 10 {
            return Err(status_error("pn53x_command_from_host_frame", NFC_EIO));
        }
        let length = ((frame[5] as usize) << 8) | frame[6] as usize;
        if frame[5].wrapping_add(frame[6]).wrapping_add(frame[7]) != 0 {
            return Err(status_error("pn53x_command_from_host_frame", NFC_EIO));
        }
        (8, length)
    } else {
        if frame[3].wrapping_add(frame[4]) != 0 {
            return Err(status_error("pn53x_command_from_host_frame", NFC_EIO));
        }
        (5, frame[3] as usize)
    };

    let trailer_offset = body_offset + body_len;
    if frame.len() < trailer_offset + 2 || body_len < 2 || frame[body_offset] != HOST_TO_PN53X_TFI {
        return Err(status_error("pn53x_command_from_host_frame", NFC_EIO));
    }

    let body = &frame[body_offset..trailer_offset];
    let expected_dcs = body
        .iter()
        .fold(0u8, |sum, byte| sum.wrapping_add(*byte))
        .wrapping_neg();
    if frame[trailer_offset] != expected_dcs || frame[trailer_offset + 1] != 0x00 {
        return Err(status_error("pn53x_command_from_host_frame", NFC_EIO));
    }

    Ok(body[1..].to_vec())
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

    fn queue_probe_responses(transport: &mut FakeTransport) {
        transport.received.push_back(PN53X_ACK_FRAME.to_vec());
        transport
            .received
            .push_back(response_frame(PN532_SAM_CONFIGURATION, &[]));
        transport.received.push_back(PN53X_ACK_FRAME.to_vec());
        transport
            .received
            .push_back(response_frame(0x02, &[0x32, 0x01, 0x06, 0x07]));
    }

    fn queue_command_response(transport: &mut FakeTransport, command: u8, payload: &[u8]) {
        transport.received.push_back(PN53X_ACK_FRAME.to_vec());
        transport.received.push_back(response_frame(command, payload));
    }

    fn probed_device() -> Pn53xDevice<FakeTransport> {
        let mut transport = FakeTransport::default();
        queue_probe_responses(&mut transport);
        let connstring = ConnectionString::new("pn532_uart:/dev/null:115200").unwrap();
        Pn53xDevice::probe_with_profile(
            "PN532",
            connstring,
            Pn53xProfile::pn532("pn532_uart"),
            transport,
            25,
        )
        .unwrap()
    }

    #[test]
    fn build_frame_supports_standard_frames() {
        let frame = build_frame(&[0x02, 0x03, 0x04]).unwrap();
        assert_eq!(
            frame,
            vec![
                0x00, 0x00, 0xff, 0x04, 0xfc, 0xD4, 0x02, 0x03, 0x04, 0x23, 0x00
            ]
        );
    }

    #[test]
    fn build_frame_supports_extended_frames() {
        let payload = vec![0xAB; 255];
        let frame = build_frame(&payload).unwrap();
        assert_eq!(
            &frame[..9],
            &[0x00, 0x00, 0xff, 0xff, 0xff, 0x01, 0x00, 0xff, 0xD4]
        );
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
        queue_probe_responses(&mut transport);

        let mut core = Pn53xCore::default();
        let payload = core
            .get_firmware_version(Pn53xProfile::pn532("pn532_uart"), &mut transport, 25)
            .unwrap();

        assert_eq!(transport.wake_up_calls, 1);
        assert_eq!(transport.sent.len(), 2);
        assert_eq!(payload.ic, 0x32);
        assert_eq!(core.chip_type(), Pn53xType::Pn532);
        assert_eq!(core.last_command(), Some(0x02));
        assert_eq!(core.power_mode(), Pn53xPowerMode::Normal);
    }

    #[test]
    fn probe_builds_pure_rust_device_and_reports_information() {
        let mut transport = FakeTransport::default();
        queue_probe_responses(&mut transport);

        let connstring = ConnectionString::new("pn532_uart:/dev/ttyUSB0:115200").unwrap();
        let mut device = Pn53xDevice::probe_with_profile(
            "PN532",
            connstring,
            Pn53xProfile::pn532("pn532_uart"),
            transport,
            25,
        )
        .unwrap();

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
        queue_probe_responses(&mut transport);

        let connstring = ConnectionString::new("pn532_uart:/dev/null:115200").unwrap();
        let mut device = Pn53xDevice::probe_with_profile(
            "PN532",
            connstring,
            Pn53xProfile::pn532("pn532_uart"),
            transport,
            25,
        )
        .unwrap();

        assert_eq!(
            device.property_bool_state(Property::EasyFraming),
            Some(true)
        );
        device
            .set_property_bool(Property::EasyFraming, false)
            .unwrap();
        device
            .set_property_int(Property::TimeoutCommand, 900)
            .unwrap();
        device.initiator_init().unwrap();

        assert_eq!(
            device.property_bool_state(Property::EasyFraming),
            Some(false)
        );
        assert_eq!(
            device.property_bool_state(Property::InfiniteSelect),
            Some(true)
        );
        assert_eq!(
            device.property_bool_state(Property::ForceSpeed106),
            Some(true)
        );
        assert_eq!(device.last_error(), 0);
    }

    #[test]
    fn abort_command_delegates_to_transport() {
        let mut transport = FakeTransport::default();
        queue_probe_responses(&mut transport);

        let connstring = ConnectionString::new("pn532_uart:/dev/null:115200").unwrap();
        let mut device = Pn53xDevice::probe_with_profile(
            "PN532",
            connstring,
            Pn53xProfile::pn532("pn532_uart"),
            transport,
            25,
        )
        .unwrap();
        device.abort_command().unwrap();

        assert_eq!(device.transport.abort_calls, 1);
    }

    #[test]
    fn transport_timeout_is_preserved_as_device_error() {
        let mut transport = FakeTransport::default();
        transport.received.push_back(PN53X_ACK_FRAME.to_vec());

        let mut core = Pn53xCore::default();
        let error = core
            .get_firmware_version(Pn53xProfile::pn532("pn532_uart"), &mut transport, 25)
            .unwrap_err();

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

    #[test]
    fn select_passive_target_decodes_iso14443a_and_tracks_current_target() {
        let mut device = probed_device();
        device
            .transport
            .received
            .push_back(PN53X_ACK_FRAME.to_vec());
        device.transport.received.push_back(response_frame(
            PN53X_IN_LIST_PASSIVE_TARGET,
            &[
                0x01, 0x01, 0x04, 0x00, 0x08, 0x04, 0xde, 0xad, 0xbe, 0xef, 0x05, 0x75, 0x77, 0x81,
                0x02,
            ],
        ));

        let target = device
            .select_passive_target(
                Modulation {
                    modulation_type: ModulationType::Iso14443A,
                    baud_rate: BaudRate::Br106,
                },
                None,
            )
            .unwrap()
            .unwrap();

        assert_eq!(
            target.info,
            TargetInfo::Iso14443A {
                atqa: [0x04, 0x00],
                sak: 0x08,
                uid: vec![0xde, 0xad, 0xbe, 0xef],
                ats: vec![0x75, 0x77, 0x81, 0x02],
            }
        );
        assert_eq!(device.core.current_target(), Some(&target));
    }

    #[test]
    fn select_dep_target_and_deselect_share_runtime_logic() {
        let mut device = probed_device();
        queue_command_response(
            &mut device.transport,
            PN53X_IN_JUMP_FOR_DEP,
            &[
                0x00, 0x01, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x22, 0x33,
                0x44, 0x55, 0x66, 0xaa, 0xbb,
            ],
        );
        queue_command_response(&mut device.transport, 0x00, &[0x00]);
        queue_command_response(&mut device.transport, PN53X_IN_DESELECT, &[0x00]);

        let target = device
            .select_dep_target(DepMode::Passive, BaudRate::Br106, None, 250)
            .unwrap()
            .unwrap();
        assert_eq!(
            target.info,
            TargetInfo::Dep(DepInfo {
                nfcid3: [0x11; 10],
                did: 0x22,
                bs: 0x33,
                br: 0x44,
                timeout: 0x55,
                pp: 0x66,
                general_bytes: vec![0xaa, 0xbb],
                mode: DepMode::Passive,
            })
        );
        assert!(device.target_is_present(Some(&target)).unwrap());

        device.deselect_target().unwrap();
        assert!(device.core.current_target().is_none());
    }

    #[test]
    fn transceive_bytes_and_timed_variant_use_shared_timer_register_flow() {
        let mut device = probed_device();
        device
            .transport
            .received
            .push_back(PN53X_ACK_FRAME.to_vec());
        device
            .transport
            .received
            .push_back(response_frame(PN53X_IN_DATA_EXCHANGE, &[0x00, 0x90, 0x00]));

        let mut rx = [0u8; 8];
        let written = device
            .transceive_bytes(&[0x30, 0x04], &mut rx, 250)
            .unwrap();
        assert_eq!(written, 2);
        assert_eq!(&rx[..written], &[0x90, 0x00]);

        device
            .set_property_bool(Property::EasyFraming, false)
            .unwrap();
        device
            .set_property_bool(Property::HandleCrc, false)
            .unwrap();
        queue_command_response(&mut device.transport, PN53X_WRITE_REGISTER, &[]);
        queue_command_response(&mut device.transport, PN53X_WRITE_REGISTER, &[]);
        queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0x01]);
        queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0xaa, 0x00]);
        queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0xf0, 0x00]);

        let (timed_written, elapsed) = device.transceive_bytes_timed(&[0x50], &mut rx).unwrap();
        assert_eq!(timed_written, 1);
        assert_eq!(&rx[..timed_written], &[0xaa]);
        assert_eq!(elapsed, 3568);
    }

    #[test]
    fn target_init_and_target_byte_io_are_shared() {
        let mut device = probed_device();
        device
            .transport
            .received
            .push_back(PN53X_ACK_FRAME.to_vec());
        device
            .transport
            .received
            .push_back(response_frame(PN53X_TG_INIT_AS_TARGET, &[0x04, 0xca, 0xfe]));
        device
            .transport
            .received
            .push_back(PN53X_ACK_FRAME.to_vec());
        device
            .transport
            .received
            .push_back(response_frame(PN53X_TG_SET_DATA, &[0x00]));
        device
            .transport
            .received
            .push_back(PN53X_ACK_FRAME.to_vec());
        device
            .transport
            .received
            .push_back(response_frame(PN53X_TG_GET_DATA, &[0x00, 0xbe, 0xef]));

        let mut target = Target {
            modulation: Modulation {
                modulation_type: ModulationType::Dep,
                baud_rate: BaudRate::Undefined,
            },
            info: TargetInfo::Dep(DepInfo {
                nfcid3: [0x22; 10],
                did: 0x01,
                bs: 0x02,
                br: 0x03,
                timeout: 0x04,
                pp: 0x05,
                general_bytes: vec![0xaa],
                mode: DepMode::Passive,
            }),
        };
        let mut rx = [0u8; 8];
        let init_len = device.target_init(&mut target, &mut rx, 250).unwrap();
        assert_eq!(init_len, 2);
        assert_eq!(&rx[..init_len], &[0xca, 0xfe]);
        assert_eq!(target.modulation.baud_rate, BaudRate::Br106);

        assert_eq!(device.target_send_bytes(&[0x90], 250).unwrap(), 1);
        let recv_len = device.target_receive_bytes(&mut rx, 250).unwrap();
        assert_eq!(recv_len, 2);
        assert_eq!(&rx[..recv_len], &[0xbe, 0xef]);
    }

    #[test]
    fn wrap_and_unwrap_frame_preserve_parity_bits() {
        let wrapped = pn53x_wrap_frame(&[0x93, 0x20], 16, Some(&[1, 0])).unwrap();
        let mut rx = [0u8; 8];
        let mut parity = [0u8; 8];
        let bits = pn53x_unwrap_frame(&wrapped, 18, &mut rx, Some(&mut parity)).unwrap();

        assert_eq!(bits, 16);
        assert_eq!(&rx[..2], &[0x93, 0x20]);
        assert_eq!(&parity[..2], &[1, 0]);
    }

    #[test]
    fn transceive_bits_supports_short_frames_with_register_backed_last_bits() {
        let mut device = probed_device();
        queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[SYMBOL_TX_CRC_ENABLE]);
        queue_command_response(
            &mut device.transport,
            PN53X_WRITE_REGISTER,
            &[],
        );
        queue_command_response(
            &mut device.transport,
            PN53X_IN_COMMUNICATE_THRU,
            &[0x00, 0x04, 0x00],
        );
        queue_command_response(
            &mut device.transport,
            PN53X_READ_REGISTER,
            &[SYMBOL_TX_CRC_ENABLE],
        );
        let mut rx = [0u8; 8];
        let bits = device
            .transceive_bits(&[0x26], 7, None, &mut rx, None)
            .unwrap();
        assert_eq!(bits, 16);
        assert_eq!(&rx[..2], &[0x04, 0x00]);
    }

    #[test]
    fn target_receive_bits_unwraps_raw_frame_and_parity() {
        let mut device = probed_device();
        device
            .set_property_bool(Property::HandleParity, false)
            .unwrap();
        let wrapped = pn53x_wrap_frame(&[0x93, 0x20], 16, Some(&[1, 0])).unwrap();
        let mut payload = Vec::with_capacity(wrapped.len() + 1);
        payload.push(0x00);
        payload.extend_from_slice(&wrapped);
        queue_command_response(
            &mut device.transport,
            PN53X_TG_GET_INITIATOR_COMMAND,
            &payload,
        );
        queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0x02]);

        let mut rx = [0u8; 8];
        let mut parity = [0u8; 8];
        let bits = device
            .target_receive_bits(&mut rx, Some(&mut parity))
            .unwrap();
        assert_eq!(bits, 16);
        assert_eq!(&rx[..2], &[0x93, 0x20]);
        assert_eq!(&parity[..2], &[1, 0]);
    }

    #[test]
    fn transceive_bits_timed_uses_shared_register_timer_flow() {
        let mut device = probed_device();
        device
            .set_property_bool(Property::EasyFraming, false)
            .unwrap();
        device
            .set_property_bool(Property::HandleParity, false)
            .unwrap();
        device
            .set_property_bool(Property::HandleCrc, false)
            .unwrap();
        let wrapped = pn53x_wrap_frame(&[0x93, 0x20], 16, Some(&[1, 0])).unwrap();
        queue_command_response(&mut device.transport, PN53X_WRITE_REGISTER, &[]);
        queue_command_response(&mut device.transport, PN53X_WRITE_REGISTER, &[]);
        queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[wrapped.len() as u8]);
        let mut fifo_payload = wrapped.clone();
        fifo_payload.push(0x00);
        queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &fifo_payload);
        queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0x02]);
        queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0xf0, 0x00]);

        let mut rx = [0u8; 8];
        let mut parity = [0u8; 8];
        let (bits, elapsed) = device
            .transceive_bits_timed(&[0x26], 7, None, &mut rx, Some(&mut parity))
            .unwrap();
        assert_eq!(bits, 16);
        assert_eq!(&rx[..2], &[0x93, 0x20]);
        assert_eq!(&parity[..2], &[1, 0]);
        assert_eq!(elapsed, 3504);
    }

    #[test]
    fn target_send_bits_wraps_non_byte_aligned_frames() {
        let mut device = probed_device();
        device
            .set_property_bool(Property::HandleParity, false)
            .unwrap();
        queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0x00]);
        queue_command_response(&mut device.transport, PN53X_WRITE_REGISTER, &[]);
        queue_command_response(&mut device.transport, PN53X_TG_RESPONSE_TO_INITIATOR, &[0x00]);
        queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0x00]);

        let sent = device.target_send_bits(&[0x93, 0x20], 16, Some(&[1, 0])).unwrap();
        assert_eq!(sent, 16);
    }

    #[test]
    fn timed_bytes_reads_tx_mode_before_register_timed_exchange() {
        let mut device = probed_device();
        device
            .set_property_bool(Property::EasyFraming, false)
            .unwrap();
        let sent_before = device.transport.sent.len();
        queue_command_response(
            &mut device.transport,
            PN53X_READ_REGISTER,
            &[SYMBOL_TX_CRC_ENABLE],
        );
        queue_command_response(&mut device.transport, PN53X_WRITE_REGISTER, &[]);
        queue_command_response(&mut device.transport, PN53X_WRITE_REGISTER, &[]);
        queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0x02]);
        queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0x90, 0x00, 0x00]);
        queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0xf0, 0x00]);

        let mut rx = [0u8; 4];
        let (written, elapsed) = device.transceive_bytes_timed(&[0x00], &mut rx).unwrap();
        assert_eq!(written, 2);
        assert_eq!(&rx[..2], &[0x90, 0x00]);
        assert_eq!(elapsed, 3504);
        assert_eq!(device.transport.sent[sent_before][6], PN53X_READ_REGISTER);
        assert_eq!(
            &device.transport.sent[sent_before][7..9],
            &[(PN53X_REG_CIU_TX_MODE >> 8) as u8, PN53X_REG_CIU_TX_MODE as u8]
        );
    }

    #[test]
    fn target_is_present_for_dep_uses_shared_diagnose_path() {
        let mut device = probed_device();
        let target = Target {
            modulation: Modulation {
                modulation_type: ModulationType::Dep,
                baud_rate: BaudRate::Br106,
            },
            info: TargetInfo::Dep(DepInfo {
                nfcid3: [0x11; 10],
                did: 0x22,
                bs: 0x33,
                br: 0x44,
                timeout: 0x55,
                pp: 0x66,
                general_bytes: vec![0xaa, 0xbb],
                mode: DepMode::Passive,
            }),
        };
        device.core.remember_target(target.clone());
        queue_command_response(&mut device.transport, 0x00, &[0x00]);

        assert!(device.target_is_present(Some(&target)).unwrap());
    }

    #[test]
    fn target_is_present_for_mifare_classic_reselects_saved_uid() {
        let mut device = probed_device();
        let target = Target {
            modulation: Modulation {
                modulation_type: ModulationType::Iso14443A,
                baud_rate: BaudRate::Br106,
            },
            info: TargetInfo::Iso14443A {
                atqa: [0x00, 0x04],
                sak: 0x08,
                uid: vec![0xde, 0xad, 0xbe, 0xef],
                ats: Vec::new(),
            },
        };
        device.core.remember_target(target.clone());
        queue_command_response(
            &mut device.transport,
            PN53X_IN_LIST_PASSIVE_TARGET,
            &[0x01, 0x01, 0x04, 0x00, 0x08, 0x04, 0xde, 0xad, 0xbe, 0xef],
        );

        assert!(device.target_is_present(Some(&target)).unwrap());
    }
}
