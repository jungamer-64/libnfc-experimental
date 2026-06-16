use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Pn53xType {
    Unknown,
    Pn531,
    Pn532,
    Pn533,
    Rcs360,
}

impl Pn53xType {
    pub(super) fn from_ic_byte(ic: u8) -> Self {
        match ic {
            0x31 => Self::Pn531,
            0x32 => Self::Pn532,
            0x33 => Self::Pn533,
            0x88 => Self::Rcs360,
            _ => Self::Unknown,
        }
    }

    pub(super) fn label(self) -> &'static str {
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
    pub(super) fn from_raw(mode: u8) -> Option<Self> {
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

    pub(super) fn supported_modulations(self, mode: Mode) -> Vec<ModulationType> {
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
    pub(super) fn chip_type(&self) -> Pn53xType {
        Pn53xType::from_ic_byte(self.ic)
    }

    pub(super) fn text(&self) -> String {
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
pub(super) struct PropertyState {
    pub(super) handle_crc: bool,
    pub(super) handle_parity: bool,
    pub(super) activate_field: bool,
    pub(super) activate_crypto1: bool,
    pub(super) infinite_select: bool,
    pub(super) accept_invalid_frames: bool,
    pub(super) accept_multiple_frames: bool,
    pub(super) auto_iso14443_4: bool,
    pub(super) easy_framing: bool,
    pub(super) force_iso14443_a: bool,
    pub(super) force_iso14443_b: bool,
    pub(super) force_speed_106: bool,
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
    pub(super) fn get(self, property: Property) -> Option<bool> {
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

    pub(super) fn set(&mut self, property: Property, value: bool) -> Result<(), Error> {
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
