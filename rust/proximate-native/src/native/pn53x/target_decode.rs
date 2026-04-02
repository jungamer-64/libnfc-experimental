use super::*;

pub(super) fn cascade_iso14443a_uid(uid: &[u8]) -> Vec<u8> {
    match uid.len() {
        4 => uid.to_vec(),
        7 => {
            let mut cascaded = Vec::with_capacity(8);
            cascaded.extend_from_slice(&[0x88, uid[0], uid[1], uid[2]]);
            cascaded.extend_from_slice(&uid[3..]);
            cascaded
        }
        10 => {
            let mut cascaded = Vec::with_capacity(12);
            cascaded.extend_from_slice(&[0x88, uid[0], uid[1], uid[2]]);
            cascaded.extend_from_slice(&[0x88, uid[3], uid[4], uid[5]]);
            cascaded.extend_from_slice(&uid[6..]);
            cascaded
        }
        _ => Vec::new(),
    }
}

pub(super) fn default_initiator_payload(modulation: Modulation) -> &'static [u8] {
    match modulation.modulation_type {
        ModulationType::Iso14443B => &[0x00],
        ModulationType::Felica => &[0x00, 0xff, 0xff, 0x01, 0x00],
        _ => &[],
    }
}

pub(super) fn nm_to_pm(modulation: Modulation) -> Option<u8> {
    match (modulation.modulation_type, modulation.baud_rate) {
        (ModulationType::Iso14443A, _) => Some(0x00),
        (ModulationType::Felica, BaudRate::Br212) => Some(0x01),
        (ModulationType::Felica, BaudRate::Br424) => Some(0x02),
        (ModulationType::Iso14443B, BaudRate::Br106) => Some(0x03),
        (ModulationType::Jewel, _) => Some(0x04),
        _ => None,
    }
}

pub(super) fn nm_to_ptt(modulation: Modulation) -> Option<u8> {
    match (modulation.modulation_type, modulation.baud_rate) {
        (ModulationType::Iso14443A, _) => Some(0x10),
        (ModulationType::Iso14443B, BaudRate::Br106) => Some(0x03),
        (ModulationType::Jewel, _) => Some(0x04),
        (ModulationType::Felica, BaudRate::Br212) => Some(0x11),
        (ModulationType::Felica, BaudRate::Br424) => Some(0x12),
        _ => None,
    }
}

#[allow(dead_code)]
fn ptt_to_modulation(value: u8) -> Modulation {
    match value {
        0x03 | 0x23 => Modulation {
            modulation_type: ModulationType::Iso14443B,
            baud_rate: BaudRate::Br106,
        },
        0x04 => Modulation {
            modulation_type: ModulationType::Jewel,
            baud_rate: BaudRate::Br106,
        },
        0x11 => Modulation {
            modulation_type: ModulationType::Felica,
            baud_rate: BaudRate::Br212,
        },
        0x12 => Modulation {
            modulation_type: ModulationType::Felica,
            baud_rate: BaudRate::Br424,
        },
        0x40 | 0x80 => Modulation {
            modulation_type: ModulationType::Dep,
            baud_rate: BaudRate::Br106,
        },
        0x41 | 0x81 => Modulation {
            modulation_type: ModulationType::Dep,
            baud_rate: BaudRate::Br212,
        },
        0x42 | 0x82 => Modulation {
            modulation_type: ModulationType::Dep,
            baud_rate: BaudRate::Br424,
        },
        _ => Modulation {
            modulation_type: ModulationType::Iso14443A,
            baud_rate: BaudRate::Br106,
        },
    }
}

fn process_cascade_uid(uid: &[u8]) -> Vec<u8> {
    match uid {
        [0x88, a, b, c, tail @ ..] if tail.len() == 4 => {
            let mut real = vec![*a, *b, *c];
            real.extend_from_slice(tail);
            real
        }
        [0x88, a, b, c, 0x88, d, e, f, tail @ ..] if tail.len() == 4 => {
            let mut real = vec![*a, *b, *c, *d, *e, *f];
            real.extend_from_slice(tail);
            real
        }
        value => value.to_vec(),
    }
}

pub(super) fn decode_target_data(
    chip_type: Pn53xType,
    modulation: Modulation,
    raw: &[u8],
) -> Result<Target, Error> {
    let info = match modulation.modulation_type {
        ModulationType::Iso14443A => decode_iso14443a_target(chip_type, raw)?,
        ModulationType::Iso14443B => decode_iso14443b_target(raw)?,
        ModulationType::Felica => decode_felica_target(raw)?,
        ModulationType::Jewel => decode_jewel_target(raw)?,
        _ => return Err(Error::UnsupportedOperation("decode_target_data")),
    };
    Ok(Target { modulation, info })
}

