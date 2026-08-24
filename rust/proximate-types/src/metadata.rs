/// Returns the libnfc compatibility version exposed by the public C ABI.
///
/// This value deliberately does not follow the Rust crate release version:
/// the crate and the emulated libnfc ABI have independent version authorities.
pub const fn version() -> &'static str {
    "1.8.0"
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
