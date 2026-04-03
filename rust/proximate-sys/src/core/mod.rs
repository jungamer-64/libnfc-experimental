// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Ported from libnfc/nfc.c.

#[cfg(test)]
use crate::bridge::registry_snapshot;
use crate::bridge::{
    ConnstringsOut, attach_rust_device, clear_registry, context_from_c, decode_connstring_ptr,
    register_external_drivers,
};
use crate::c_api_impl::{
    LOG_GROUP_GENERAL, LOG_PRIORITY_DEBUG, LOG_PRIORITY_ERROR, NFC_BUFSIZE_CONNSTRING,
};
#[cfg(any(test, not(libnfc_external_bridges)))]
use crate::ffi_support::as_ref;
use crate::ffi_support::{bounded_strlen, c_string_ptr_to_string, copy_bytes_to_c_buffer};
#[cfg(test)]
use crate::ffi_support::{copy_c_string_to_c_buffer, fixed_c_buffer_to_string};
#[cfg(test)]
use crate::lifecycle::{DEVICE_NAME_LENGTH, NFC_DRIVER_NAME_MAX, scan_type_enum};
use crate::lifecycle::{nfc_connstring, nfc_context, nfc_context_new, nfc_device, nfc_driver};
use crate::{
    MALLOC_LABEL, emit_log_message, ffi_catch_unwind_int, ffi_catch_unwind_ptr,
    ffi_catch_unwind_void,
};
use libc::{c_char, c_int, size_t};
use proximate_driver as rt;
use std::ffi::CString;

mod context;
mod driver_registration;
mod runtime;
#[cfg(test)]
mod tests;

#[cfg(test)]
const NFC_SUCCESS: c_int = 0;
const NFC_ESOFT: c_int = -80;
const LOG_PRIORITY_INFO: u8 = 2;
const GENERAL_LOG_CATEGORY: *const c_char = b"libnfc.general\0" as *const u8 as *const c_char;

fn log_general_message(priority: u8, message: &str) {
    if let Ok(c_msg) = CString::new(message) {
        unsafe {
            emit_log_message(
                LOG_GROUP_GENERAL,
                GENERAL_LOG_CATEGORY,
                priority,
                c_msg.as_ptr(),
            );
        }
    }
}

fn log_general_debug(message: &str) {
    log_general_message(LOG_PRIORITY_DEBUG, message);
}

fn log_general_error(message: &str) {
    log_general_message(LOG_PRIORITY_ERROR, message);
}

fn log_general_info(message: &str) {
    log_general_message(LOG_PRIORITY_INFO, message);
}

pub use context::{nfc_exit, nfc_init};
pub(crate) use driver_registration::bridge_close_device;
pub use driver_registration::nfc_register_driver;
pub use runtime::{nfc_list_devices, nfc_open};
