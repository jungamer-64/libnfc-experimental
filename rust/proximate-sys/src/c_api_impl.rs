// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// This Rust crate contains libnfc FFI support code together with Rust
// implementations of selected libnfc helpers. The connstring decoding helper
// in this file is derived from libnfc/nfc-internal.c.
//
// Libnfc historical contributors:
// Copyright (C) 2009      Roel Verdult
// Copyright (C) 2009-2013 Romuald Conty
// Copyright (C) 2010-2012 Romain Tartiere
// Copyright (C) 2010-2013 Philippe Teuwen
// Copyright (C) 2012-2013 Ludovic Rousseau
// Copyright (C) 2020      Adam Laurie
// See AUTHORS file for a more comprehensive list of contributors.

use crate::ffi_support::{bounded_strlen, copy_bytes_to_c_buffer};
use crate::logger;
use libc::{c_char, c_int, c_void, size_t};
use proximate::rust_api as rt;
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::panic;
use std::ptr;

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

pub const NFC_COMMON_SUCCESS: c_int = 0;
pub const NFC_COMMON_ERROR: c_int = -1;
pub const NFC_COMMON_INVALID: c_int = -(libc::EINVAL as c_int);

pub const LOG_GROUP_GENERAL: u8 = 1;
#[cfg(any(feature = "lifecycle", cbindgen))]
pub(crate) const LOG_PRIORITY_NONE: u8 = 0;
pub const LOG_PRIORITY_ERROR: u8 = 1;
pub const LOG_PRIORITY_DEBUG: u8 = 3;

const LOG_CATEGORY: *const c_char = b"libnfc.common\0" as *const u8 as *const c_char;
pub const NFC_BUFSIZE_CONNSTRING: usize = 1024;
pub(crate) const MALLOC_LABEL: *const c_char = b"malloc\0" as *const u8 as *const c_char;

#[cfg(any(feature = "c_ffi", cbindgen, test))]
pub(crate) unsafe fn nfc_rs_log_message(
    group: u8,
    category: *const c_char,
    priority: u8,
    message: *const c_char,
) {
    unsafe { logger::log_message_ptrs(group, category, priority, message) };
}

pub(crate) fn log_message(priority: u8, message: &str) {
    if let Ok(c_msg) = CString::new(message) {
        unsafe { emit_log_message(LOG_GROUP_GENERAL, LOG_CATEGORY, priority, c_msg.as_ptr()) };
    }
}

pub(crate) unsafe fn emit_log_message(
    group: u8,
    category: *const c_char,
    priority: u8,
    message: *const c_char,
) {
    unsafe { logger::log_message_ptrs(group, category, priority, message) };
}

#[inline]
pub(crate) fn log_error(message: &str) {
    log_message(LOG_PRIORITY_ERROR, message);
}

#[inline]
pub(crate) fn log_debug(message: &str) {
    log_message(LOG_PRIORITY_DEBUG, message);
}

pub(crate) fn set_last_error_message<S: Into<String>>(message: S) {
    let message = message.into();
    LAST_ERROR.with(|cell| {
        let cstr = CString::new(message)
            .unwrap_or_else(|_| CString::new("error message contained interior NUL").unwrap());
        *cell.borrow_mut() = Some(cstr);
    });
}

pub(crate) fn reset_last_error() {
    LAST_ERROR.with(|cell| {
        cell.borrow_mut().take();
    });
}

fn ensure_utf8(cstr: &CStr, context: &str) -> Result<(), c_int> {
    if cstr.to_str().is_err() {
        let message = format!("{} contains non UTF-8 data", context);
        log_error(&message);
        set_last_error_message(message);
        return Err(NFC_COMMON_INVALID);
    }
    Ok(())
}

fn validate_non_null(ptr: *const c_char, message: &str) -> Result<&CStr, c_int> {
    if ptr.is_null() {
        log_error(message);
        set_last_error_message(message);
        return Err(NFC_COMMON_INVALID);
    }

    unsafe { Ok(CStr::from_ptr(ptr)) }
}

