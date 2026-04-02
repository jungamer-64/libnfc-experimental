use crate::ffi_support::{as_ref, fixed_c_buffer_to_string};
use crate::ffi_types::{
    nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_modulation, nfc_modulation_type, nfc_property,
    nfc_target, nfc_target_info,
};
use crate::lifecycle::{MAX_USER_DEFINED_DEVICES, nfc_context};
use proximate_driver as rt;
use std::ptr;

pub(crate) fn context_from_c(context: *const nfc_context) -> rt::Context {
    let Some(context_ref) = (unsafe { as_ref(context) }) else {
        return rt::Context::default();
    };

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

    rt::Context::with_config(rt::ContextConfig {
        allow_autoscan: context_ref.allow_autoscan,
        allow_intrusive_scan: context_ref.allow_intrusive_scan,
        log_level: context_ref.log_level,
        user_defined_devices,
    })
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
