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
 * Copyright (C) 2020      Adam Laurie
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Pn53xOperatingMode {
    Idle,
    Initiator,
    Target,
}

/// Whether the protocol engine can issue a new command without first rebuilding chip state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Pn53xProtocolState {
    Ready,
    NeedsReinitialization { cause: Error },
}

impl Pn53xProtocolState {
    pub(super) fn recovery_cause(&self) -> Option<&Error> {
        match self {
            Self::Ready => None,
            Self::NeedsReinitialization { cause } => Some(cause),
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
        if self.ic == 0x33 && self.version == 0x01 {
            Pn53xType::Rcs360
        } else {
            Pn53xType::from_ic_byte(self.ic)
        }
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

/// Capabilities derived exactly once from the GetFirmwareVersion response.
///
/// The support byte and chip identity are kept together so driver profiles and
/// local state cannot become competing authorities for protocol admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChipCapabilities {
    firmware: Pn53xFirmwareVersion,
}

impl ChipCapabilities {
    pub(super) fn from_firmware_response(payload: &[u8]) -> Result<Self, Error> {
        let firmware = match payload {
            [version, revision] => Pn53xFirmwareVersion {
                ic: 0x31,
                version: *version,
                revision: *revision,
                support: SUPPORT_ISO14443A | SUPPORT_ISO18092,
            },
            [ic @ (0x32 | 0x33), version, revision, support] => Pn53xFirmwareVersion {
                ic: *ic,
                version: *version,
                revision: *revision,
                support: *support,
            },
            _ => return Err(Error::UnsupportedOperation("pn53x_firmware_version")),
        };
        Ok(Self { firmware })
    }

    pub(super) fn firmware(&self) -> &Pn53xFirmwareVersion {
        &self.firmware
    }

    pub(super) fn chip_type(&self) -> Pn53xType {
        self.firmware.chip_type()
    }

    pub(super) fn supported_modulations(&self, mode: Mode) -> Vec<ModulationType> {
        if mode == Mode::Target {
            return vec![
                ModulationType::Iso14443A,
                ModulationType::Felica,
                ModulationType::Dep,
            ];
        }

        let mut supported = Vec::new();
        if self.firmware.support & SUPPORT_ISO14443A != 0 {
            supported.push(ModulationType::Iso14443A);
            supported.push(ModulationType::Felica);
        }
        if self.firmware.support & SUPPORT_ISO14443B != 0 {
            supported.extend_from_slice(&[
                ModulationType::Iso14443B,
                ModulationType::Iso14443Bi,
                ModulationType::Iso14443B2Sr,
                ModulationType::Iso14443B2Ct,
                ModulationType::Iso14443BiClass,
            ]);
        }
        if self.chip_type() != Pn53xType::Pn531 {
            supported.push(ModulationType::Jewel);
            supported.push(ModulationType::Barcode);
        }
        supported.push(ModulationType::Dep);
        supported
    }

    pub(super) fn supported_baud_rates(
        &self,
        mode: Mode,
        modulation_type: ModulationType,
    ) -> Vec<BaudRate> {
        match modulation_type {
            ModulationType::Iso14443A
                if self.chip_type() == Pn53xType::Pn533 && mode == Mode::Initiator =>
            {
                vec![
                    BaudRate::Br847,
                    BaudRate::Br424,
                    BaudRate::Br212,
                    BaudRate::Br106,
                ]
            }
            ModulationType::Iso14443A => {
                vec![BaudRate::Br424, BaudRate::Br212, BaudRate::Br106]
            }
            ModulationType::Iso14443B if self.chip_type() == Pn53xType::Pn533 => vec![
                BaudRate::Br847,
                BaudRate::Br424,
                BaudRate::Br212,
                BaudRate::Br106,
            ],
            ModulationType::Iso14443B
            | ModulationType::Iso14443Bi
            | ModulationType::Iso14443B2Sr
            | ModulationType::Iso14443B2Ct
            | ModulationType::Iso14443BiClass => vec![BaudRate::Br106],
            ModulationType::Felica => vec![BaudRate::Br424, BaudRate::Br212],
            ModulationType::Dep => {
                vec![BaudRate::Br424, BaudRate::Br212, BaudRate::Br106]
            }
            ModulationType::Jewel | ModulationType::Barcode => vec![BaudRate::Br106],
        }
    }

    pub(super) fn supports(&self, modulation: Modulation, mode: Mode) -> bool {
        self.supported_modulations(mode)
            .contains(&modulation.modulation_type())
            && self
                .supported_baud_rates(mode, modulation.modulation_type())
                .contains(&modulation.baud_rate())
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
            Property::ForceIso14443A
            | Property::ForceIso14443B
            | Property::ForceSpeed106
            | Property::TimeoutCommand
            | Property::TimeoutAtr
            | Property::TimeoutCom => return None,
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
            Property::ForceIso14443A
            | Property::ForceIso14443B
            | Property::ForceSpeed106
            | Property::TimeoutCommand
            | Property::TimeoutAtr
            | Property::TimeoutCom => {
                return Err(Error::InvalidArgument("property"));
            }
        }
        Ok(())
    }
}
