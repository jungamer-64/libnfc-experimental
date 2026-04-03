use crate::nci::TagInfo;
use proximate_driver::{Modulation, ModulationType, Target, TargetInfo};

use super::consts::{
    DESFIRE_ATS, NFA_PROTOCOL_T1T, TARGET_TYPE_FELICA, TARGET_TYPE_ISO14443_3A,
    TARGET_TYPE_ISO14443_3B, TARGET_TYPE_ISO14443_4, TARGET_TYPE_MIFARE_CLASSIC,
    TARGET_TYPE_MIFARE_UL,
};

pub(super) fn technology_matches(tag: &TagInfo, modulation: ModulationType) -> bool {
    match modulation {
        ModulationType::Iso14443A => matches!(
            tag.technology,
            TARGET_TYPE_ISO14443_4
                | TARGET_TYPE_ISO14443_3A
                | TARGET_TYPE_MIFARE_CLASSIC
                | TARGET_TYPE_MIFARE_UL
        ),
        ModulationType::Iso14443B
        | ModulationType::Iso14443Bi
        | ModulationType::Iso14443B2Sr
        | ModulationType::Iso14443B2Ct => tag.technology == TARGET_TYPE_ISO14443_3B,
        ModulationType::Felica => tag.technology == TARGET_TYPE_FELICA,
        ModulationType::Jewel => {
            tag.technology == TARGET_TYPE_ISO14443_3A && tag.protocol == NFA_PROTOCOL_T1T
        }
        _ => false,
    }
}

pub(super) fn build_target(tag: &TagInfo, modulation: Modulation) -> Option<Target> {
    if !technology_matches(tag, modulation.modulation_type) {
        return None;
    }

    let uid_len = (tag.uid_length as usize).min(tag.uid.len());
    if uid_len == 0 {
        return None;
    }

    let target = match modulation.modulation_type {
        ModulationType::Iso14443A => Target {
            modulation,
            info: TargetInfo::Iso14443A {
                atqa: [0x00, 0x00],
                sak: if tag.technology == TARGET_TYPE_MIFARE_CLASSIC {
                    0x08
                } else {
                    0x20
                },
                uid: tag.uid[..uid_len].to_vec(),
                ats: if tag.technology == TARGET_TYPE_MIFARE_CLASSIC {
                    Vec::new()
                } else {
                    DESFIRE_ATS.to_vec()
                },
            },
        },
        ModulationType::Iso14443B => {
            let mut pupi = [0u8; 4];
            let copy_len = uid_len.min(pupi.len());
            pupi[..copy_len].copy_from_slice(&tag.uid[..copy_len]);
            Target {
                modulation,
                info: TargetInfo::Iso14443B {
                    pupi,
                    application_data: [0; 4],
                    protocol_info: [0; 3],
                    card_identifier: 0,
                },
            }
        }
        ModulationType::Iso14443Bi => {
            let mut div = [0u8; 4];
            let copy_len = uid_len.min(div.len());
            div[..copy_len].copy_from_slice(&tag.uid[..copy_len]);
            Target {
                modulation,
                info: TargetInfo::Iso14443Bi {
                    div,
                    version_log: 0,
                    config: 0,
                    atr: Vec::new(),
                },
            }
        }
        ModulationType::Iso14443B2Sr => {
            let mut uid = [0u8; 8];
            let copy_len = uid_len.min(uid.len());
            uid[..copy_len].copy_from_slice(&tag.uid[..copy_len]);
            Target {
                modulation,
                info: TargetInfo::Iso14443B2Sr { uid },
            }
        }
        ModulationType::Iso14443B2Ct => {
            let mut uid = [0u8; 4];
            let copy_len = uid_len.min(uid.len());
            uid[..copy_len].copy_from_slice(&tag.uid[..copy_len]);
            Target {
                modulation,
                info: TargetInfo::Iso14443B2Ct {
                    uid,
                    product_code: 0,
                    fabrication_code: 0,
                },
            }
        }
        ModulationType::Felica => {
            let mut id = [0u8; 8];
            let copy_len = uid_len.min(id.len());
            id[..copy_len].copy_from_slice(&tag.uid[..copy_len]);
            Target {
                modulation,
                info: TargetInfo::Felica {
                    len: copy_len,
                    response_code: 0,
                    id,
                    pad: [0; 8],
                    system_code: [0; 2],
                },
            }
        }
        ModulationType::Jewel => {
            let mut id = [0u8; 4];
            let copy_len = uid_len.min(id.len());
            id[..copy_len].copy_from_slice(&tag.uid[..copy_len]);
            Target {
                modulation,
                info: TargetInfo::Jewel {
                    sens_res: [0; 2],
                    id,
                },
            }
        }
        _ => return None,
    };

    Some(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proximate_driver::{BaudRate, Modulation};

    #[test]
    fn unsupported_modulation_returns_none() {
        let tag = TagInfo {
            technology: TARGET_TYPE_ISO14443_3A,
            handle: 0,
            uid: [0xAA; 32],
            uid_length: 4,
            protocol: 0,
        };
        let modulation = Modulation {
            modulation_type: ModulationType::Dep,
            baud_rate: BaudRate::Br106,
        };
        assert_eq!(build_target(&tag, modulation), None);
    }

    #[test]
    fn zero_uid_length_returns_none() {
        let tag = TagInfo {
            technology: TARGET_TYPE_ISO14443_3A,
            handle: 0,
            uid: [0; 32],
            uid_length: 0,
            protocol: 0,
        };
        let modulation = Modulation {
            modulation_type: ModulationType::Iso14443A,
            baud_rate: BaudRate::Br106,
        };
        assert_eq!(build_target(&tag, modulation), None);
    }
}
