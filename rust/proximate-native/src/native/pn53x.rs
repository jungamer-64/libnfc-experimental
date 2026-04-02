#![allow(dead_code)]

use proximate_driver::{
    BaudRate, ConnectionString, DepInfo, DepMode, DeviceCaps, DeviceMeta, Error, InfoBackend,
    InitiatorBackend, Mode, Modulation, ModulationType, Pn53xBackend, Property, PropertyBackend,
    Target, TargetBackend, TargetInfo,
};
use std::thread;
use std::time::Duration;

mod crc_bits;
mod frame;
mod target_decode;
mod transport;

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

use self::crc_bits::{
    bits_to_bytes_len, even_parity_bit, pn53x_unwrap_frame, pn53x_wrap_frame, raw_frame_bits_len,
    timer_last_command_byte,
};
#[allow(unused_imports)]
pub(crate) use self::frame::{
    build_frame, build_response_frame, command_from_host_frame, is_ack_frame,
    payload_from_host_frame,
};
use self::frame::{parse_response_frame, split_status_response};
use self::target_decode::{
    build_injump_for_dep_command, build_target_init_command, cascade_iso14443a_uid,
    decode_activation_mode, decode_target_data, default_initiator_payload, is_iso14443_4_target,
    nm_to_pm, parse_dep_target,
};
pub(crate) use self::transport::Pn53xTransport;
use self::transport::{BitTransceiveRequest, pn53x_translate_status, status_code, status_error};

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
    VirtualCard = 0x02,
    WiredCard = 0x03,
    DualCard = 0x04,
}

impl Pn532SamMode {
    fn from_raw(mode: u8) -> Option<Self> {
        match mode {
            0x01 => Some(Self::Normal),
            0x02 => Some(Self::VirtualCard),
            0x03 => Some(Self::WiredCard),
            0x04 => Some(Self::DualCard),
            _ => None,
        }
    }
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

    pub(crate) const fn acr122_usb() -> Self {
        Self {
            driver_name: "acr122_usb",
            initial_power_mode: Pn53xPowerMode::Normal,
            sam_mode_on_low_vbat: None,
            secure_element_mode: None,
            timer_correction: 46,
            usb_model: None,
        }
    }

    pub(crate) const fn acr122s() -> Self {
        Self {
            driver_name: "ACR122S",
            initial_power_mode: Pn53xPowerMode::Normal,
            sam_mode_on_low_vbat: None,
            secure_element_mode: None,
            timer_correction: 46,
            usb_model: None,
        }
    }

