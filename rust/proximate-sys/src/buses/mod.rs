// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Internal native bus helpers used by Rust-owned builtin drivers and exported
// through proximate-sys so the remaining C drivers can keep using the existing
// `uart_*`, `spi_*`, and `i2c_*` ABIs unchanged.

#![allow(non_camel_case_types)]

use crate::c_api_impl::NFC_BUFSIZE_CONNSTRING;
use crate::ffi_support::{bounded_strlen, copy_bytes_to_c_buffer};
use libc::{c_char, c_void};
use std::ptr;

pub(crate) mod i2c;
pub(crate) mod spi;
pub(crate) mod uart;

const INVALID_MINUS_ONE: isize = !1isize;
const INVALID_MINUS_TWO: isize = !2isize;

pub(crate) const fn sentinel_ptr(value: isize) -> *mut c_void {
    value as *mut c_void
}

pub(crate) unsafe fn c_path_to_string(path: *const c_char) -> Option<String> {
    if path.is_null() {
        return None;
    }

    let len = bounded_strlen(path, NFC_BUFSIZE_CONNSTRING);
    let bytes = unsafe { std::slice::from_raw_parts(path.cast::<u8>(), len) };
    Some(String::from_utf8_lossy(bytes).into_owned())
}

pub(crate) unsafe fn allocate_c_string(value: &[u8]) -> *mut c_char {
    let buffer = unsafe { libc::malloc(value.len() + 1) as *mut c_char };
    if buffer.is_null() {
        return ptr::null_mut();
    }

    if !unsafe { copy_bytes_to_c_buffer(buffer, value.len() + 1, value) } {
        unsafe { crate::release_allocated_ptr(buffer.cast::<c_void>()) };
        return ptr::null_mut();
    }

    buffer
}

pub(crate) unsafe fn allocate_c_string_array(values: &[Vec<u8>]) -> *mut *mut c_char {
    let count = values.len() + 1;
    let array =
        unsafe { libc::calloc(count, std::mem::size_of::<*mut c_char>()) as *mut *mut c_char };
    if array.is_null() {
        return ptr::null_mut();
    }

    for (index, value) in values.iter().enumerate() {
        let entry = unsafe { allocate_c_string(value) };
        if entry.is_null() {
            for cleanup_index in 0..index {
                unsafe {
                    crate::release_allocated_ptr((*array.add(cleanup_index)).cast::<c_void>());
                }
            }
            unsafe { crate::release_allocated_ptr(array.cast::<c_void>()) };
            return ptr::null_mut();
        }
        unsafe {
            *array.add(index) = entry;
        }
    }

    unsafe {
        *array.add(values.len()) = ptr::null_mut();
    }
    array
}

pub(crate) const fn invalid_serial_port() -> *mut c_void {
    sentinel_ptr(INVALID_MINUS_ONE)
}

pub(crate) const fn claimed_serial_port() -> *mut c_void {
    sentinel_ptr(INVALID_MINUS_TWO)
}

pub(crate) const fn invalid_spi_port() -> *mut c_void {
    sentinel_ptr(INVALID_MINUS_ONE)
}

#[cfg(any(test, all(libnfc_driver_pn532_spi, not(target_os = "linux"))))]
pub(crate) const fn claimed_spi_port() -> *mut c_void {
    sentinel_ptr(INVALID_MINUS_TWO)
}

pub(crate) const fn invalid_i2c_bus() -> *mut c_void {
    sentinel_ptr(INVALID_MINUS_ONE)
}

pub(crate) const fn invalid_i2c_address() -> *mut c_void {
    sentinel_ptr(INVALID_MINUS_TWO)
}
