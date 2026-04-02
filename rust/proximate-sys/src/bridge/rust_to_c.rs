use crate::c_api_impl::NFC_BUFSIZE_CONNSTRING;
use crate::ffi_support::{as_mut, copy_bytes_to_c_buffer};
use crate::ffi_types::{
    nfc_barcode_info, nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_felica_info,
    nfc_iso14443a_info, nfc_iso14443b_info, nfc_iso14443b2ct_info, nfc_iso14443b2sr_info,
    nfc_iso14443bi_info, nfc_iso14443biclass_info, nfc_jewel_info, nfc_mode, nfc_modulation,
    nfc_modulation_type, nfc_property, nfc_target, nfc_target_info,
};
use crate::lifecycle::{
    DEVICE_NAME_LENGTH, MAX_USER_DEFINED_DEVICES, nfc_context, nfc_user_defined_device,
};
use proximate_driver as rt;
use std::ptr;

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
