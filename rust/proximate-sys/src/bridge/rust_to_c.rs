use crate::c_api_impl::NFC_BUFSIZE_CONNSTRING;
use crate::ffi_support::{as_mut, copy_bytes_to_c_buffer};
use crate::ffi_types::{
    nfc_barcode_info, nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_felica_info,
    nfc_iso14443a_info, nfc_iso14443b_info, nfc_iso14443b2ct_info, nfc_iso14443b2sr_info,
    nfc_iso14443bi_info, nfc_iso14443biclass_info, nfc_jewel_info, nfc_mode, nfc_modulation,
    nfc_modulation_type, nfc_property, nfc_target, nfc_target_info,
};
use crate::lifecycle::{
    DEVICE_NAME_LENGTH, MAX_USER_DEFINED_DEVICES, nfc_connstring, nfc_context, nfc_device,
    nfc_user_defined_device,
};
use libc::{c_char, c_int, size_t};
use proximate_driver as rt;
use std::{ffi::CString, ptr};

pub(crate) fn write_context_to_c(context: &rt::Context, destination: *mut nfc_context) {
    let Some(destination) = (unsafe { as_mut(destination) }) else {
        return;
    };

    destination.allow_autoscan = context.config.allow_autoscan;
    destination.allow_intrusive_scan = context.config.allow_intrusive_scan;
    destination.log_level = context.config.log_level;
    destination.user_defined_device_count = 0;

    for slot in &mut destination.user_defined_devices {
        unsafe { ptr::write_bytes(slot as *mut nfc_user_defined_device, 0, 1) };
    }

    for (index, configured) in context
        .config
        .user_defined_devices
        .iter()
        .take(MAX_USER_DEFINED_DEVICES)
        .enumerate()
    {
        let slot = &mut destination.user_defined_devices[index];
        let _ = unsafe {
            copy_bytes_to_c_buffer(
                slot.name.as_mut_ptr(),
                DEVICE_NAME_LENGTH,
                configured.name.as_bytes(),
            )
        };
        let _ = unsafe {
            copy_bytes_to_c_buffer(
                slot.connstring.as_mut_ptr(),
                NFC_BUFSIZE_CONNSTRING,
                configured.connstring.as_str().as_bytes(),
            )
        };
        slot.optional = configured.optional;
        destination.user_defined_device_count += 1;
    }
}

pub(crate) fn write_target_to_c(target: &rt::Target, destination: *mut nfc_target) {
    let Some(destination) = (unsafe { as_mut(destination) }) else {
        return;
    };

    let mut raw_target = unsafe { std::mem::zeroed::<nfc_target>() };
    raw_target.nm = modulation_to_c(target.modulation);
    raw_target.nti = target_info_to_c(&target.info);
    unsafe { ptr::write_unaligned(destination, raw_target) };
}

pub(crate) fn mode_to_c(mode: rt::Mode) -> nfc_mode {
    match mode {
        rt::Mode::Target => nfc_mode::N_TARGET,
        rt::Mode::Initiator => nfc_mode::N_INITIATOR,
    }
}

pub(crate) fn modulation_to_c(modulation: rt::Modulation) -> nfc_modulation {
    nfc_modulation {
        nmt: modulation_type_to_c(modulation.modulation_type),
        nbr: baud_rate_to_c(modulation.baud_rate),
    }
}

pub(crate) fn dep_mode_to_c(mode: rt::DepMode) -> nfc_dep_mode {
    match mode {
        rt::DepMode::Undefined => nfc_dep_mode::NDM_UNDEFINED,
        rt::DepMode::Passive => nfc_dep_mode::NDM_PASSIVE,
        rt::DepMode::Active => nfc_dep_mode::NDM_ACTIVE,
    }
}

pub(crate) fn baud_rate_to_c(rate: rt::BaudRate) -> nfc_baud_rate {
    match rate {
        rt::BaudRate::Undefined => nfc_baud_rate::NBR_UNDEFINED,
        rt::BaudRate::Br106 => nfc_baud_rate::NBR_106,
        rt::BaudRate::Br212 => nfc_baud_rate::NBR_212,
        rt::BaudRate::Br424 => nfc_baud_rate::NBR_424,
        rt::BaudRate::Br847 => nfc_baud_rate::NBR_847,
    }
}

