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
