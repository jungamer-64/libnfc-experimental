#[cfg_attr(not(any(feature = "c_ffi", cbindgen)), allow(dead_code))]
/// cbindgen:ignore
mod c_abi;
#[cfg(any(feature = "lifecycle", cbindgen))]
#[cfg_attr(not(any(feature = "c_ffi", cbindgen)), allow(dead_code))]
/// cbindgen:ignore
mod c_boundary;
#[cfg(any(feature = "lifecycle", cbindgen))]
#[cfg_attr(not(any(feature = "c_ffi", cbindgen)), allow(dead_code))]
/// cbindgen:ignore
mod core;
#[cfg(any(feature = "lifecycle", feature = "orchestration", cbindgen))]
#[cfg_attr(not(any(feature = "c_ffi", cbindgen)), allow(dead_code))]
/// cbindgen:ignore
mod domain_bridge;
/// cbindgen:ignore
mod ffi_strings;
/// cbindgen:ignore
#[cfg(any(feature = "orchestration", cbindgen))]
#[cfg_attr(not(any(feature = "c_ffi", cbindgen)), allow(dead_code))]
mod initiator;
#[cfg(any(feature = "lifecycle", cbindgen))]
#[cfg_attr(not(any(feature = "c_ffi", cbindgen)), allow(dead_code))]
mod lifecycle;
/// cbindgen:ignore
mod logger;
#[cfg(any(feature = "c_ffi", cbindgen))]
pub use c_abi::exports::*;
#[cfg(cbindgen)]
use c_abi::private as _;
#[cfg(any(feature = "c_ffi", cbindgen, test))]
pub use c_abi::types::{
    nfc_barcode_info, nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_felica_info,
    nfc_iso14443a_info, nfc_iso14443b_info, nfc_iso14443b2ct_info, nfc_iso14443b2sr_info,
    nfc_iso14443bi_info, nfc_iso14443biclass_info, nfc_jewel_info, nfc_mode, nfc_modulation,
    nfc_modulation_type, nfc_property, nfc_target, nfc_target_info,
};
#[cfg(any(feature = "c_ffi", cbindgen, test))]
pub use c_boundary::NFC_BUFSIZE_CONNSTRING;
pub(crate) use c_boundary::{
    MALLOC_LABEL, emit_log_message, ffi_catch_unwind_int, ffi_catch_unwind_ptr,
    ffi_catch_unwind_void, log_error, log_message, release_allocated_ptr, reset_last_error,
    set_last_error_message,
};
#[cfg(any(feature = "c_ffi", cbindgen, test))]
pub use lifecycle::{nfc_connstring, nfc_context, nfc_device, nfc_driver};
#[cfg(test)]
pub(crate) use logger::{
    test_clear_rendered_logs as test_clear_last_log, test_get_last_log, test_get_logs,
    test_reset_log_level,
};
