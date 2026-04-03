// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Ported from libnfc/nfc.c.

use crate::bridge::{
    CStringOut, CyclesOut, InputBytes, NFC_EDEVNOTSUPP, NFC_EINVARG, NFC_ESOFT, OutputBytes,
    ParityMarker, ParityMarkerMut, SupportedBaudRatesOut, SupportedModulationsOut, TargetInOut,
    TargetOut, TargetSliceOut, baud_rate_from_c, baud_rate_to_c, borrowed_device,
    decode_modulations, decode_optional_dep_info, decode_optional_target, dep_info_from_c,
    dep_mode_from_c, device_last_error, error_message_ptr, error_to_status, is_rust_shim_device,
    modulation_from_c, modulation_type_from_c, modulation_type_to_c, property_from_c,
    reset_device_last_error, runtime_result_status, rust_device_state_mut, set_device_last_error,
    target_from_c, unsupported_driver_operation, write_target_to_c,
};
use crate::c_api_impl::{LOG_GROUP_GENERAL, LOG_PRIORITY_DEBUG};
use crate::ffi_strings::device_error_message_cstr;
use crate::ffi_support::{
    as_mut, as_ref, bounded_strlen, c_string_ptr_to_string, copy_bytes_to_c_buffer,
};
use crate::ffi_types::{
    nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_mode, nfc_modulation, nfc_modulation_type,
    nfc_property, nfc_target,
};
use crate::lifecycle::nfc_device;
use crate::{emit_log_message, ffi_catch_unwind_int, ffi_catch_unwind_ptr, ffi_catch_unwind_void};
use libc::{c_char, c_int, c_void, size_t};
use proximate_driver as rt;
use std::ffi::CString;
#[cfg(test)]
use std::mem::size_of;
use std::ptr;
use std::slice;

mod accessors;
mod driver_dispatch;
mod emulation;
mod operations;
mod runtime;
#[cfg(test)]
mod tests;

#[cfg(test)]
const NFC_ETIMEOUT: c_int = -6;
const ISO7816_SHORT_C_APDU_MAX_LEN: usize = 261;
const ISO7816_SHORT_R_APDU_MAX_LEN: usize = 258;

const GENERAL_LOG_CATEGORY: *const c_char = b"libnfc.general\0" as *const u8 as *const c_char;
const NULL_ERROR_PREFIX: *const c_char = b"(null)\0" as *const u8 as *const c_char;

unsafe extern "C" {
    static mut stderr: *mut libc::FILE;
}

pub use accessors::{
    nfc_device_get_connstring, nfc_device_get_information_about, nfc_device_get_last_error,
    nfc_device_get_name, nfc_device_get_supported_baud_rate,
    nfc_device_get_supported_baud_rate_target_mode, nfc_device_get_supported_modulation,
    nfc_perror, nfc_strerror, nfc_strerror_r,
};
use driver_dispatch::*;
#[allow(unused_imports)]
pub use emulation::{
    nfc_emulate_target, nfc_emulation_io_fn, nfc_emulation_state_machine, nfc_emulator,
};
pub use operations::{
    nfc_abort_command, nfc_device_set_property_bool, nfc_device_set_property_int, nfc_idle,
    nfc_initiator_deselect_target, nfc_initiator_init, nfc_initiator_init_secure_element,
    nfc_initiator_list_passive_targets, nfc_initiator_poll_dep_target, nfc_initiator_poll_target,
    nfc_initiator_select_dep_target, nfc_initiator_select_passive_target,
    nfc_initiator_target_is_present, nfc_initiator_transceive_bits,
    nfc_initiator_transceive_bits_timed, nfc_initiator_transceive_bytes,
    nfc_initiator_transceive_bytes_timed, nfc_target_init, nfc_target_receive_bits,
    nfc_target_receive_bytes, nfc_target_send_bits, nfc_target_send_bytes,
};

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

fn property_name(property: nfc_property) -> &'static str {
    property_from_c(property).name()
}
