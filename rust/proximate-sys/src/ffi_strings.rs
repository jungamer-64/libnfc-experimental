use proximate::rust_api as rt;
use std::ffi::{CStr, CString};
use std::sync::OnceLock;

static VERSION_CSTR: OnceLock<CString> = OnceLock::new();

pub(crate) fn version_cstr() -> &'static CStr {
    VERSION_CSTR
        .get_or_init(|| CString::new(rt::version()).expect("version string must not contain NUL"))
        .as_c_str()
}

pub(crate) const fn device_error_message_cstr(code: i32) -> &'static CStr {
    match code {
        0 => c"Success",
        -1 => c"Input / Output Error",
        -2 => c"Invalid argument(s)",
        -3 => c"Not Supported by Device",
        -4 => c"No Such Device",
        -5 => c"Buffer Overflow",
        -6 => c"Timeout",
        -7 => c"Operation Aborted",
        -8 => c"Not (yet) Implemented",
        -10 => c"Target Released",
        -20 => c"RF Transmission Error",
        -30 => c"Mifare Authentication Failed",
        -90 => c"Device's Internal Chip Error",
        _ => c"Unknown error",
    }
}

pub(crate) const fn baud_rate_label_cstr(value: rt::BaudRate) -> &'static CStr {
    match value {
        rt::BaudRate::Undefined => c"undefined baud rate",
        rt::BaudRate::Br106 => c"106 kbps",
        rt::BaudRate::Br212 => c"212 kbps",
        rt::BaudRate::Br424 => c"424 kbps",
        rt::BaudRate::Br847 => c"847 kbps",
    }
}

pub(crate) const fn modulation_label_cstr(value: rt::ModulationType) -> &'static CStr {
    match value {
        rt::ModulationType::Undefined => c"???",
        rt::ModulationType::Iso14443A => c"ISO/IEC 14443A",
        rt::ModulationType::Jewel => c"Innovision Jewel",
        rt::ModulationType::Iso14443B => c"ISO/IEC 14443-4B",
        rt::ModulationType::Iso14443Bi => c"ISO/IEC 14443-4B'",
        rt::ModulationType::Iso14443B2Sr => c"ISO/IEC 14443-2B ST SRx",
        rt::ModulationType::Iso14443B2Ct => c"ISO/IEC 14443-2B ASK CTx",
        rt::ModulationType::Felica => c"FeliCa",
        rt::ModulationType::Dep => c"D.E.P.",
        rt::ModulationType::Barcode => c"Thinfilm NFC Barcode",
        rt::ModulationType::Iso14443BiClass => c"ISO/IEC 14443-2B-3B iClass (Picopass)",
    }
}