pub(crate) fn target_to_c(target: &rt::Target) -> nfc_target {
    nfc_target {
        nti: target_info_to_c(&target.info),
        nm: modulation_to_c(target.modulation),
    }
}

pub(crate) fn dep_info_to_c(info: &rt::DepInfo) -> nfc_dep_info {
    let mut raw = unsafe { std::mem::zeroed::<nfc_dep_info>() };
    raw.abtNFCID3 = info.nfcid3;
    raw.btDID = info.did;
    raw.btBS = info.bs;
    raw.btBR = info.br;
    raw.btTO = info.timeout;
    raw.btPP = info.pp;
    raw.szGB = info.general_bytes.len().min(raw.abtGB.len());
    raw.abtGB[..raw.szGB].copy_from_slice(&info.general_bytes[..raw.szGB]);
    raw.ndm = dep_mode_to_c(info.mode);
    raw
}

pub(crate) fn property_to_c(property: rt::Property) -> nfc_property {
    match property {
        rt::Property::TimeoutCommand => nfc_property::NP_TIMEOUT_COMMAND,
        rt::Property::TimeoutAtr => nfc_property::NP_TIMEOUT_ATR,
        rt::Property::TimeoutCom => nfc_property::NP_TIMEOUT_COM,
        rt::Property::HandleCrc => nfc_property::NP_HANDLE_CRC,
        rt::Property::HandleParity => nfc_property::NP_HANDLE_PARITY,
        rt::Property::ActivateField => nfc_property::NP_ACTIVATE_FIELD,
        rt::Property::ActivateCrypto1 => nfc_property::NP_ACTIVATE_CRYPTO1,
        rt::Property::InfiniteSelect => nfc_property::NP_INFINITE_SELECT,
        rt::Property::AcceptInvalidFrames => nfc_property::NP_ACCEPT_INVALID_FRAMES,
        rt::Property::AcceptMultipleFrames => nfc_property::NP_ACCEPT_MULTIPLE_FRAMES,
        rt::Property::AutoIso14443_4 => nfc_property::NP_AUTO_ISO14443_4,
        rt::Property::EasyFraming => nfc_property::NP_EASY_FRAMING,
        rt::Property::ForceIso14443A => nfc_property::NP_FORCE_ISO14443_A,
        rt::Property::ForceIso14443B => nfc_property::NP_FORCE_ISO14443_B,
        rt::Property::ForceSpeed106 => nfc_property::NP_FORCE_SPEED_106,
    }
}

pub(crate) fn modulation_type_to_c(value: rt::ModulationType) -> nfc_modulation_type {
    match value {
        rt::ModulationType::Undefined => nfc_modulation_type::NMT_UNDEFINED,
        rt::ModulationType::Iso14443A => nfc_modulation_type::NMT_ISO14443A,
        rt::ModulationType::Jewel => nfc_modulation_type::NMT_JEWEL,
        rt::ModulationType::Iso14443B => nfc_modulation_type::NMT_ISO14443B,
        rt::ModulationType::Iso14443Bi => nfc_modulation_type::NMT_ISO14443BI,
        rt::ModulationType::Iso14443B2Sr => nfc_modulation_type::NMT_ISO14443B2SR,
        rt::ModulationType::Iso14443B2Ct => nfc_modulation_type::NMT_ISO14443B2CT,
        rt::ModulationType::Felica => nfc_modulation_type::NMT_FELICA,
        rt::ModulationType::Dep => nfc_modulation_type::NMT_DEP,
        rt::ModulationType::Barcode => nfc_modulation_type::NMT_BARCODE,
        rt::ModulationType::Iso14443BiClass => nfc_modulation_type::NMT_ISO14443BICLASS,
    }
}