fn validate_mut_ptr(ptr: *mut c_char, message: &str) -> Result<*mut c_char, c_int> {
    if ptr.is_null() {
        log_error(message);
        set_last_error_message(message);
        return Err(NFC_COMMON_INVALID);
    }
    Ok(ptr)
}

fn set_error_and_return(code: c_int, message: String) -> c_int {
    log_error(&message);
    set_last_error_message(message);
    code
}

fn write_checked_c_buffer(dst: &mut [c_char], value: &[u8], overflow_message: String) -> c_int {
    if !unsafe { copy_bytes_to_c_buffer(dst.as_mut_ptr(), dst.len(), value) } {
        set_last_error_message(overflow_message.clone());
        log_error(&overflow_message);
        return NFC_COMMON_ERROR;
    }

    NFC_COMMON_SUCCESS
}
unsafe fn alloc_and_copy(segment: &[u8]) -> Result<*mut c_char, ()> {
    unsafe {
        let length = segment.len().min(NFC_BUFSIZE_CONNSTRING);
        let size = length + 1;
        let memory = libc::malloc(size) as *mut c_char;
        if memory.is_null() {
            libc::perror(MALLOC_LABEL);
            return Err(());
        }

        if !copy_bytes_to_c_buffer(memory, size, &segment[..length]) {
            release_allocated_ptr(memory.cast::<c_void>());
            return Err(());
        }

        Ok(memory)
    }
}

unsafe extern "C" {
    #[link_name = "free"]
    fn c_free(ptr: *mut c_void);
}

pub(crate) unsafe fn release_allocated_ptr(ptr: *mut c_void) {
    if !ptr.is_null() {
        unsafe { c_free(ptr) };
    }
}

fn record_panic(context: &str) {
    let message = format!("panic in {}", context);
    log_error(&message);
    set_last_error_message(message);
}

/// Run the provided operation inside a panic boundary and convert panics
/// into a stable error return code. This prevents Rust panics from
/// unwinding across the FFI boundary. The function logs the panic and
/// sets the thread-local last-error buffer so C callers can inspect it.
///
/// For unit testing it's sometimes useful to observe panics directly.
/// To support those cases we provide an opt-in feature
/// `test_no_catch` which compiles a version that *does not* catch
/// panics. This feature should only be enabled in test builds and is
/// intentionally opt-in to avoid changing FFI behavior in normal
/// builds.
#[cfg(not(feature = "test_no_catch"))]
pub(crate) fn ffi_catch_unwind_int<F>(context: &str, panic_code: c_int, operation: F) -> c_int
where
    F: FnOnce() -> c_int,
    F: panic::UnwindSafe,
{
    match panic::catch_unwind(panic::AssertUnwindSafe(operation)) {
        Ok(result) => result,
        Err(_) => {
            record_panic(context);
            panic_code
        }
    }
}

// Test-only variant that bypasses the panic-catching wrapper so tests
// can observe panics directly. Enable by running `cargo test --features test_no_catch`.
#[cfg(feature = "test_no_catch")]
pub(crate) fn ffi_catch_unwind_int<F>(_context: &str, _panic_code: c_int, operation: F) -> c_int
where
    F: FnOnce() -> c_int,
    F: panic::UnwindSafe,
{
    // Intentionally do not catch unwinds; let the panic propagate to
    // the test harness so tests can assert #[should_panic] behavior.
    operation()
}

#[cfg(not(feature = "test_no_catch"))]
#[allow(dead_code)]
pub(crate) fn ffi_catch_unwind_ptr<T, F>(context: &str, operation: F) -> *mut T
where
    F: FnOnce() -> *mut T,
    F: panic::UnwindSafe,
{
    match panic::catch_unwind(panic::AssertUnwindSafe(operation)) {
        Ok(result) => result,
        Err(_) => {
            record_panic(context);
            ptr::null_mut()
        }
    }
}

#[cfg(feature = "test_no_catch")]
#[allow(dead_code)]
pub(crate) fn ffi_catch_unwind_ptr<T, F>(_context: &str, operation: F) -> *mut T
where
    F: FnOnce() -> *mut T,
    F: panic::UnwindSafe,
{
    operation()
}

