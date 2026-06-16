// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// ABI mirrors for packed public NFC types from include/nfc/nfc-types.h.
#![allow(dead_code, non_camel_case_types, non_snake_case)]

use libc::size_t;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub enum nfc_property {
    NP_TIMEOUT_COMMAND = 0,
    NP_TIMEOUT_ATR = 1,
    NP_TIMEOUT_COM = 2,
    NP_HANDLE_CRC = 3,
    NP_HANDLE_PARITY = 4,
    NP_ACTIVATE_FIELD = 5,
    NP_ACTIVATE_CRYPTO1 = 6,
    NP_INFINITE_SELECT = 7,
    NP_ACCEPT_INVALID_FRAMES = 8,
    NP_ACCEPT_MULTIPLE_FRAMES = 9,
    NP_AUTO_ISO14443_4 = 10,
    NP_EASY_FRAMING = 11,
    NP_FORCE_ISO14443_A = 12,
    NP_FORCE_ISO14443_B = 13,
    NP_FORCE_SPEED_106 = 14,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub enum nfc_dep_mode {
    NDM_UNDEFINED = 0,
    NDM_PASSIVE = 1,
    NDM_ACTIVE = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub enum nfc_baud_rate {
    NBR_UNDEFINED = 0,
    NBR_106 = 1,
    NBR_212 = 2,
    NBR_424 = 3,
    NBR_847 = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub enum nfc_modulation_type {
    NMT_UNDEFINED = 0,
    NMT_ISO14443A = 1,
    NMT_JEWEL = 2,
    NMT_ISO14443B = 3,
    NMT_ISO14443BI = 4,
    NMT_ISO14443B2SR = 5,
    NMT_ISO14443B2CT = 6,
    NMT_FELICA = 7,
    NMT_DEP = 8,
    NMT_BARCODE = 9,
    NMT_ISO14443BICLASS = 10,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub enum nfc_mode {
    N_TARGET = 0,
    N_INITIATOR = 1,
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct nfc_dep_info {
    pub abtNFCID3: [u8; 10],
    pub btDID: u8,
    pub btBS: u8,
    pub btBR: u8,
    pub btTO: u8,
    pub btPP: u8,
    pub abtGB: [u8; 48],
    pub szGB: size_t,
    pub ndm: nfc_dep_mode,
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct nfc_iso14443a_info {
    pub abtAtqa: [u8; 2],
    pub btSak: u8,
    pub szUidLen: size_t,
    pub abtUid: [u8; 10],
    pub szAtsLen: size_t,
    pub abtAts: [u8; 254],
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct nfc_felica_info {
    pub szLen: size_t,
    pub btResCode: u8,
    pub abtId: [u8; 8],
    pub abtPad: [u8; 8],
    pub abtSysCode: [u8; 2],
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct nfc_iso14443b_info {
    pub abtPupi: [u8; 4],
    pub abtApplicationData: [u8; 4],
    pub abtProtocolInfo: [u8; 3],
    pub ui8CardIdentifier: u8,
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct nfc_iso14443bi_info {
    pub abtDIV: [u8; 4],
    pub btVerLog: u8,
    pub btConfig: u8,
    pub szAtrLen: size_t,
    pub abtAtr: [u8; 33],
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct nfc_iso14443biclass_info {
    pub abtUID: [u8; 8],
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct nfc_iso14443b2sr_info {
    pub abtUID: [u8; 8],
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct nfc_iso14443b2ct_info {
    pub abtUID: [u8; 4],
    pub btProdCode: u8,
    pub btFabCode: u8,
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct nfc_jewel_info {
    pub btSensRes: [u8; 2],
    pub btId: [u8; 4],
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct nfc_barcode_info {
    pub szDataLen: size_t,
    pub abtData: [u8; 32],
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub union nfc_target_info {
    pub nai: nfc_iso14443a_info,
    pub nfi: nfc_felica_info,
    pub nbi: nfc_iso14443b_info,
    pub nii: nfc_iso14443bi_info,
    pub nsi: nfc_iso14443b2sr_info,
    pub nci: nfc_iso14443b2ct_info,
    pub nji: nfc_jewel_info,
    pub ndi: nfc_dep_info,
    pub nti: nfc_barcode_info,
    pub nhi: nfc_iso14443biclass_info,
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct nfc_modulation {
    pub nmt: nfc_modulation_type,
    pub nbr: nfc_baud_rate,
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct nfc_target {
    pub nti: nfc_target_info,
    pub nm: nfc_modulation,
}