fn decode_iso14443a_target(chip_type: Pn53xType, raw: &[u8]) -> Result<TargetInfo, Error> {
    if raw.len() < 5 {
        return Err(status_error("decode_iso14443a_target", NFC_EIO));
    }

    let mut offset = 1;
    let atqa = if chip_type == Pn53xType::Pn531 {
        let value = [raw[offset + 1], raw[offset]];
        offset += 2;
        value
    } else {
        let value = [raw[offset], raw[offset + 1]];
        offset += 2;
        value
    };
    let sak = raw[offset];
    offset += 1;
    let uid_len = raw[offset] as usize;
    offset += 1;
    if raw.len() < offset + uid_len {
        return Err(status_error("decode_iso14443a_target", NFC_EIO));
    }
    let uid = process_cascade_uid(&raw[offset..offset + uid_len]);
    offset += uid_len;

    let mut ats = Vec::new();
    if let Some(&ats_header) = raw.get(offset) {
        offset += 1;
        if ats_header > 1 {
            let ats_len = usize::from(ats_header - 1);
            if raw.len() < offset + ats_len {
                return Err(status_error("decode_iso14443a_target", NFC_EIO));
            }
            ats.extend_from_slice(&raw[offset..offset + ats_len]);
        }
    }

    Ok(TargetInfo::Iso14443A {
        atqa,
        sak,
        uid,
        ats,
    })
}

fn decode_iso14443b_target(raw: &[u8]) -> Result<TargetInfo, Error> {
    if raw.len() < 13 {
        return Err(status_error("decode_iso14443b_target", NFC_EIO));
    }
    let mut offset = 2;
    let mut pupi = [0u8; 4];
    pupi.copy_from_slice(&raw[offset..offset + 4]);
    offset += 4;
    let mut application_data = [0u8; 4];
    application_data.copy_from_slice(&raw[offset..offset + 4]);
    offset += 4;
    let mut protocol_info = [0u8; 3];
    protocol_info.copy_from_slice(&raw[offset..offset + 3]);
    offset += 3;
    let card_identifier = if raw.len() > offset + 1 && raw[offset] > 0 {
        raw[offset + 1]
    } else {
        0
    };
    Ok(TargetInfo::Iso14443B {
        pupi,
        application_data,
        protocol_info,
        card_identifier,
    })
}

fn decode_felica_target(raw: &[u8]) -> Result<TargetInfo, Error> {
    if raw.len() < 19 {
        return Err(status_error("decode_felica_target", NFC_EIO));
    }
    let len = raw[1] as usize;
    let response_code = raw[2];
    let mut id = [0u8; 8];
    id.copy_from_slice(&raw[3..11]);
    let mut pad = [0u8; 8];
    pad.copy_from_slice(&raw[11..19]);
    let mut system_code = [0u8; 2];
    if len > 18 && raw.len() >= 21 {
        system_code.copy_from_slice(&raw[19..21]);
    }
    Ok(TargetInfo::Felica {
        len,
        response_code,
        id,
        pad,
        system_code,
    })
}

fn decode_jewel_target(raw: &[u8]) -> Result<TargetInfo, Error> {
    if raw.len() < 7 {
        return Err(status_error("decode_jewel_target", NFC_EIO));
    }
    let mut sens_res = [0u8; 2];
    sens_res.copy_from_slice(&raw[1..3]);
    let mut id = [0u8; 4];
    id.copy_from_slice(&raw[3..7]);
    Ok(TargetInfo::Jewel { sens_res, id })
}

pub(super) fn build_injump_for_dep_command(
    mode: DepMode,
    baud_rate: BaudRate,
    initiator: Option<&DepInfo>,
) -> Result<Vec<u8>, Error> {
    let (baud_code, passive_initiator) = match baud_rate {
        BaudRate::Br106 => (0x00, None),
        BaudRate::Br212 => (0x01, Some(&[0x00, 0xff, 0xff, 0x00, 0x0f][..])),
        BaudRate::Br424 => (0x02, Some(&[0x00, 0xff, 0xff, 0x00, 0x0f][..])),
        _ => return Err(Error::InvalidArgument("baud_rate")),
    };

    let mut payload = vec![
        if mode == DepMode::Active { 0x01 } else { 0x00 },
        baud_code,
        0x00,
    ];

    if mode == DepMode::Passive
        && let Some(passive) = passive_initiator
    {
        payload[2] |= 0x01;
        payload.extend_from_slice(passive);
    }

    if let Some(initiator) = initiator {
        payload[2] |= 0x02;
        payload.extend_from_slice(&initiator.nfcid3);
        if !initiator.general_bytes.is_empty() {
            payload[2] |= 0x04;
            payload.extend_from_slice(&initiator.general_bytes);
        }
    }

    Ok(payload)
}

