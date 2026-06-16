// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Shared low-level helpers for Rust FFI code.

use libc::c_char;
use std::{ptr, slice};

pub(crate) unsafe fn optional_ref<'a, T>(ptr: *const T) -> Option<&'a T> {
    unsafe { ptr.as_ref() }
}

pub(crate) unsafe fn optional_mut<'a, T>(ptr: *mut T) -> Option<&'a mut T> {
    unsafe { ptr.as_mut() }
}

pub(crate) fn bounded_strlen(ptr: *const c_char, max: usize) -> usize {
    if ptr.is_null() || max == 0 {
        return 0;
    }

    let bytes = unsafe { slice::from_raw_parts(ptr.cast::<u8>(), max) };
    bytes.iter().position(|&byte| byte == 0).unwrap_or(max)
}

pub(crate) fn c_string_ptr_to_string(ptr: *const c_char, max_len: usize) -> String {
    if ptr.is_null() {
        return String::new();
    }

    let length = bounded_strlen(ptr, max_len);
    let bytes = unsafe { slice::from_raw_parts(ptr.cast::<u8>(), length) };
    String::from_utf8_lossy(bytes).into_owned()
}

pub(crate) fn fixed_c_buffer_to_string(buffer: &[c_char]) -> String {
    let length = buffer
        .iter()
        .position(|&ch| ch == 0)
        .unwrap_or(buffer.len());
    let bytes: Vec<u8> = buffer[..length].iter().map(|&ch| ch as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

pub(crate) unsafe fn copy_bytes_to_c_buffer(dst: *mut c_char, dst_size: usize, src: &[u8]) -> bool {
    if dst.is_null() || src.len() >= dst_size {
        return false;
    }

    unsafe {
        if !src.is_empty() {
            ptr::copy_nonoverlapping(src.as_ptr().cast::<c_char>(), dst, src.len());
        }
        *dst.add(src.len()) = 0;
    }

    true
}

pub(crate) unsafe fn copy_c_string_to_c_buffer(
    dst: *mut c_char,
    dst_size: usize,
    src: *const c_char,
) -> bool {
    if dst.is_null() || src.is_null() {
        return false;
    }

    let length = bounded_strlen(src, dst_size);
    if length >= dst_size {
        return false;
    }

    unsafe {
        if length > 0 {
            ptr::copy_nonoverlapping(src, dst, length);
        }
        *dst.add(length) = 0;
    }

    true
}