#[cfg(not(feature = "test_no_catch"))]
pub(crate) fn ffi_catch_unwind_void<F>(context: &str, operation: F)
where
    F: FnOnce(),
    F: panic::UnwindSafe,
{
    if panic::catch_unwind(panic::AssertUnwindSafe(operation)).is_err() {
        record_panic(context);
    }
}

#[cfg(feature = "test_no_catch")]
pub(crate) fn ffi_catch_unwind_void<F>(_context: &str, operation: F)
where
    F: FnOnce(),
    F: panic::UnwindSafe,
{
    operation()
}

pub unsafe fn nfc_parse_connstring(
    connstring: *const c_char,
    prefix: *const c_char,
    param_name: *const c_char,
    param_value: *mut c_char,
    param_value_size: size_t,
) -> c_int {
    unsafe {
        if param_value_size == 0 {
            return set_error_and_return(
                NFC_COMMON_INVALID,
                "Zero-size param_value buffer in connstring parsing".to_string(),
            );
        }

        let connstring_c = match validate_non_null(connstring, "NULL connstring in parsing") {
            Ok(value) => value,
            Err(code) => return code,
        };
        let prefix_c = match validate_non_null(prefix, "NULL prefix in connstring parsing") {
            Ok(value) => value,
            Err(code) => return code,
        };
        let param_name_c =
            match validate_non_null(param_name, "NULL param_name in connstring parsing") {
                Ok(value) => value,
                Err(code) => return code,
            };
        let param_value_ptr =
            match validate_mut_ptr(param_value, "NULL param_value buffer in connstring parsing") {
                Ok(ptr) => ptr,
                Err(code) => return code,
            };

        if let Err(code) = ensure_utf8(connstring_c, "connstring") {
            return code;
        }
        if let Err(code) = ensure_utf8(prefix_c, "prefix") {
            return code;
        }
        if let Err(code) = ensure_utf8(param_name_c, "param_name") {
            return code;
        }

        let param_buffer = std::slice::from_raw_parts_mut(param_value_ptr, param_value_size);
        let parsed = match rt::parse_connstring(
            connstring_c.to_str().expect("validated UTF-8"),
            prefix_c.to_str().expect("validated UTF-8"),
            param_name_c.to_str().expect("validated UTF-8"),
        ) {
            Ok(value) => value,
            Err(rt::Error::InvalidConnectionString(message)) => {
                log_debug(&message);
                set_last_error_message(message);
                return NFC_COMMON_ERROR;
            }
            Err(_) => return NFC_COMMON_INVALID,
        };

        let write_result = write_checked_c_buffer(
            param_buffer,
            parsed.as_bytes(),
            format!(
                "Parameter value too long ({} bytes, buffer size {})",
                parsed.len(),
                param_buffer.len()
            ),
        );
        if write_result != NFC_COMMON_SUCCESS {
            return write_result;
        }

        log_debug(&format!(
            "Extracted parameter '{}'='{}' from connstring",
            String::from_utf8_lossy(param_name_c.to_bytes()),
            parsed
        ));

        reset_last_error();

        NFC_COMMON_SUCCESS
    }
}

