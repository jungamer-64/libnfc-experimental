use crate::c_abi::types::{nfc_baud_rate, nfc_modulation, nfc_modulation_type, nfc_target};
use crate::c_boundary::NFC_BUFSIZE_CONNSTRING;
use crate::c_boundary::raw::{
    bounded_strlen, c_string_ptr_to_string, copy_bytes_to_c_buffer, fixed_c_buffer_to_string,
    optional_mut, optional_ref,
};
use crate::c_boundary::status::{NFC_EDEVNOTSUPP, set_device_last_error};
use crate::domain_bridge::decode::{baud_rate_from_c, modulation_type_from_c, target_from_c};
use crate::domain_bridge::encode::{
    baud_rate_to_c, dep_info_to_c, dep_mode_to_c, mode_to_c, modulation_to_c, modulation_type_to_c,
    property_to_c, target_to_c, write_context_to_c,
};
use crate::lifecycle::{
    DEVICE_NAME_LENGTH, NFC_DRIVER_NAME_MAX, nfc_context, nfc_device, nfc_device_new, nfc_driver,
    scan_type_enum,
};
use crate::release_allocated_ptr;
use libc::{c_char, c_int};
use proximate_driver as rt;
use std::ffi::CString;
use std::ptr;
use std::slice;

mod borrowed_rust;
mod common;
mod external;
mod rust_owned;
#[cfg(test)]
mod tests;

const DEFAULT_SCAN_CAPACITY: usize = 4;
const MAX_SCAN_CAPACITY: usize = 256;
const RUST_DEVICE_DRIVER_NAME_BYTES: &[u8] = b"proximate_rust_shim";
#[cfg(test)]
const RUST_DEVICE_DRIVER_NAME_NUL: &[u8] = b"proximate_rust_shim\0";
#[cfg(test)]
const RUST_DEVICE_DRIVER_NAME: *const c_char =
    RUST_DEVICE_DRIVER_NAME_NUL.as_ptr().cast::<c_char>();

fn is_rust_device_driver(driver: &nfc_driver) -> bool {
    let name_len = bounded_strlen(driver.name, NFC_DRIVER_NAME_MAX);
    if name_len != RUST_DEVICE_DRIVER_NAME_BYTES.len() {
        return false;
    }

    let name = unsafe { slice::from_raw_parts(driver.name.cast::<u8>(), name_len) };
    name == RUST_DEVICE_DRIVER_NAME_BYTES
}

pub(crate) use borrowed_rust::borrowed_device;
use common::*;
pub(crate) use external::ExternalDriver;
pub(crate) use rust_owned::{
    attach_rust_device, free_rust_device, is_rust_shim_device, rust_device_state_mut,
};
