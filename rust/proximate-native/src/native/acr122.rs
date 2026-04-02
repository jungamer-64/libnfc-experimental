use proximate_driver::Error;

#[cfg_attr(not(test), allow(dead_code))]
const ACR122_APDU_CLASS: u8 = 0xFF;
#[cfg_attr(not(test), allow(dead_code))]
const ACR122_APDU_INS_DIRECT_TRANSMIT: u8 = 0x00;
#[cfg_attr(not(test), allow(dead_code))]
const ACR122_APDU_INS_GET_ADDITIONAL_DATA: u8 = 0xC0;
#[cfg_attr(not(test), allow(dead_code))]
const ACR122_APDU_P1_GET_FIRMWARE_VERSION: u8 = 0x48;
#[cfg_attr(not(test), allow(dead_code))]
const ACR122_PN53X_HOST_TO_READER: u8 = 0xD4;
#[cfg_attr(not(test), allow(dead_code))]
const ACR122_SW1_MORE_DATA_AVAILABLE: u8 = 0x61;
#[cfg_attr(not(test), allow(dead_code))]
const ACR122_SW1_WARNING_WITH_NV_CHANGED: u8 = 0x63;
#[cfg_attr(not(test), allow(dead_code))]
const ACR122_SW1_SUCCESS: u8 = 0x90;
#[cfg_attr(not(test), allow(dead_code))]
const ACR122_SW2_SUCCESS: u8 = 0x00;
#[cfg_attr(not(test), allow(dead_code))]
const ACR122_SW2_PN53X_APPLICATION_LEVEL_ERROR: u8 = 0x7F;

#[cfg_attr(not(test), allow(dead_code))]
const PCSC_READER_PREFIXES: &[&str] = &[
    "ACS ACR122",
    "ACS ACR 38U-CCID",
    "ACS ACR38U-CCID",
    "ACS AET65",
    "    CCID USB",
];