pub(super) fn parse_dep_target(
    payload: &[u8],
    mode: DepMode,
    baud_rate: BaudRate,
) -> Result<Option<Target>, Error> {
    if payload.is_empty() {
        return Err(status_error("parse_dep_target", NFC_EIO));
    }
    if payload[0] == 0 {
        return Ok(None);
    }
    if payload.len() < 16 {
        return Err(status_error("parse_dep_target", NFC_EIO));
    }
    let mut nfcid3 = [0u8; 10];
    nfcid3.copy_from_slice(&payload[1..11]);
    let general_bytes = if payload.len() > 16 {
        payload[16..].to_vec()
    } else {
        Vec::new()
    };
    Ok(Some(Target {
        modulation: Modulation {
            modulation_type: ModulationType::Dep,
            baud_rate,
        },
        info: TargetInfo::Dep(DepInfo {
            nfcid3,
            did: payload[11],
            bs: payload[12],
            br: payload[13],
            timeout: payload[14],
            pp: payload[15],
            general_bytes,
            mode,
        }),
    }))
}

pub(super) fn is_iso14443_4_target(target: &Target) -> bool {
    matches!(
        target.info,
        TargetInfo::Iso14443A { sak, .. } if sak & SAK_ISO14443_4_COMPLIANT != 0
    )
}

pub(super) fn build_target_init_command(
    chip_type: Pn53xType,
    properties: PropertyState,
    target: &Target,
) -> Result<Vec<u8>, Error> {
    let mut command = vec![0u8; 39 + 47 + 48];
    command[0] = PN53X_TG_INIT_AS_TARGET;
    let mut target_mode = PN53X_TARGET_MODE_NORMAL;
    let optional_bytes;

    match &target.info {
        TargetInfo::Iso14443A { atqa, sak, uid, .. } => {
            if uid.len() != 4 || uid[0] != 0x08 {
                return Err(Error::InvalidArgument("target.uid"));
            }
            target_mode |= PN53X_TARGET_MODE_PASSIVE_ONLY;
            if chip_type == Pn53xType::Pn532
                && properties.auto_iso14443_4
                && sak & SAK_ISO14443_4_COMPLIANT != 0
            {
                target_mode |= PN53X_TARGET_MODE_ISO14443_4_PICC_ONLY;
            }
            command[2] = atqa[1];
            command[3] = atqa[0];
            command[4] = uid[1];
            command[5] = uid[2];
            command[6] = uid[3];
            command[7] = *sak;
            command[36] = 0;
            optional_bytes = 2;
        }
        TargetInfo::Felica {
            id,
            pad,
            system_code,
            ..
        } => {
            target_mode |= PN53X_TARGET_MODE_PASSIVE_ONLY;
            command[8..16].copy_from_slice(id);
            command[16..24].copy_from_slice(pad);
            command[24..26].copy_from_slice(system_code);
            command[36] = 0;
            optional_bytes = 2;
        }
        TargetInfo::Dep(dep) => {
            target_mode |= PN53X_TARGET_MODE_DEP_ONLY;
            if dep.mode == DepMode::Passive {
                target_mode |= PN53X_TARGET_MODE_PASSIVE_ONLY;
            }
            command[2] = 0x08;
            command[3] = 0x00;
            command[4] = 0x12;
            command[5] = 0x34;
            command[6] = 0x56;
            command[7] = 0x40;
            command[8..16].copy_from_slice(&[0x01, 0xfe, 0x12, 0x34, 0x56, 0x78, 0x90, 0x12]);
            command[16..24].copy_from_slice(&[0xc0, 0xc1, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7]);
            command[24..26].copy_from_slice(&[0x0f, 0xab]);
            command[26..36].copy_from_slice(&dep.nfcid3);
            let gb_len = dep.general_bytes.len().min(47);
            command[36] = gb_len as u8;
            command[37..37 + gb_len].copy_from_slice(&dep.general_bytes[..gb_len]);
            command[37 + gb_len] = 0;
            optional_bytes = gb_len + 2;
        }
        _ => return Err(Error::UnsupportedOperation("target_init")),
    }

    command[1] = target_mode;
    command.truncate(36 + optional_bytes);
    Ok(command)
}

pub(super) fn decode_activation_mode(mode: u8) -> (Modulation, DepMode) {
    let baud_rate = match mode & 0x70 {
        0x10 => BaudRate::Br212,
        0x20 => BaudRate::Br424,
        _ => BaudRate::Br106,
    };
    if mode & 0x04 != 0 {
        let dep_mode = if mode & 0x03 == 0x01 {
            DepMode::Active
        } else {
            DepMode::Passive
        };
        (
            Modulation {
                modulation_type: ModulationType::Dep,
                baud_rate,
            },
            dep_mode,
        )
    } else {
        let modulation_type = if mode & 0x03 == 0x02 {
            ModulationType::Felica
        } else {
            ModulationType::Iso14443A
        };
        (
            Modulation {
                modulation_type,
                baud_rate,
            },
            DepMode::Undefined,
        )
    }
}
