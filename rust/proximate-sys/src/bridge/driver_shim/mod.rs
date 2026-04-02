use crate::bridge::c_to_rust::{
    baud_rate_from_c, dep_info_from_c, dep_mode_from_c, modulation_from_c, modulation_type_from_c,
    property_from_c, target_from_c,
};
use crate::bridge::rust_to_c::{
    baud_rate_to_c, dep_info_to_c, dep_mode_to_c, mode_to_c, modulation_to_c, modulation_type_to_c,
    property_to_c, target_to_c, write_context_to_c, write_target_to_c,
};
use crate::bridge::status::{
    NFC_EDEVNOTSUPP, NFC_EINVARG, NFC_ESOFT, error_to_status, set_device_last_error,
};
use crate::c_api_impl::NFC_BUFSIZE_CONNSTRING;
use crate::ffi_support::{
    as_mut, as_ref, bounded_strlen, c_string_ptr_to_string, copy_bytes_to_c_buffer,
    fixed_c_buffer_to_string,
};
use crate::ffi_types::{
    nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_mode, nfc_modulation, nfc_modulation_type,
    nfc_property, nfc_target,
};
use crate::lifecycle::{
    DEVICE_NAME_LENGTH, NFC_DRIVER_NAME_MAX, nfc_context, nfc_device, nfc_device_new, nfc_driver,
    scan_type_enum,
};
use crate::release_allocated_ptr;
use libc::{c_char, c_int};
use proximate_driver as rt;
use std::ffi::CString;
use std::{ptr, slice};

mod borrowed_rust;
mod common;
mod external;
mod rust_owned;
#[cfg(test)]
mod tests;

const DEFAULT_SCAN_CAPACITY: usize = 4;
const MAX_SCAN_CAPACITY: usize = 256;
const RUST_DEVICE_DRIVER_NAME: *const c_char =
    b"proximate_rust_shim\0" as *const u8 as *const c_char;

use common::*;
pub(crate) use borrowed_rust::borrowed_device;
pub(crate) use external::ExternalDriver;
pub(crate) use rust_owned::{attach_rust_device, is_rust_shim_device};