#[cfg_attr(not(test), allow(dead_code))]
const USB_DEVICES: &[(u16, u16, &str)] = &[
    (0x072F, 0x2200, "ACS ACR122"),
    (0x072F, 0x90CC, "Touchatag"),
    (0x072F, 0x2214, "ACS ACR1222"),
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Acr122StatusWord {
    pub sw1: u8,
    pub sw2: u8,
    pub more_data_length: u8,
    pub has_more_data: bool,
    pub application_error: bool,
    pub no_reply: bool,
    pub ok: bool,
    pub unexpected: bool,
}

fn invalid_buffer(name: &'static str) -> Error {
    Error::InvalidArgument(name)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn is_usb_device(vendor_id: u16, product_id: u16) -> bool {
    usb_device_name(vendor_id, product_id).is_some()
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn usb_device_name(vendor_id: u16, product_id: u16) -> Option<&'static str> {
    USB_DEVICES
        .iter()
        .find(|(vendor, product, _)| *vendor == vendor_id && *product == product_id)
        .map(|(_, _, name)| *name)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn is_pcsc_reader_name(reader_name: &str) -> bool {
    PCSC_READER_PREFIXES
        .iter()
        .any(|prefix| reader_name.starts_with(prefix))
}

pub(super) fn build_apdu(ins: u8, p1: u8, p2: u8, data: &[u8], le: u8) -> Result<Vec<u8>, Error> {
    if data.len() > u8::MAX as usize {
        return Err(invalid_buffer("data"));
    }

    let mut buffer = Vec::with_capacity(5 + data.len());
    buffer.extend_from_slice(&[ACR122_APDU_CLASS, ins, p1, p2]);
    if data.is_empty() {
        buffer.push(le);
    } else {
        buffer.push(data.len() as u8);
        buffer.extend_from_slice(data);
    }
    Ok(buffer)
}

pub(super) fn build_direct_transmit_apdu(payload: &[u8]) -> Result<Vec<u8>, Error> {
    if payload.len() > (u8::MAX as usize - 1) {
        return Err(invalid_buffer("payload"));
    }

    let mut data = Vec::with_capacity(payload.len() + 1);
    data.push(ACR122_PN53X_HOST_TO_READER);
    data.extend_from_slice(payload);
    build_apdu(ACR122_APDU_INS_DIRECT_TRANSMIT, 0x00, 0x00, &data, 0x00)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn build_get_firmware_version_apdu() -> Result<Vec<u8>, Error> {
    build_apdu(0x00, ACR122_APDU_P1_GET_FIRMWARE_VERSION, 0x00, &[], 0x00)
}

pub(super) fn build_get_additional_data_apdu(le: u8) -> Result<Vec<u8>, Error> {
    build_apdu(ACR122_APDU_INS_GET_ADDITIONAL_DATA, 0x00, 0x00, &[], le)
}

pub(super) fn parse_status_words(status_bytes: &[u8]) -> Option<Acr122StatusWord> {
    if status_bytes.len() < 2 {
        return None;
    }

    let sw1 = status_bytes[0];
    let sw2 = status_bytes[1];
    let has_more_data = sw1 == ACR122_SW1_MORE_DATA_AVAILABLE;
    let application_error = sw1 == ACR122_SW1_WARNING_WITH_NV_CHANGED
        && sw2 == ACR122_SW2_PN53X_APPLICATION_LEVEL_ERROR;
    let no_reply = sw1 == ACR122_SW1_WARNING_WITH_NV_CHANGED && sw2 == 0x00;
    let ok = sw1 == ACR122_SW1_SUCCESS && sw2 == ACR122_SW2_SUCCESS;

    Some(Acr122StatusWord {
        sw1,
        sw2,
        more_data_length: if has_more_data { sw2 } else { 0 },
        has_more_data,
        application_error,
        no_reply,
        ok,
        unexpected: !(has_more_data || application_error || no_reply || ok),
    })
}

pub(super) fn has_firmware_prefix(firmware: &str, prefix: &str) -> bool {
    firmware.starts_with(prefix)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn is_acr122u_firmware(firmware: &str) -> bool {
    has_firmware_prefix(firmware, "ACR122U")
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn is_acr122s_firmware(firmware: &str) -> bool {
    has_firmware_prefix(firmware, "ACR122S")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_direct_transmit_matches_c_helper() {
        let payload = [0x4A, 0x01, 0x00];
        assert_eq!(
            build_direct_transmit_apdu(&payload).unwrap(),
            vec![0xFF, 0x00, 0x00, 0x00, 0x04, 0xD4, 0x4A, 0x01, 0x00]
        );
    }

    #[test]
    fn build_get_firmware_version_matches_c_helper() {
        assert_eq!(
            build_get_firmware_version_apdu().unwrap(),
            vec![0xFF, 0x00, 0x48, 0x00, 0x00]
        );
    }

    #[test]
    fn build_get_additional_data_matches_c_helper() {
        assert_eq!(
            build_get_additional_data_apdu(0x08).unwrap(),
            vec![0xFF, 0xC0, 0x00, 0x00, 0x08]
        );
    }

    #[test]
    fn parse_status_words_matches_existing_cases() {
        let more_data = parse_status_words(&[0x61, 0x08]).unwrap();
        assert!(more_data.has_more_data);
        assert_eq!(more_data.more_data_length, 0x08);
        assert!(!more_data.unexpected);

        let app_error = parse_status_words(&[0x63, 0x7F]).unwrap();
        assert!(app_error.application_error);
        assert!(!app_error.has_more_data);
    }

    #[test]
    fn matcher_helpers_match_existing_c_tests() {
        assert!(is_usb_device(0x072F, 0x2200));
        assert!(is_usb_device(0x072F, 0x90CC));
        assert!(!is_usb_device(0x04CC, 0x0531));

        assert!(is_pcsc_reader_name("ACS ACR122U PICC Interface 00 00"));
        assert!(is_pcsc_reader_name("ACS ACR38U-CCID 00 00"));
        assert!(!is_pcsc_reader_name("Feitian R502 CL Reader 0"));
    }

    #[test]
    fn firmware_helpers_match_existing_c_tests() {
        assert!(is_acr122u_firmware("ACR122U203"));
        assert!(is_acr122s_firmware("ACR122S101"));
        assert!(!is_acr122u_firmware("PN533"));
    }
}
