// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// ABI mirrors intentionally keep libnfc's public C names and exported layout.
#![allow(dead_code, non_camel_case_types, non_snake_case)]

use libc::{c_int, size_t};

macro_rules! c_enum_carrier {
    ($name:ident { $($constant:ident = $value:expr),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[repr(transparent)]
        pub struct $name(c_int);

        impl $name {
            $(pub const $constant: Self = Self($value);)+

            /// Constructs the ABI carrier without assuming the integer is a
            /// known C enumerator. Semantic validation happens at the boundary.
            pub const fn from_raw(raw: c_int) -> Self {
                Self(raw)
            }

            pub const fn raw(self) -> c_int {
                self.0
            }
        }
    };
}

c_enum_carrier!(nfc_property {
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
});

c_enum_carrier!(nfc_dep_mode {
    NDM_UNDEFINED = 0,
    NDM_PASSIVE = 1,
    NDM_ACTIVE = 2,
});

c_enum_carrier!(nfc_baud_rate {
    NBR_UNDEFINED = 0,
    NBR_106 = 1,
    NBR_212 = 2,
    NBR_424 = 3,
    NBR_847 = 4,
});

c_enum_carrier!(nfc_modulation_type {
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
});

c_enum_carrier!(nfc_mode {
    N_TARGET = 0,
    N_INITIATOR = 1,
});

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn public_enum_values_and_representation_match_the_c_abi() {
        assert_eq!(size_of::<nfc_property>(), 4);
        assert_eq!(nfc_property::NP_TIMEOUT_COMMAND.raw(), 0);
        assert_eq!(nfc_property::NP_FORCE_SPEED_106.raw(), 14);

        assert_eq!(size_of::<nfc_dep_mode>(), 4);
        assert_eq!(nfc_dep_mode::NDM_UNDEFINED.raw(), 0);
        assert_eq!(nfc_dep_mode::NDM_PASSIVE.raw(), 1);
        assert_eq!(nfc_dep_mode::NDM_ACTIVE.raw(), 2);

        assert_eq!(size_of::<nfc_baud_rate>(), 4);
        assert_eq!(nfc_baud_rate::NBR_UNDEFINED.raw(), 0);
        assert_eq!(nfc_baud_rate::NBR_106.raw(), 1);
        assert_eq!(nfc_baud_rate::NBR_212.raw(), 2);
        assert_eq!(nfc_baud_rate::NBR_424.raw(), 3);
        assert_eq!(nfc_baud_rate::NBR_847.raw(), 4);

        assert_eq!(size_of::<nfc_modulation_type>(), 4);
        assert_eq!(nfc_modulation_type::NMT_UNDEFINED.raw(), 0);
        assert_eq!(nfc_modulation_type::NMT_ISO14443A.raw(), 1);
        assert_eq!(nfc_modulation_type::NMT_JEWEL.raw(), 2);
        assert_eq!(nfc_modulation_type::NMT_ISO14443B.raw(), 3);
        assert_eq!(nfc_modulation_type::NMT_ISO14443BI.raw(), 4);
        assert_eq!(nfc_modulation_type::NMT_ISO14443B2SR.raw(), 5);
        assert_eq!(nfc_modulation_type::NMT_ISO14443B2CT.raw(), 6);
        assert_eq!(nfc_modulation_type::NMT_FELICA.raw(), 7);
        assert_eq!(nfc_modulation_type::NMT_DEP.raw(), 8);
        assert_eq!(nfc_modulation_type::NMT_BARCODE.raw(), 9);
        assert_eq!(nfc_modulation_type::NMT_ISO14443BICLASS.raw(), 10);

        assert_eq!(size_of::<nfc_mode>(), 4);
        assert_eq!(nfc_mode::N_TARGET.raw(), 0);
        assert_eq!(nfc_mode::N_INITIATOR.raw(), 1);
    }

    #[test]
    fn public_packed_structs_and_union_match_the_c_abi() {
        let word = size_of::<size_t>();

        assert_eq!(align_of::<nfc_dep_info>(), 1);
        assert_eq!(size_of::<nfc_dep_info>(), 67 + word);
        assert_eq!(offset_of!(nfc_dep_info, abtNFCID3), 0);
        assert_eq!(offset_of!(nfc_dep_info, btDID), 10);
        assert_eq!(offset_of!(nfc_dep_info, btBS), 11);
        assert_eq!(offset_of!(nfc_dep_info, btBR), 12);
        assert_eq!(offset_of!(nfc_dep_info, btTO), 13);
        assert_eq!(offset_of!(nfc_dep_info, btPP), 14);
        assert_eq!(offset_of!(nfc_dep_info, abtGB), 15);
        assert_eq!(offset_of!(nfc_dep_info, szGB), 63);
        assert_eq!(offset_of!(nfc_dep_info, ndm), 63 + word);

        assert_eq!(align_of::<nfc_iso14443a_info>(), 1);
        assert_eq!(size_of::<nfc_iso14443a_info>(), 267 + 2 * word);
        assert_eq!(offset_of!(nfc_iso14443a_info, abtAtqa), 0);
        assert_eq!(offset_of!(nfc_iso14443a_info, btSak), 2);
        assert_eq!(offset_of!(nfc_iso14443a_info, szUidLen), 3);
        assert_eq!(offset_of!(nfc_iso14443a_info, abtUid), 3 + word);
        assert_eq!(offset_of!(nfc_iso14443a_info, szAtsLen), 13 + word);
        assert_eq!(offset_of!(nfc_iso14443a_info, abtAts), 13 + 2 * word);

        assert_eq!(size_of::<nfc_felica_info>(), 19 + word);
        assert_eq!(offset_of!(nfc_felica_info, szLen), 0);
        assert_eq!(offset_of!(nfc_felica_info, btResCode), word);
        assert_eq!(offset_of!(nfc_felica_info, abtId), word + 1);
        assert_eq!(offset_of!(nfc_felica_info, abtPad), word + 9);
        assert_eq!(offset_of!(nfc_felica_info, abtSysCode), word + 17);

        assert_eq!(size_of::<nfc_iso14443b_info>(), 12);
        assert_eq!(offset_of!(nfc_iso14443b_info, abtPupi), 0);
        assert_eq!(offset_of!(nfc_iso14443b_info, abtApplicationData), 4);
        assert_eq!(offset_of!(nfc_iso14443b_info, abtProtocolInfo), 8);
        assert_eq!(offset_of!(nfc_iso14443b_info, ui8CardIdentifier), 11);

        assert_eq!(size_of::<nfc_iso14443bi_info>(), 39 + word);
        assert_eq!(offset_of!(nfc_iso14443bi_info, abtDIV), 0);
        assert_eq!(offset_of!(nfc_iso14443bi_info, btVerLog), 4);
        assert_eq!(offset_of!(nfc_iso14443bi_info, btConfig), 5);
        assert_eq!(offset_of!(nfc_iso14443bi_info, szAtrLen), 6);
        assert_eq!(offset_of!(nfc_iso14443bi_info, abtAtr), 6 + word);

        assert_eq!(size_of::<nfc_iso14443biclass_info>(), 8);
        assert_eq!(offset_of!(nfc_iso14443biclass_info, abtUID), 0);
        assert_eq!(size_of::<nfc_iso14443b2sr_info>(), 8);
        assert_eq!(offset_of!(nfc_iso14443b2sr_info, abtUID), 0);
        assert_eq!(size_of::<nfc_iso14443b2ct_info>(), 6);
        assert_eq!(offset_of!(nfc_iso14443b2ct_info, abtUID), 0);
        assert_eq!(offset_of!(nfc_iso14443b2ct_info, btProdCode), 4);
        assert_eq!(offset_of!(nfc_iso14443b2ct_info, btFabCode), 5);
        assert_eq!(size_of::<nfc_jewel_info>(), 6);
        assert_eq!(offset_of!(nfc_jewel_info, btSensRes), 0);
        assert_eq!(offset_of!(nfc_jewel_info, btId), 2);

        assert_eq!(size_of::<nfc_barcode_info>(), 32 + word);
        assert_eq!(offset_of!(nfc_barcode_info, szDataLen), 0);
        assert_eq!(offset_of!(nfc_barcode_info, abtData), word);

        assert_eq!(align_of::<nfc_target_info>(), 1);
        assert_eq!(
            size_of::<nfc_target_info>(),
            size_of::<nfc_iso14443a_info>()
        );
        assert_eq!(offset_of!(nfc_target_info, nai), 0);
        assert_eq!(offset_of!(nfc_target_info, nfi), 0);
        assert_eq!(offset_of!(nfc_target_info, nbi), 0);
        assert_eq!(offset_of!(nfc_target_info, nii), 0);
        assert_eq!(offset_of!(nfc_target_info, nsi), 0);
        assert_eq!(offset_of!(nfc_target_info, nci), 0);
        assert_eq!(offset_of!(nfc_target_info, nji), 0);
        assert_eq!(offset_of!(nfc_target_info, ndi), 0);
        assert_eq!(offset_of!(nfc_target_info, nti), 0);
        assert_eq!(offset_of!(nfc_target_info, nhi), 0);

        assert_eq!(align_of::<nfc_modulation>(), 1);
        assert_eq!(size_of::<nfc_modulation>(), 8);
        assert_eq!(offset_of!(nfc_modulation, nmt), 0);
        assert_eq!(offset_of!(nfc_modulation, nbr), 4);

        assert_eq!(align_of::<nfc_target>(), 1);
        assert_eq!(
            size_of::<nfc_target>(),
            size_of::<nfc_target_info>() + size_of::<nfc_modulation>()
        );
        assert_eq!(offset_of!(nfc_target, nti), 0);
        assert_eq!(offset_of!(nfc_target, nm), size_of::<nfc_target_info>());
    }
}