    pub(crate) const fn arygon() -> Self {
        Self {
            driver_name: "arygon",
            initial_power_mode: Pn53xPowerMode::Normal,
            sam_mode_on_low_vbat: None,
            secure_element_mode: None,
            timer_correction: 46,
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

        if previous_mode == Pn53xPowerMode::LowVbat
            && let Some(mode) = profile.sam_mode_on_low_vbat
        {
            let payload = match mode {
                Pn532SamMode::Normal => [mode as u8, 0x00],
                Pn532SamMode::WiredCard => [mode as u8, 0x00],
                Pn532SamMode::VirtualCard => [mode as u8, 0x00],
                Pn532SamMode::DualCard => [mode as u8, 0x00],
            };
            let _ = self.exchange_prepared_command(
                transport,
                PN532_SAM_CONFIGURATION,
                &payload,
                timeout_ms,
            )?;
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
        if self.core.chip_type() != Pn53xType::Pn532 {
            return self.remember(Err(status_error("pn532_SAMConfiguration", NFC_EDEVNOTSUPP)));
        }
        let payload = match mode {
            Pn532SamMode::Normal => [mode as u8, 0x00],
            Pn532SamMode::WiredCard => [mode as u8, 0x00],
            Pn532SamMode::VirtualCard => [mode as u8, 0x00],
            Pn532SamMode::DualCard => [mode as u8, 0x00],
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
        let response =
            self.exchange_raw(PN53X_READ_REGISTER, &payload, self.core.timeout_command_ms)?;
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
        let _ = self.exchange_raw(PN53X_WRITE_REGISTER, &payload, self.core.timeout_command_ms)?;
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
                SYMBOL_TAUTO | (((self.core.timer_prescaler >> 8) as u8) & SYMBOL_TPRESCALERHI),
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
        let values =
            self.read_registers(&[PN53X_REG_CIU_TCOUNTER_VAL_HI, PN53X_REG_CIU_TCOUNTER_VAL_LO])?;
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
        request: BitTransceiveRequest<'_, '_, '_>,
    ) -> Result<usize, Error> {
        let BitTransceiveRequest {
            operation,
            command,
            tx,
            tx_bits_len,
            tx_parity,
            rx,
            rx_parity,
            timeout_ms,
        } = request;
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
            TargetInfo::Iso14443A { atqa, sak, .. } if *sak == 0x00 && *atqa == [0x00, 0x44] => {
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

impl<T: Pn53xTransport + Send + 'static> DeviceMeta for Pn53xDevice<T> {
    fn name(&self) -> &str {
        &self.name
    }

    fn connstring(&self) -> &ConnectionString {
        &self.connstring
    }

    fn caps(&self) -> DeviceCaps {
        let mut caps = DeviceCaps::INFO
            | DeviceCaps::SET_PROPERTY_BOOL
            | DeviceCaps::SET_PROPERTY_INT
            | DeviceCaps::SUPPORTED_MODULATIONS
            | DeviceCaps::SUPPORTED_BAUD_RATES
            | DeviceCaps::INITIATOR_INIT
            | DeviceCaps::SELECT_PASSIVE_TARGET
            | DeviceCaps::POLL_TARGET
            | DeviceCaps::SELECT_DEP_TARGET
            | DeviceCaps::DESELECT_TARGET
            | DeviceCaps::TARGET_IS_PRESENT
            | DeviceCaps::TARGET_INIT
            | DeviceCaps::TRANSCEIVE_BYTES
            | DeviceCaps::TRANSCEIVE_BITS
            | DeviceCaps::TRANSCEIVE_BYTES_TIMED
            | DeviceCaps::TRANSCEIVE_BITS_TIMED
            | DeviceCaps::TARGET_SEND_BYTES
            | DeviceCaps::TARGET_RECEIVE_BYTES
            | DeviceCaps::TARGET_SEND_BITS
            | DeviceCaps::TARGET_RECEIVE_BITS
            | DeviceCaps::ABORT_COMMAND
            | DeviceCaps::IDLE
            | DeviceCaps::POWERDOWN
            | DeviceCaps::PN53X_TRANSCEIVE
            | DeviceCaps::PN53X_READ_REGISTER
            | DeviceCaps::PN53X_WRITE_REGISTER
            | DeviceCaps::PN532_SAM_CONFIGURATION;
        if self.profile.secure_element_mode.is_some() {
            caps |= DeviceCaps::INITIATOR_INIT_SECURE_ELEMENT;
        }
        caps
    }

    fn last_error(&self) -> i32 {
        self.last_error
    }
}

impl<T: Pn53xTransport + Send + 'static> InfoBackend for Pn53xDevice<T> {
    fn information_about(&mut self) -> Result<String, Error> {
        let message = format!("{} via {}", self.firmware_text(), self.connstring);
        self.last_error = 0;
        Ok(message)
    }
}

impl<T: Pn53xTransport + Send + 'static> PropertyBackend for Pn53xDevice<T> {
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
}

impl<T: Pn53xTransport + Send + 'static> InitiatorBackend for Pn53xDevice<T> {
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
        self.transceive_bits_shared(BitTransceiveRequest {
            operation: "transceive_bits",
            command: PN53X_IN_COMMUNICATE_THRU,
            tx,
            tx_bits_len,
            tx_parity,
            rx,
            rx_parity,
            timeout_ms: self.core.timeout_communication_ms,
        })
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

impl<T: Pn53xTransport + Send + 'static> TargetBackend for Pn53xDevice<T> {
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
        let _ = self.transceive_bits_shared(BitTransceiveRequest {
            operation: "target_send_bits",
            command: PN53X_TG_RESPONSE_TO_INITIATOR,
            tx,
            tx_bits_len,
            tx_parity,
            rx: &mut sink,
            rx_parity: None,
            timeout_ms: self.core.timeout_communication_ms,
        })?;
        self.last_error = 0;
        Ok(tx_bits_len)
    }

    fn target_receive_bits_driver(
        &mut self,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        self.transceive_bits_shared(BitTransceiveRequest {
            operation: "target_receive_bits",
            command: PN53X_TG_GET_INITIATOR_COMMAND,
            tx: &[],
            tx_bits_len: 0,
            tx_parity: None,
            rx,
            rx_parity,
            timeout_ms: self.core.timeout_communication_ms,
        })
    }
}

impl<T: Pn53xTransport + Send + 'static> Pn53xBackend for Pn53xDevice<T> {
    fn pn53x_transceive_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error> {
        let Some((&command, payload)) = tx.split_first() else {
            return self.remember(Err(status_error("pn53x_transceive", NFC_EINVARG)));
        };
        let timeout = if timeout >= 0 {
            timeout
        } else {
            self.core.timeout_command_ms
        };
        let response = self.exchange_raw(command, payload, timeout)?;
        let written = Self::copy_into("pn53x_transceive", &response, rx)?;
        self.last_error = 0;
        Ok(written)
    }