fn target_info_to_c(info: &rt::TargetInfo) -> nfc_target_info {
    match info {
        rt::TargetInfo::None => unsafe { std::mem::zeroed() },
        rt::TargetInfo::Iso14443A {
            atqa,
            sak,
            uid,
            ats,
        } => {
            let mut value = unsafe { std::mem::zeroed::<nfc_iso14443a_info>() };
            value.abtAtqa = *atqa;
            value.btSak = *sak;
            value.szUidLen = uid.len().min(value.abtUid.len());
            value.abtUid[..value.szUidLen].copy_from_slice(&uid[..value.szUidLen]);
            value.szAtsLen = ats.len().min(value.abtAts.len());
            value.abtAts[..value.szAtsLen].copy_from_slice(&ats[..value.szAtsLen]);
            nfc_target_info { nai: value }
        }
        rt::TargetInfo::Felica {
            len,
            response_code,
            id,
            pad,
            system_code,
        } => nfc_target_info {
            nfi: nfc_felica_info {
                szLen: *len,
                btResCode: *response_code,
                abtId: *id,
                abtPad: *pad,
                abtSysCode: *system_code,
            },
        },
        rt::TargetInfo::Iso14443B {
            pupi,
            application_data,
            protocol_info,
            card_identifier,
        } => nfc_target_info {
            nbi: nfc_iso14443b_info {
                abtPupi: *pupi,
                abtApplicationData: *application_data,
                abtProtocolInfo: *protocol_info,
                ui8CardIdentifier: *card_identifier,
            },
        },
        rt::TargetInfo::Iso14443Bi {
            div,
            version_log,
            config,
            atr,
        } => {
            let mut value = unsafe { std::mem::zeroed::<nfc_iso14443bi_info>() };
            value.abtDIV = *div;
            value.btVerLog = *version_log;
            value.btConfig = *config;
            value.szAtrLen = atr.len().min(value.abtAtr.len());
            value.abtAtr[..value.szAtrLen].copy_from_slice(&atr[..value.szAtrLen]);
            nfc_target_info { nii: value }
        }
        rt::TargetInfo::Iso14443BiClass { uid } => nfc_target_info {
            nhi: nfc_iso14443biclass_info { abtUID: *uid },
        },
        rt::TargetInfo::Iso14443B2Sr { uid } => nfc_target_info {
            nsi: nfc_iso14443b2sr_info { abtUID: *uid },
        },
        rt::TargetInfo::Iso14443B2Ct {
            uid,
            product_code,
            fabrication_code,
        } => nfc_target_info {
            nci: nfc_iso14443b2ct_info {
                abtUID: *uid,
                btProdCode: *product_code,
                btFabCode: *fabrication_code,
            },
        },
        rt::TargetInfo::Jewel { sens_res, id } => nfc_target_info {
            nji: nfc_jewel_info {
                btSensRes: *sens_res,
                btId: *id,
            },
        },
        rt::TargetInfo::Dep(info) => nfc_target_info {
            ndi: dep_info_to_c(info),
        },
        rt::TargetInfo::Barcode { data } => {
            let mut value = unsafe { std::mem::zeroed::<nfc_barcode_info>() };
            value.szDataLen = data.len().min(value.abtData.len());
            value.abtData[..value.szDataLen].copy_from_slice(&data[..value.szDataLen]);
            nfc_target_info { nti: value }
        }
    }
}

pub(crate) struct TargetOut {
    raw: *mut nfc_target,
}

impl TargetOut {
    pub(crate) unsafe fn from_raw(raw: *mut nfc_target) -> Self {
        Self { raw }
    }

    pub(crate) fn write_back(&self, target: &rt::Target) {
        if !self.raw.is_null() {
            write_target_to_c(target, self.raw);
        }
    }
}

pub(crate) struct TargetSliceOut {
    raw: *mut nfc_target,
    len: usize,
}

impl TargetSliceOut {
    pub(crate) unsafe fn from_raw(
        device: *mut nfc_device,
        raw: *mut nfc_target,
        len: size_t,
    ) -> Result<Self, c_int> {
        if len == 0 {
            return Ok(Self { raw, len: 0 });
        }
        if raw.is_null() {
            return Err(super::status::invalid_argument_status(device));
        }
        Ok(Self { raw, len })
    }

    pub(crate) fn write_back(&self, targets: &[rt::Target]) {
        for (index, target) in targets.iter().take(self.len).enumerate() {
            write_target_to_c(target, unsafe { self.raw.add(index) });
        }
    }
}