pub unsafe fn nfc_build_connstring(
    dest: *mut c_char,
    dest_size: size_t,
    driver_name: *const c_char,
    param_name: *const c_char,
    param_value: *const c_char,
) -> c_int {
    unsafe {
        if dest_size == 0 {
            return set_error_and_return(
                NFC_COMMON_INVALID,
                "Zero-size destination buffer in connstring building".to_string(),
            );
        }

        let dest_ptr =
            match validate_mut_ptr(dest, "NULL destination buffer in connstring building") {
                Ok(ptr) => ptr,
                Err(code) => return code,
            };
        let driver_name_c =
            match validate_non_null(driver_name, "NULL driver_name in connstring building") {
                Ok(value) => value,
                Err(code) => return code,
            };
        let param_name_c =
            match validate_non_null(param_name, "NULL param_name in connstring building") {
                Ok(value) => value,
                Err(code) => return code,
            };
        let param_value_c =
            match validate_non_null(param_value, "NULL param_value in connstring building") {
                Ok(value) => value,
                Err(code) => return code,
            };

        if let Err(code) = ensure_utf8(driver_name_c, "driver_name") {
            return code;
        }
        if let Err(code) = ensure_utf8(param_name_c, "param_name") {
            return code;
        }
        if let Err(code) = ensure_utf8(param_value_c, "param_value") {
            return code;
        }

        let result = match rt::build_connstring(
            driver_name_c.to_str().expect("validated UTF-8"),
            param_name_c.to_str().expect("validated UTF-8"),
            param_value_c.to_str().expect("validated UTF-8"),
        ) {
            Ok(value) => value,
            Err(rt::Error::BufferTooSmall { needed, .. }) => {
                return set_error_and_return(
                    NFC_COMMON_ERROR,
                    format!(
                        "Connection string buffer overflow (need {} bytes, have {})",
                        needed, dest_size
                    ),
                );
            }
            Err(_) => return NFC_COMMON_INVALID,
        };
        let dest_buffer = std::slice::from_raw_parts_mut(dest_ptr, dest_size);
        let write_result = write_checked_c_buffer(
            dest_buffer,
            result.as_str().as_bytes(),
            format!(
                "Connection string buffer overflow (need {} bytes, have {})",
                result.as_str().len() + 1,
                dest_buffer.len()
            ),
        );
        if write_result != NFC_COMMON_SUCCESS {
            return write_result;
        }

        log_debug(&format!("Built connection string: '{}'", result));

        reset_last_error();

        NFC_COMMON_SUCCESS
    }
}

pub fn nfc_get_last_error() -> *const c_char {
    LAST_ERROR.with(|cell| match cell.borrow().as_ref() {
        Some(message) => message.as_ptr(),
        None => ptr::null(),
    })
}

pub fn nfc_clear_last_error() {
    ffi_catch_unwind_void("nfc_clear_last_error", reset_last_error);
}

pub unsafe fn nfc_set_last_error(message: *const c_char) {
    ffi_catch_unwind_void("nfc_set_last_error", || {
        if message.is_null() {
            reset_last_error();
            return;
        }

        let c_message = unsafe { CStr::from_ptr(message) };
        let owned = String::from_utf8_lossy(c_message.to_bytes()).into_owned();
        set_last_error_message(owned);
    });
}

/// Free memory allocated by Rust FFI helpers
pub unsafe fn nfc_rs_free(ptr: *mut c_void) {
    ffi_catch_unwind_void("nfc_rs_free", || unsafe {
        release_allocated_ptr(ptr);
    });
}

unsafe fn connstring_decode_impl(
    connstring: *const c_char,
    driver_name: *const c_char,
    bus_name: *const c_char,
    pparam1: *mut *mut c_char,
    pparam2: *mut *mut c_char,
) -> c_int {
    unsafe {
        if connstring.is_null() {
            return 0;
        }

        let driver_bytes = if driver_name.is_null() {
            &[][..]
        } else {
            CStr::from_ptr(driver_name).to_bytes()
        };
        let bus_bytes = if bus_name.is_null() {
            &[][..]
        } else {
            CStr::from_ptr(bus_name).to_bytes()
        };

        let slice = std::slice::from_raw_parts(
            connstring.cast::<u8>(),
            bounded_strlen(connstring, NFC_BUFSIZE_CONNSTRING),
        );
        let Some((result, param1_segment, param2_segment)) =
            rt::decode_connstring_segments_bytes(slice, driver_bytes, bus_bytes)
        else {
            return 0;
        };

        if !pparam1.is_null() {
            if result >= 2 {
                match alloc_and_copy(param1_segment.unwrap_or(&[])) {
                    Ok(ptr_value) => {
                        *pparam1 = ptr_value;
                    }
                    Err(()) => {
                        *pparam1 = ptr::null_mut();
                        if !pparam2.is_null() {
                            *pparam2 = ptr::null_mut();
                        }
                        return 0;
                    }
                }
            } else {
                *pparam1 = ptr::null_mut();
            }
        }

        if !pparam2.is_null() {
            if result >= 3 {
                match alloc_and_copy(param2_segment.unwrap_or(&[])) {
                    Ok(ptr_value) => {
                        *pparam2 = ptr_value;
                    }
                    Err(()) => {
                        if !pparam1.is_null() {
                            release_allocated_ptr((*pparam1).cast::<c_void>());
                            *pparam1 = ptr::null_mut();
                        }
                        *pparam2 = ptr::null_mut();
                        return 0;
                    }
                }
            } else {
                *pparam2 = ptr::null_mut();
            }
        }

        result
    }
}

