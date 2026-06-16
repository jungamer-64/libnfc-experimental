use super::{ICC_TYPE_UNKNOWN, NFC_EDEVNOTSUPP, device_error};
use proximate_driver::Error;

pub(super) fn attr_to_string(value: &[u8]) -> Option<String> {
    let end = value
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(value.len());
    if end == 0 {
        None
    } else {
        Some(String::from_utf8_lossy(&value[..end]).into_owned())
    }
}

pub(super) fn is_feitian_reader(name: &str) -> bool {
    let lowercase = name.to_ascii_lowercase();
    lowercase.contains("feitian")
}

pub(super) fn command_response_data<'a>(
    response: &'a [u8],
    operation: &'static str,
) -> Result<&'a [u8], Error> {
    if response.len() < 2 {
        return Err(device_error(operation, NFC_EDEVNOTSUPP));
    }
    Ok(&response[..response.len() - 2])
}

pub(super) fn icc_type_matches(icc_type: u8, expected_type: u8) -> bool {
    icc_type == ICC_TYPE_UNKNOWN || icc_type == expected_type
}

pub(super) fn iso14443a_uid_length_valid(uid_length: usize) -> bool {
    matches!(uid_length, 0 | 4 | 7 | 10)
}

pub(super) fn iso14443a_atr_valid(atr: &[u8]) -> bool {
    atr.len() >= 5
        && atr[0] == 0x3B
        && atr[1] == (0x80 | (atr.len() as u8 - 5))
        && atr[2] == 0x80
        && atr[3] == 0x01
}

pub(super) fn iso14443b_uid_length_valid(uid_length: usize) -> bool {
    matches!(uid_length, 0 | 8)
}

pub(super) fn iso14443b_atr_valid(atr: &[u8]) -> bool {
    atr.len() == 13 && atr[0] == 0x3B && atr[1] == (0x80 | 0x08) && atr[2] == 0x80 && atr[3] == 0x01
}