    fn pn53x_read_register_driver(&mut self, register: u16) -> Result<u8, Error> {
        let value = self.read_register(register)?;
        self.last_error = 0;
        Ok(value)
    }

    fn pn53x_write_register_driver(
        &mut self,
        register: u16,
        symbol_mask: u8,
        value: u8,
    ) -> Result<(), Error> {
        self.update_register_bits(register, symbol_mask, value)?;
        self.last_error = 0;
        Ok(())
    }

    fn pn532_sam_configuration_driver(&mut self, mode: u8, timeout: i32) -> Result<i32, Error> {
        let mode = Pn532SamMode::from_raw(mode)
            .ok_or_else(|| status_error("pn532_SAMConfiguration", NFC_EINVARG))?;
        let timeout = if timeout >= 0 {
            timeout
        } else {
            self.core.timeout_command_ms
        };
        self.sam_configuration(mode, timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proximate_driver::{ChipDebugOps, InitiatorOps, TargetOps};
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
        transport
            .received
            .push_back(response_frame(command, payload));
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
    fn hidden_pn53x_helpers_route_through_shared_core() {
        let mut device = probed_device();
        queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0x12]);
        assert_eq!(device.pn53x_read_register(0x6302).unwrap(), 0x12);

        queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0x12]);
        queue_command_response(&mut device.transport, PN53X_WRITE_REGISTER, &[]);
        device.pn53x_write_register(0x6302, 0x0f, 0x05).unwrap();

        queue_command_response(&mut device.transport, 0x40, &[0xaa, 0xbb]);
        let mut rx = [0u8; 4];
        assert_eq!(
            device
                .pn53x_transceive(&[0x40, 0xde, 0xad], &mut rx, 25)
                .unwrap(),
            2
        );
        assert_eq!(&rx[..2], &[0xaa, 0xbb]);

        queue_command_response(&mut device.transport, PN532_SAM_CONFIGURATION, &[]);
        assert_eq!(device.pn532_sam_configuration(0x03, 25).unwrap(), 0);
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
        queue_command_response(
            &mut device.transport,
            PN53X_READ_REGISTER,
            &[SYMBOL_TX_CRC_ENABLE],
        );
        queue_command_response(&mut device.transport, PN53X_WRITE_REGISTER, &[]);
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
        queue_command_response(
            &mut device.transport,
            PN53X_READ_REGISTER,
            &[wrapped.len() as u8],
        );
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
        queue_command_response(
            &mut device.transport,
            PN53X_TG_RESPONSE_TO_INITIATOR,
            &[0x00],
        );
        queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0x00]);

        let sent = device
            .target_send_bits(&[0x93, 0x20], 16, Some(&[1, 0]))
            .unwrap();
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
        queue_command_response(
            &mut device.transport,
            PN53X_READ_REGISTER,
            &[0x90, 0x00, 0x00],
        );
        queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0xf0, 0x00]);

        let mut rx = [0u8; 4];
        let (written, elapsed) = device.transceive_bytes_timed(&[0x00], &mut rx).unwrap();
        assert_eq!(written, 2);
        assert_eq!(&rx[..2], &[0x90, 0x00]);
        assert_eq!(elapsed, 3504);
        assert_eq!(device.transport.sent[sent_before][6], PN53X_READ_REGISTER);
        assert_eq!(
            &device.transport.sent[sent_before][7..9],
            &[
                (PN53X_REG_CIU_TX_MODE >> 8) as u8,
                PN53X_REG_CIU_TX_MODE as u8
            ]
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