pub unsafe fn connstring_decode(
    connstring: *const c_char,
    driver_name: *const c_char,
    bus_name: *const c_char,
    pparam1: *mut *mut c_char,
    pparam2: *mut *mut c_char,
) -> c_int {
    unsafe {
        ffi_catch_unwind_int("connstring_decode", NFC_COMMON_ERROR, || {
            connstring_decode_impl(connstring, driver_name, bus_name, pparam1, pparam2)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{test_clear_last_log, test_get_last_log};
    use std::ffi::{CStr, CString};

    fn free_if_not_null(ptr: *mut c_char) {
        if !ptr.is_null() {
            unsafe { release_allocated_ptr(ptr as *mut c_void) };
        }
    }

    #[test]
    fn last_error_roundtrip() {
        unsafe {
            nfc_clear_last_error();
            assert!(nfc_get_last_error().is_null());

            let msg = CString::new("roundtrip-error").unwrap();
            nfc_set_last_error(msg.as_ptr());

            let ptr = nfc_get_last_error();
            assert!(!ptr.is_null());
            let recovered = CStr::from_ptr(ptr).to_str().unwrap();
            assert_eq!(recovered, "roundtrip-error");

            nfc_clear_last_error();
            assert!(nfc_get_last_error().is_null());
        }
    }

    #[test]
    fn decode_driver_only_sets_null_outputs() {
        unsafe {
            let conn = CString::new("pn532").unwrap();
            let driver = CString::new("pn532").unwrap();
            let mut param1: *mut c_char = ptr::null_mut();
            let mut param2: *mut c_char = ptr::null_mut();

            let level = connstring_decode(
                conn.as_ptr(),
                driver.as_ptr(),
                ptr::null(),
                &mut param1,
                &mut param2,
            );

            assert_eq!(level, 1);
            assert!(param1.is_null());
            assert!(param2.is_null());
        }
    }

    #[test]
    fn decode_with_parameters_returns_segments() {
        unsafe {
            let conn = CString::new("pn53x_usb:/dev/usb:115200").unwrap();
            let driver = CString::new("pn53x_usb").unwrap();
            let mut param1: *mut c_char = ptr::null_mut();
            let mut param2: *mut c_char = ptr::null_mut();

            let level = connstring_decode(
                conn.as_ptr(),
                driver.as_ptr(),
                ptr::null(),
                &mut param1,
                &mut param2,
            );

            assert_eq!(level, 3);
            assert!(!param1.is_null());
            assert!(!param2.is_null());

            let first = CStr::from_ptr(param1).to_bytes();
            let second = CStr::from_ptr(param2).to_bytes();
            assert_eq!(first, b"/dev/usb");
            assert_eq!(second, b"115200");

            free_if_not_null(param1);
            free_if_not_null(param2);
        }
    }

    #[test]
    fn decode_mismatched_driver_leaves_outputs_untouched() {
        unsafe {
            let conn = CString::new("pn53x_usb:/dev/usb").unwrap();
            let driver = CString::new("pn532_spi").unwrap();
            let mut param1: *mut c_char = 0x1 as *mut c_char;
            let mut param2: *mut c_char = 0x2 as *mut c_char;

            let level = connstring_decode(
                conn.as_ptr(),
                driver.as_ptr(),
                ptr::null(),
                &mut param1,
                &mut param2,
            );

            assert_eq!(level, 0);
            assert_eq!(param1 as usize, 0x1);
            assert_eq!(param2 as usize, 0x2);
        }
    }

    #[test]
    fn parse_connstring_logs_on_prefix_mismatch() {
        unsafe {
            test_clear_last_log();
            crate::logger::log_init(3);
            let conn = CString::new("pn53x_usb:/dev/usb").unwrap();
            let prefix = CString::new("pn532").unwrap();
            let mut buf = [0u8; 64];
            let rc = nfc_parse_connstring(
                conn.as_ptr(),
                prefix.as_ptr(),
                CString::new("param").unwrap().as_ptr(),
                buf.as_mut_ptr() as *mut c_char,
                buf.len(),
            );
            // should be error
            assert_ne!(rc, NFC_COMMON_SUCCESS);
            let logged = test_get_last_log();
            assert!(logged.is_some());
            assert!(logged.unwrap().contains("does not match prefix"));
        }
    }

    #[test]
    fn parse_connstring_rejects_truncated_output_buffer() {
        unsafe {
            let conn = CString::new("pn53x_usb:path=/dev/ttyUSB0").unwrap();
            let prefix = CString::new("pn53x_usb").unwrap();
            let param = CString::new("path").unwrap();
            let mut buf = [0 as c_char; 4];

            let rc = nfc_parse_connstring(
                conn.as_ptr(),
                prefix.as_ptr(),
                param.as_ptr(),
                buf.as_mut_ptr(),
                buf.len(),
            );

            assert_eq!(rc, NFC_COMMON_ERROR);
            let err = nfc_get_last_error();
            assert!(!err.is_null());
            let recovered = CStr::from_ptr(err).to_str().unwrap();
            assert!(recovered.contains("Parameter value too long"));
        }
    }

    #[test]
    fn build_connstring_rejects_truncated_output_buffer() {
        unsafe {
            let driver = CString::new("pn53x_usb").unwrap();
            let param = CString::new("path").unwrap();
            let value = CString::new("/dev/ttyUSB0").unwrap();
            let mut buf = [0 as c_char; 8];

            let rc = nfc_build_connstring(
                buf.as_mut_ptr(),
                buf.len(),
                driver.as_ptr(),
                param.as_ptr(),
                value.as_ptr(),
            );

            assert_eq!(rc, NFC_COMMON_ERROR);
            let err = nfc_get_last_error();
            assert!(!err.is_null());
            let recovered = CStr::from_ptr(err).to_str().unwrap();
            assert!(recovered.contains("Connection string buffer overflow"));
        }
    }

    #[test]
    fn ffi_catch_unwind_maps_panic_to_error() {
        // Ensure the panic boundary converts a panic into the
        // appropriate external error code instead of letting the
        // panic unwind across the FFI boundary.
        let rc = ffi_catch_unwind_int("test_panic", NFC_COMMON_ERROR, || panic!("boom"));
        assert_eq!(rc, NFC_COMMON_ERROR);
    }

    #[test]
    fn ffi_catch_unwind_ptr_maps_panic_to_null() {
        reset_last_error();
        let ptr = ffi_catch_unwind_ptr::<c_char, _>("test_ptr_panic", || panic!("boom"));
        assert!(ptr.is_null());

        let err = nfc_get_last_error();
        assert!(!err.is_null());
        let recovered = unsafe { CStr::from_ptr(err) }.to_str().unwrap();
        assert!(recovered.contains("panic in test_ptr_panic"));
    }

    #[test]
    fn ffi_catch_unwind_void_maps_panic_to_last_error() {
        reset_last_error();
        ffi_catch_unwind_void("test_void_panic", || panic!("boom"));

        let err = nfc_get_last_error();
        assert!(!err.is_null());
        let recovered = unsafe { CStr::from_ptr(err) }.to_str().unwrap();
        assert!(recovered.contains("panic in test_void_panic"));
    }
}
