use crate::bridge::invalid_argument_status;
use crate::c_api_impl::NFC_BUFSIZE_CONNSTRING;
use crate::ffi_support::{
    as_ref, bounded_strlen, c_string_ptr_to_string, fixed_c_buffer_to_string,
};
use crate::ffi_types::{
    nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_modulation, nfc_modulation_type, nfc_property,
    nfc_target, nfc_target_info,
};
use crate::lifecycle::{MAX_USER_DEFINED_DEVICES, nfc_context, runtime_context_from_c};
use libc::{c_char, c_int, size_t};
use proximate_driver as rt;
use std::{ptr, slice};

pub(crate) fn context_from_c(context: *const nfc_context) -> rt::Context {
    let Some(context_ref) = (unsafe { as_ref(context) }) else {
        return rt::Context::default();
    };

    let mut runtime = unsafe { runtime_context_from_c(context) }.unwrap_or_default();

    let mut user_defined_devices = Vec::new();
    let count = (context_ref.user_defined_device_count as usize).min(MAX_USER_DEFINED_DEVICES);
    for configured in &context_ref.user_defined_devices[..count] {
        let connstring = fixed_c_buffer_to_string(&configured.connstring);
        if connstring.is_empty() {
            continue;
        }
        let Ok(connstring) = rt::ConnectionString::new(connstring) else {
            continue;
        };
        user_defined_devices.push(rt::UserDefinedDevice {
            name: fixed_c_buffer_to_string(&configured.name),
            connstring,
            optional: configured.optional,
        });
    }

    runtime.config = rt::ContextConfig {
        allow_autoscan: context_ref.allow_autoscan,
        allow_intrusive_scan: context_ref.allow_intrusive_scan,
        log_level: context_ref.log_level,
        user_defined_devices,
    };
    runtime
}

pub(crate) fn target_from_c(target: *const nfc_target) -> rt::Target {
    let Some(target_ref) = (unsafe { as_ref(target) }) else {
        return rt::Target::new(rt::Modulation {
            modulation_type: rt::ModulationType::Undefined,
            baud_rate: rt::BaudRate::Undefined,
        });
    };

    let modulation =
        modulation_from_c(unsafe { ptr::read_unaligned(ptr::addr_of!(target_ref.nm)) });
    let info_union: nfc_target_info = unsafe { ptr::read_unaligned(ptr::addr_of!(target_ref.nti)) };
    let info = match modulation.modulation_type {
        rt::ModulationType::Iso14443A => {
            let info = unsafe { info_union.nai };
            rt::TargetInfo::Iso14443A {
                atqa: info.abtAtqa,
                sak: info.btSak,
                uid: info.abtUid[..info.szUidLen.min(info.abtUid.len())].to_vec(),
                ats: info.abtAts[..info.szAtsLen.min(info.abtAts.len())].to_vec(),
            }
        }
        rt::ModulationType::Felica => {
            let info = unsafe { info_union.nfi };
            rt::TargetInfo::Felica {
                len: info.szLen,
                response_code: info.btResCode,
                id: info.abtId,
                pad: info.abtPad,
                system_code: info.abtSysCode,
            }
        }
        rt::ModulationType::Iso14443B => {
            let info = unsafe { info_union.nbi };
            rt::TargetInfo::Iso14443B {
                pupi: info.abtPupi,
                application_data: info.abtApplicationData,
                protocol_info: info.abtProtocolInfo,
                card_identifier: info.ui8CardIdentifier,
            }
        }
        rt::ModulationType::Iso14443Bi => {
            let info = unsafe { info_union.nii };
            rt::TargetInfo::Iso14443Bi {
                div: info.abtDIV,
                version_log: info.btVerLog,
                config: info.btConfig,
                atr: info.abtAtr[..info.szAtrLen.min(info.abtAtr.len())].to_vec(),
            }
        }
        rt::ModulationType::Iso14443BiClass => {
            let info = unsafe { info_union.nhi };
            rt::TargetInfo::Iso14443BiClass { uid: info.abtUID }
        }
        rt::ModulationType::Iso14443B2Sr => {
            let info = unsafe { info_union.nsi };
            rt::TargetInfo::Iso14443B2Sr { uid: info.abtUID }
        }
        rt::ModulationType::Iso14443B2Ct => {
            let info = unsafe { info_union.nci };
            rt::TargetInfo::Iso14443B2Ct {
                uid: info.abtUID,
                product_code: info.btProdCode,
                fabrication_code: info.btFabCode,
            }
        }
        rt::ModulationType::Jewel => {
            let info = unsafe { info_union.nji };
            rt::TargetInfo::Jewel {
                sens_res: info.btSensRes,
                id: info.btId,
            }
        }
        rt::ModulationType::Dep => {
            let info = unsafe { info_union.ndi };
            rt::TargetInfo::Dep(dep_info_from_c(info))
        }
        rt::ModulationType::Barcode => {
            let info = unsafe { info_union.nti };
            rt::TargetInfo::Barcode {
                data: info.abtData[..info.szDataLen.min(info.abtData.len())].to_vec(),
            }
        }
        rt::ModulationType::Undefined => rt::TargetInfo::None,
    };

    rt::Target { modulation, info }
}

