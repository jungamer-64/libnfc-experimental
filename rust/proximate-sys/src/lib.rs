//! libnfc 1.8 C ABI boundary backed by Rust domain and driver types.
//!
//! Context and device pointers are opaque identities for Rust-owned
//! allocations. Unsafe pointer interpretation, integer-carrier validation,
//! legacy external-driver projections, and C allocation ownership stay in
//! this crate; opened devices use one safe `proximate_driver::Device` path.

/// cbindgen:ignore
mod c_abi;
/// cbindgen:ignore
mod c_boundary;
/// cbindgen:ignore
mod core;
/// cbindgen:ignore
mod domain_bridge;
/// cbindgen:ignore
mod ffi_strings;
/// cbindgen:ignore
mod initiator;
mod lifecycle;
/// cbindgen:ignore
mod logger;
#[cfg(test)]
mod test_support;
pub use c_abi::exports::*;
#[cfg(cbindgen)]
use c_abi::private as _;
pub use c_abi::types::{
    nfc_barcode_info, nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_felica_info,
    nfc_iso14443a_info, nfc_iso14443b_info, nfc_iso14443b2ct_info, nfc_iso14443b2sr_info,
    nfc_iso14443bi_info, nfc_iso14443biclass_info, nfc_jewel_info, nfc_mode, nfc_modulation,
    nfc_modulation_type, nfc_property, nfc_target, nfc_target_info,
};
pub use c_boundary::NFC_BUFSIZE_CONNSTRING;
pub(crate) use c_boundary::{
    MALLOC_LABEL, emit_log_message, ffi_catch_unwind_int, ffi_catch_unwind_ptr,
    ffi_catch_unwind_void, log_error, log_message, release_allocated_ptr, reset_last_error,
    set_last_error_message,
};
pub use lifecycle::{nfc_connstring, nfc_context, nfc_device, nfc_driver};