pub(crate) struct TargetInOut {
    raw: *mut nfc_target,
    value: rt::Target,
}

impl TargetInOut {
    pub(crate) unsafe fn from_raw(
        device: *mut nfc_device,
        raw: *mut nfc_target,
    ) -> Result<Self, c_int> {
        if raw.is_null() {
            return Err(super::status::invalid_argument_status(device));
        }
        Ok(Self {
            raw,
            value: super::decode::target_from_c(raw.cast_const()),
        })
    }

    pub(crate) fn as_mut(&mut self) -> &mut rt::Target {
        &mut self.value
    }

    pub(crate) fn write_back(&self) {
        write_target_to_c(&self.value, self.raw);
    }
}

pub(crate) struct CyclesOut {
    raw: *mut u32,
}

impl CyclesOut {
    pub(crate) unsafe fn from_raw(raw: *mut u32) -> Self {
        Self { raw }
    }

    pub(crate) fn write_back(&self, cycles: u32) {
        if let Some(raw) = unsafe { as_mut(self.raw) } {
            *raw = cycles;
        }
    }
}

pub(crate) struct CStringOut {
    raw: *mut *mut c_char,
}

impl CStringOut {
    pub(crate) unsafe fn from_raw(
        device: *mut nfc_device,
        raw: *mut *mut c_char,
    ) -> Result<Self, c_int> {
        if raw.is_null() {
            return Err(super::status::invalid_argument_status(device));
        }
        Ok(Self { raw })
    }

    pub(crate) fn write_back(&self, device: *mut nfc_device, value: &str) -> c_int {
        let rendered = match CString::new(value) {
            Ok(value) => value,
            Err(_) => return super::status::soft_error_status(device),
        };
        let allocation_len = rendered.as_bytes().len() + 1;
        let allocation = unsafe { libc::malloc(allocation_len) as *mut c_char };
        if allocation.is_null() {
            return super::status::soft_error_status(device);
        }

        if !unsafe { copy_bytes_to_c_buffer(allocation, allocation_len, rendered.as_bytes()) } {
            unsafe { libc::free(allocation.cast()) };
            return super::status::soft_error_status(device);
        }

        unsafe {
            *self.raw = allocation;
        }
        super::status::reset_device_last_error(device);
        rendered.as_bytes().len() as c_int
    }
}

pub(crate) struct SupportedModulationsOut {
    device: *mut nfc_device,
    raw: *mut *const nfc_modulation_type,
}

impl SupportedModulationsOut {
    pub(crate) unsafe fn from_raw(
        device: *mut nfc_device,
        raw: *mut *const nfc_modulation_type,
    ) -> Result<Self, c_int> {
        if raw.is_null() || unsafe { super::rust_device_state_mut(device) }.is_none() {
            return Err(super::status::invalid_argument_status(device));
        }
        Ok(Self { device, raw })
    }

    pub(crate) fn write_back(&self, values: Vec<rt::ModulationType>) -> c_int {
        let Some(state) = (unsafe { super::rust_device_state_mut(self.device) }) else {
            return super::status::invalid_argument_status(self.device);
        };
        state.supported_modulations.clear();
        state
            .supported_modulations
            .extend(values.into_iter().map(modulation_type_to_c));
        state
            .supported_modulations
            .push(nfc_modulation_type::NMT_UNDEFINED);
        unsafe {
            *self.raw = state.supported_modulations.as_ptr();
        }
        super::status::reset_device_last_error(self.device);
        0
    }
}

pub(crate) struct SupportedBaudRatesOut {
    device: *mut nfc_device,
    raw: *mut *const nfc_baud_rate,
}

impl SupportedBaudRatesOut {
    pub(crate) unsafe fn from_raw(
        device: *mut nfc_device,
        raw: *mut *const nfc_baud_rate,
    ) -> Result<Self, c_int> {
        if raw.is_null() || unsafe { super::rust_device_state_mut(device) }.is_none() {
            return Err(super::status::invalid_argument_status(device));
        }
        Ok(Self { device, raw })
    }