pub(crate) fn property_from_c(property: nfc_property) -> rt::Property {
    match property {
        nfc_property::NP_TIMEOUT_COMMAND => rt::Property::TimeoutCommand,
        nfc_property::NP_TIMEOUT_ATR => rt::Property::TimeoutAtr,
        nfc_property::NP_TIMEOUT_COM => rt::Property::TimeoutCom,
        nfc_property::NP_HANDLE_CRC => rt::Property::HandleCrc,
        nfc_property::NP_HANDLE_PARITY => rt::Property::HandleParity,
        nfc_property::NP_ACTIVATE_FIELD => rt::Property::ActivateField,
        nfc_property::NP_ACTIVATE_CRYPTO1 => rt::Property::ActivateCrypto1,
        nfc_property::NP_INFINITE_SELECT => rt::Property::InfiniteSelect,
        nfc_property::NP_ACCEPT_INVALID_FRAMES => rt::Property::AcceptInvalidFrames,
        nfc_property::NP_ACCEPT_MULTIPLE_FRAMES => rt::Property::AcceptMultipleFrames,
        nfc_property::NP_AUTO_ISO14443_4 => rt::Property::AutoIso14443_4,
        nfc_property::NP_EASY_FRAMING => rt::Property::EasyFraming,
        nfc_property::NP_FORCE_ISO14443_A => rt::Property::ForceIso14443A,
        nfc_property::NP_FORCE_ISO14443_B => rt::Property::ForceIso14443B,
        nfc_property::NP_FORCE_SPEED_106 => rt::Property::ForceSpeed106,
    }
}

pub(crate) fn modulation_from_c(modulation: nfc_modulation) -> rt::Modulation {
    rt::Modulation {
        modulation_type: modulation_type_from_c(unsafe {
            ptr::addr_of!(modulation.nmt).read_unaligned()
        }),
        baud_rate: baud_rate_from_c(unsafe { ptr::addr_of!(modulation.nbr).read_unaligned() }),
    }
}

pub(crate) fn dep_mode_from_c(mode: nfc_dep_mode) -> rt::DepMode {
    match mode {
        nfc_dep_mode::NDM_UNDEFINED => rt::DepMode::Undefined,
        nfc_dep_mode::NDM_PASSIVE => rt::DepMode::Passive,
        nfc_dep_mode::NDM_ACTIVE => rt::DepMode::Active,
    }
}

pub(crate) fn baud_rate_from_c(rate: nfc_baud_rate) -> rt::BaudRate {
    match rate {
        nfc_baud_rate::NBR_UNDEFINED => rt::BaudRate::Undefined,
        nfc_baud_rate::NBR_106 => rt::BaudRate::Br106,
        nfc_baud_rate::NBR_212 => rt::BaudRate::Br212,
        nfc_baud_rate::NBR_424 => rt::BaudRate::Br424,
        nfc_baud_rate::NBR_847 => rt::BaudRate::Br847,
    }
}

pub(crate) fn dep_info_from_c(info: nfc_dep_info) -> rt::DepInfo {
    rt::DepInfo {
        nfcid3: info.abtNFCID3,
        did: info.btDID,
        bs: info.btBS,
        br: info.btBR,
        timeout: info.btTO,
        pp: info.btPP,
        general_bytes: info.abtGB[..info.szGB.min(info.abtGB.len())].to_vec(),
        mode: dep_mode_from_c(info.ndm),
    }
}

