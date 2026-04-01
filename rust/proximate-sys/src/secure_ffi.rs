// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// C-facing secure memory helpers owned by proximate-sys.

use crate::ffi_support::bounded_strlen;
use libc::{c_char, c_int, size_t};
use std::ptr;
use std::sync::atomic::{Ordering, compiler_fence};

pub const NFC_SECURE_SUCCESS: c_int = 0;
pub const NFC_SECURE_ERROR_INVALID: c_int = -1;
pub const NFC_SECURE_ERROR_OVERFLOW: c_int = -2;
pub const NFC_SECURE_ERROR_RANGE: c_int = -3;
pub const NFC_SECURE_ERROR_ZERO_SIZE: c_int = -4;
const NFC_SECURE_ERROR_INTERNAL: c_int = -5;
const NFC_SECURE_MAX_REASONABLE_SIZE_64: usize = 1usize << 47;

fn secure_max_size() -> size_t {
    if std::mem::size_of::<size_t>() >= 8 {
        NFC_SECURE_MAX_REASONABLE_SIZE_64 as size_t
    } else {
        size_t::MAX / 4
    }
}

fn validate_copy_params(
    dst: *mut u8,
    dst_size: size_t,
    src: *const u8,
    src_size: size_t,
) -> Result<usize, c_int> {
    if dst.is_null() || src.is_null() {
        return Err(NFC_SECURE_ERROR_INVALID);
    }
    if src_size == 0 {
        return Ok(0);
    }
    let max = secure_max_size();
    if src_size > max || dst_size > max {
        return Err(NFC_SECURE_ERROR_RANGE);
    }
    if dst_size.checked_add(src_size).is_none() {
        return Err(NFC_SECURE_ERROR_RANGE);
    }
    if dst_size < src_size {
        return Err(NFC_SECURE_ERROR_OVERFLOW);
    }
    Ok(src_size as usize)
}

fn validate_fill_target(ptr: *mut libc::c_void, size: size_t) -> Result<usize, c_int> {
    if ptr.is_null() {
        return Err(NFC_SECURE_ERROR_INVALID);
    }
    if size == 0 {
        return Ok(0);
    }
    if size > secure_max_size() {
        return Err(NFC_SECURE_ERROR_RANGE);
    }
    Ok(size as usize)
}

pub unsafe fn nfc_safe_memcpy(
    dst: *mut libc::c_void,
    dst_size: size_t,
    src: *const libc::c_void,
    src_size: size_t,
) -> c_int {
    crate::ffi_catch_unwind_int("nfc_safe_memcpy", NFC_SECURE_ERROR_INTERNAL, || {
        let len = match validate_copy_params(dst.cast::<u8>(), dst_size, src.cast::<u8>(), src_size)
        {
            Ok(len) => len,
            Err(error) => return error,
        };
        if len == 0 {
            return NFC_SECURE_SUCCESS;
        }
        unsafe { ptr::copy_nonoverlapping(src.cast::<u8>(), dst.cast::<u8>(), len) };
        NFC_SECURE_SUCCESS
    })
}

pub unsafe fn nfc_safe_memmove(
    dst: *mut libc::c_void,
    dst_size: size_t,
    src: *const libc::c_void,
    src_size: size_t,
) -> c_int {
    crate::ffi_catch_unwind_int("nfc_safe_memmove", NFC_SECURE_ERROR_INTERNAL, || {
        let len = match validate_copy_params(dst.cast::<u8>(), dst_size, src.cast::<u8>(), src_size)
        {
            Ok(len) => len,
            Err(error) => return error,
        };
        if len == 0 {
            return NFC_SECURE_SUCCESS;
        }
        unsafe { ptr::copy(src.cast::<u8>(), dst.cast::<u8>(), len) };
        NFC_SECURE_SUCCESS
    })
}

pub unsafe fn nfc_secure_memset(ptr: *mut libc::c_void, val: c_int, size: size_t) -> c_int {
    crate::ffi_catch_unwind_int("nfc_secure_memset", NFC_SECURE_ERROR_INTERNAL, || {
        let len = match validate_fill_target(ptr, size) {
            Ok(len) => len,
            Err(error) => return error,
        };
        if len == 0 {
            return NFC_SECURE_SUCCESS;
        }

        unsafe { ptr::write_bytes(ptr.cast::<u8>(), val as u8, len) };
        compiler_fence(Ordering::SeqCst);
        NFC_SECURE_SUCCESS
    })
}

pub unsafe fn nfc_secure_zero(ptr: *mut libc::c_void, size: size_t) -> c_int {
    crate::ffi_catch_unwind_int("nfc_secure_zero", NFC_SECURE_ERROR_INTERNAL, || {
        let len = match validate_fill_target(ptr, size) {
            Ok(len) => len,
            Err(error) => return error,
        };
        if len == 0 {
            return NFC_SECURE_SUCCESS;
        }

        unsafe { ptr::write_bytes(ptr.cast::<u8>(), 0, len) };
        compiler_fence(Ordering::SeqCst);
        NFC_SECURE_SUCCESS
    })
}

pub fn nfc_secure_strerror(code: c_int) -> *const c_char {
    match code {
        NFC_SECURE_SUCCESS => c"Success".as_ptr(),
        NFC_SECURE_ERROR_INVALID => c"Invalid parameter (NULL pointer or invalid input)".as_ptr(),
        NFC_SECURE_ERROR_OVERFLOW => c"Buffer overflow prevented (destination too small)".as_ptr(),
        NFC_SECURE_ERROR_RANGE => c"Size parameter out of valid range".as_ptr(),
        NFC_SECURE_ERROR_ZERO_SIZE => {
            c"Zero-size operation (deprecated, now treated as success)".as_ptr()
        }
        _ => c"Unknown error code".as_ptr(),
    }
}

pub unsafe fn nfc_safe_strlen(str: *const c_char, maxlen: size_t) -> size_t {
    bounded_strlen(str, maxlen as usize) as size_t
}

pub unsafe fn nfc_is_null_terminated(buf: *const c_char, bufsize: size_t) -> c_int {
    if buf.is_null() || bufsize == 0 {
        return 0;
    }

    let bytes = unsafe { std::slice::from_raw_parts(buf.cast::<u8>(), bufsize as usize) };
    bytes.contains(&0) as c_int
}

pub unsafe fn nfc_ensure_null_terminated(buf: *mut c_char, bufsize: size_t) {
    if buf.is_null() || bufsize == 0 {
        return;
    }

    let bytes = unsafe { std::slice::from_raw_parts_mut(buf.cast::<u8>(), bufsize as usize) };
    if !bytes.contains(&0) {
        if let Some(last) = bytes.last_mut() {
            *last = 0;
        }
    }
}
