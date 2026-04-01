use std::ffi::{CStr, CString};
use std::sync::OnceLock;

static VERSION: OnceLock<Box<str>> = OnceLock::new();
static VERSION_CSTR: OnceLock<CString> = OnceLock::new();

pub fn version() -> &'static str {
    VERSION
        .get_or_init(|| {
            option_env!("PROXIMATE_GIT_REVISION")
                .filter(|value| !value.is_empty())
                .unwrap_or(
                    option_env!("PROXIMATE_PACKAGE_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")),
                )
                .into()
        })
        .as_ref()
}

#[doc(hidden)]
pub fn version_cstr() -> &'static CStr {
    VERSION_CSTR
        .get_or_init(|| CString::new(version()).expect("version string must not contain NUL"))
        .as_c_str()
}

#[doc(hidden)]
pub const fn device_error_message(code: i32) -> &'static str {
    match code {
        0 => "Success",
        -1 => "Input / Output Error",
        -2 => "Invalid argument(s)",
        -3 => "Not Supported by Device",
        -4 => "No Such Device",
        -5 => "Buffer Overflow",
        -6 => "Timeout",
        -7 => "Operation Aborted",
        -8 => "Not (yet) Implemented",
        -10 => "Target Released",
        -20 => "RF Transmission Error",
        -30 => "Mifare Authentication Failed",
        -90 => "Device's Internal Chip Error",
        _ => "Unknown error",
    }
}

#[doc(hidden)]
pub const fn device_error_message_cstr(code: i32) -> &'static CStr {
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