pub(crate) fn modulation_type_from_c(value: nfc_modulation_type) -> rt::ModulationType {
    match value {
        nfc_modulation_type::NMT_UNDEFINED => rt::ModulationType::Undefined,
        nfc_modulation_type::NMT_ISO14443A => rt::ModulationType::Iso14443A,
        nfc_modulation_type::NMT_JEWEL => rt::ModulationType::Jewel,
        nfc_modulation_type::NMT_ISO14443B => rt::ModulationType::Iso14443B,
        nfc_modulation_type::NMT_ISO14443BI => rt::ModulationType::Iso14443Bi,
        nfc_modulation_type::NMT_ISO14443B2SR => rt::ModulationType::Iso14443B2Sr,
        nfc_modulation_type::NMT_ISO14443B2CT => rt::ModulationType::Iso14443B2Ct,
        nfc_modulation_type::NMT_FELICA => rt::ModulationType::Felica,
        nfc_modulation_type::NMT_DEP => rt::ModulationType::Dep,
        nfc_modulation_type::NMT_BARCODE => rt::ModulationType::Barcode,
        nfc_modulation_type::NMT_ISO14443BICLASS => rt::ModulationType::Iso14443BiClass,
    }
}

pub(crate) fn decode_connstring_ptr(
    connstring: *const c_char,
) -> Result<Option<rt::ConnectionString>, rt::Error> {
    if connstring.is_null() {
        return Ok(None);
    }

    let value = c_string_ptr_to_string(
        connstring,
        bounded_strlen(connstring, NFC_BUFSIZE_CONNSTRING),
    );
    rt::ConnectionString::new(value).map(Some)
}

#[derive(Debug)]
pub(crate) struct InputBytes<'a>(&'a [u8]);

impl<'a> InputBytes<'a> {
    pub(crate) unsafe fn from_raw(
        device: *mut crate::lifecycle::nfc_device,
        bytes: *const u8,
        len: size_t,
    ) -> Result<Self, c_int> {
        if len == 0 {
            return Ok(Self(&[]));
        }
        if bytes.is_null() {
            return Err(invalid_argument_status(device));
        }
        Ok(Self(unsafe { slice::from_raw_parts(bytes, len) }))
    }

    pub(crate) fn as_slice(&self) -> &[u8] {
        self.0
    }

    pub(crate) fn as_optional(&self) -> Option<&[u8]> {
        (!self.0.is_empty()).then_some(self.0)
    }
}

#[derive(Debug)]
pub(crate) struct OutputBytes<'a>(&'a mut [u8]);

impl<'a> OutputBytes<'a> {
    pub(crate) unsafe fn from_raw(
        device: *mut crate::lifecycle::nfc_device,
        bytes: *mut u8,
        len: size_t,
    ) -> Result<Self, c_int> {
        if len == 0 {
            return Ok(Self(&mut []));
        }
        if bytes.is_null() {
            return Err(invalid_argument_status(device));
        }
        Ok(Self(unsafe { slice::from_raw_parts_mut(bytes, len) }))
    }

    pub(crate) fn as_mut_slice(&mut self) -> &mut [u8] {
        self.0
    }
}

#[derive(Debug)]
pub(crate) struct ParityMarker<'a>(Option<&'a [u8]>);

impl<'a> ParityMarker<'a> {
    pub(crate) unsafe fn from_raw(bytes: *const u8) -> Self {
        if bytes.is_null() {
            Self(None)
        } else {
            Self(Some(unsafe { slice::from_raw_parts(bytes, 1) }))
        }
    }

    pub(crate) fn as_deref(&self) -> Option<&[u8]> {
        self.0
    }
}

#[derive(Debug)]
pub(crate) struct ParityMarkerMut<'a>(Option<&'a mut [u8]>);

impl<'a> ParityMarkerMut<'a> {
    pub(crate) unsafe fn from_raw(bytes: *mut u8) -> Self {
        if bytes.is_null() {
            Self(None)
        } else {
            Self(Some(unsafe { slice::from_raw_parts_mut(bytes, 1) }))
        }
    }

    pub(crate) fn as_deref_mut(&mut self) -> Option<&mut [u8]> {
        self.0.as_deref_mut()
    }
}

pub(crate) unsafe fn decode_modulations(
    device: *mut crate::lifecycle::nfc_device,
    modulations: *const nfc_modulation,
    len: size_t,
) -> Result<Vec<rt::Modulation>, c_int> {
    if len == 0 {
        return Ok(Vec::new());
    }
    if modulations.is_null() {
        return Err(invalid_argument_status(device));
    }
    Ok(unsafe { slice::from_raw_parts(modulations, len) }
        .iter()
        .copied()
        .map(modulation_from_c)
        .collect())
}