    pub(crate) fn write_back(&self, values: Vec<rt::BaudRate>) -> c_int {
        let Some(state) = (unsafe { super::rust_device_state_mut(self.device) }) else {
            return super::status::invalid_argument_status(self.device);
        };
        state.supported_baud_rates.clear();
        state
            .supported_baud_rates
            .extend(values.into_iter().map(baud_rate_to_c));
        state
            .supported_baud_rates
            .push(nfc_baud_rate::NBR_UNDEFINED);
        unsafe {
            *self.raw = state.supported_baud_rates.as_ptr();
        }
        super::status::reset_device_last_error(self.device);
        0
    }
}

pub(crate) struct ConnstringsOut {
    raw: *mut nfc_connstring,
    len: usize,
}

impl ConnstringsOut {
    pub(crate) unsafe fn from_raw(raw: *mut nfc_connstring, len: size_t) -> Option<Self> {
        if raw.is_null() || len == 0 {
            return None;
        }
        Some(Self { raw, len })
    }

    pub(crate) fn write_back<I>(&self, connstrings: I) -> usize
    where
        I: IntoIterator<Item = rt::ConnectionString>,
    {
        let mut written = 0usize;
        for connstring in connstrings.into_iter().take(self.len) {
            let destination = unsafe { self.raw.add(written) };
            if unsafe {
                copy_bytes_to_c_buffer(
                    destination.cast(),
                    NFC_BUFSIZE_CONNSTRING,
                    connstring.as_str().as_bytes(),
                )
            } {
                written += 1;
            }
        }
        written
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::decode;
    use crate::ffi_support::fixed_c_buffer_to_string;
    use std::ffi::CStr;
    use std::ptr;

    fn sample_target() -> rt::Target {
        rt::Target {
            modulation: rt::Modulation {
                modulation_type: rt::ModulationType::Iso14443A,
                baud_rate: rt::BaudRate::Br106,
            },
            info: rt::TargetInfo::Iso14443A {
                atqa: [0x04, 0x00],
                sak: 0x08,
                uid: vec![0x01, 0x02, 0x03, 0x04],
                ats: vec![0x75, 0x77],
            },
        }
    }

    #[test]
    fn target_out_round_trips_runtime_target() {
        let target = sample_target();
        let mut raw = unsafe { std::mem::zeroed::<nfc_target>() };
        let out = unsafe { TargetOut::from_raw(&mut raw) };
        out.write_back(&target);
        assert_eq!(decode::target_from_c(ptr::addr_of!(raw)), target);
    }

    #[test]
    fn cycles_out_writes_when_pointer_is_present() {
        let mut cycles = 0u32;
        let output = unsafe { CyclesOut::from_raw(ptr::addr_of_mut!(cycles)) };
        output.write_back(42);
        assert_eq!(cycles, 42);
    }

    #[test]
    fn cstring_out_allocates_and_writes_bytes() {
        let mut device = unsafe { std::mem::zeroed::<nfc_device>() };
        device.last_error = -7;
        let mut raw = ptr::null_mut();
        let output = unsafe { CStringOut::from_raw(&mut device, ptr::addr_of_mut!(raw)) }.unwrap();
        let written = output.write_back(&mut device, "hello");
        assert_eq!(written, 5);
        assert_eq!(device.last_error, 0);
        assert_eq!(unsafe { CStr::from_ptr(raw) }.to_str().unwrap(), "hello");
        unsafe { libc::free(raw.cast()) };
    }

    #[test]
    fn connstrings_out_writes_all_entries() {
        let mut connstrings = [[0; NFC_BUFSIZE_CONNSTRING]; 2];
        let output = unsafe { ConnstringsOut::from_raw(connstrings.as_mut_ptr(), 2) }.unwrap();
        let written = output.write_back([
            rt::ConnectionString::new("pn53x_usb:001:001").unwrap(),
            rt::ConnectionString::new("pn71xx_i2c:/dev/i2c-1").unwrap(),
        ]);
        assert_eq!(written, 2);
        assert_eq!(
            fixed_c_buffer_to_string(&connstrings[0]),
            "pn53x_usb:001:001"
        );
        assert_eq!(
            fixed_c_buffer_to_string(&connstrings[1]),
            "pn71xx_i2c:/dev/i2c-1"
        );
    }
}