pub(crate) unsafe fn decode_optional_dep_info(
    initiator: *const nfc_dep_info,
) -> Option<rt::DepInfo> {
    if initiator.is_null() {
        None
    } else {
        Some(dep_info_from_c(unsafe { ptr::read_unaligned(initiator) }))
    }
}

pub(crate) unsafe fn decode_optional_target(target: *const nfc_target) -> Option<rt::Target> {
    (!target.is_null()).then(|| target_from_c(target))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi_types::{nfc_baud_rate, nfc_dep_mode, nfc_modulation, nfc_modulation_type};
    use crate::lifecycle::nfc_device;
    use std::ptr;

    #[test]
    fn decode_connstring_ptr_handles_null() {
        assert_eq!(decode_connstring_ptr(std::ptr::null()).unwrap(), None);
    }

    #[test]
    fn output_bytes_rejects_null_nonzero_len() {
        let status = unsafe {
            OutputBytes::from_raw(std::ptr::null_mut(), std::ptr::null_mut(), 1).unwrap_err()
        };
        assert_eq!(status, crate::bridge::NFC_EINVARG);
    }

    #[test]
    fn input_bytes_handles_empty_null() {
        let bytes = unsafe { InputBytes::from_raw(ptr::null_mut(), ptr::null(), 0).unwrap() };
        assert_eq!(bytes.as_slice(), &[]);
        assert_eq!(bytes.as_optional(), None);
    }

    #[test]
    fn input_bytes_rejects_null_nonzero_and_updates_last_error() {
        let mut device = unsafe { std::mem::zeroed::<nfc_device>() };
        let status = unsafe { InputBytes::from_raw(&mut device, ptr::null(), 1).unwrap_err() };
        assert_eq!(status, crate::bridge::NFC_EINVARG);
        assert_eq!(device.last_error, crate::bridge::NFC_EINVARG);
    }

    #[test]
    fn parity_markers_support_optional_pointers() {
        let tx_parity = 0xAAu8;
        let marker = unsafe { ParityMarker::from_raw(ptr::addr_of!(tx_parity)) };
        assert_eq!(marker.as_deref(), Some(&[0xAA][..]));
        assert_eq!(
            unsafe { ParityMarker::from_raw(ptr::null()) }.as_deref(),
            None
        );

        let mut rx_parity = 0u8;
        let mut marker_mut = unsafe { ParityMarkerMut::from_raw(ptr::addr_of_mut!(rx_parity)) };
        marker_mut.as_deref_mut().unwrap()[0] = 0x55;
        assert_eq!(rx_parity, 0x55);
        let mut null_marker = unsafe { ParityMarkerMut::from_raw(ptr::null_mut()) };
        assert_eq!(null_marker.as_deref_mut(), None);
    }

    #[test]
    fn decode_modulations_requires_pointer_when_length_is_nonzero() {
        let mut device = unsafe { std::mem::zeroed::<nfc_device>() };
        let status = unsafe { decode_modulations(&mut device, ptr::null(), 1).unwrap_err() };
        assert_eq!(status, crate::bridge::NFC_EINVARG);
        assert_eq!(device.last_error, crate::bridge::NFC_EINVARG);
    }

    #[test]
    fn decode_modulations_converts_entries() {
        let modulations = [nfc_modulation {
            nmt: nfc_modulation_type::NMT_ISO14443A,
            nbr: nfc_baud_rate::NBR_106,
        }];
        let decoded =
            unsafe { decode_modulations(ptr::null_mut(), modulations.as_ptr(), 1) }.unwrap();
        assert_eq!(
            decoded,
            vec![rt::Modulation {
                modulation_type: rt::ModulationType::Iso14443A,
                baud_rate: rt::BaudRate::Br106,
            }]
        );
    }

    #[test]
    fn optional_decoders_accept_null() {
        assert_eq!(unsafe { decode_optional_dep_info(ptr::null()) }, None);
        assert_eq!(unsafe { decode_optional_target(ptr::null()) }, None);

        let dep = nfc_dep_info {
            ndm: nfc_dep_mode::NDM_ACTIVE,
            abtNFCID3: [1; 10],
            btDID: 2,
            btBS: 3,
            btBR: 4,
            btTO: 5,
            btPP: 6,
            abtGB: [0; 48],
            szGB: 0,
        };
        let decoded = unsafe { decode_optional_dep_info(ptr::addr_of!(dep)) }.unwrap();
        assert_eq!(decoded.mode, rt::DepMode::Active);
        assert_eq!(decoded.did, 2);
    }
}
