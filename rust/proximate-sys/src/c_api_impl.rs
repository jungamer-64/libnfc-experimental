// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// This Rust crate contains libnfc FFI support code together with Rust
// implementations of selected libnfc helpers.
//
// Libnfc historical contributors:
// Copyright (C) 2009      Roel Verdult
// Copyright (C) 2009-2013 Romuald Conty
// Copyright (C) 2010-2012 Romain Tartiere
// Copyright (C) 2010-2013 Philippe Teuwen
// Copyright (C) 2012-2013 Ludovic Rousseau
// Copyright (C) 2020      Adam Laurie
// See AUTHORS file for a more comprehensive list of contributors.

use crate::logger;
use libc::{c_char, c_int, c_void};
use std::cell::RefCell;
use std::ffi::CString;
use std::panic;
#[cfg(not(feature = "test_no_catch"))]
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

unsafe extern "C" {
    #[link_name = "free"]
    fn c_free(ptr: *mut c_void);
}

pub(crate) unsafe fn release_allocated_ptr(ptr: *mut c_void) {
    if !ptr.is_null() {
        unsafe { c_free(ptr) };
    }
}

#[cfg(not(feature = "test_no_catch"))]
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

#[cfg(test)]
pub(crate) fn nfc_get_last_error() -> *const c_char {
    LAST_ERROR.with(|cell| match cell.borrow().as_ref() {
        Some(message) => message.as_ptr(),
        None => std::ptr::null(),
    })
}

#[cfg(test)]
pub(crate) fn nfc_clear_last_error() {
    ffi_catch_unwind_void("nfc_clear_last_error", reset_last_error);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{CStr, CString};

    #[test]
    fn last_error_roundtrip() {
        unsafe {
            nfc_clear_last_error();
            assert!(nfc_get_last_error().is_null());

            let msg = CString::new("roundtrip-error").unwrap();
            let c_message = CStr::from_ptr(msg.as_ptr());
            let owned = String::from_utf8_lossy(c_message.to_bytes()).into_owned();
            set_last_error_message(owned);

            let ptr = nfc_get_last_error();
            assert!(!ptr.is_null());
            let recovered = CStr::from_ptr(ptr).to_str().unwrap();
            assert_eq!(recovered, "roundtrip-error");

            nfc_clear_last_error();
            assert!(nfc_get_last_error().is_null());
        }
    }

    #[test]
    #[cfg_attr(feature = "test_no_catch", should_panic(expected = "boom"))]
    fn ffi_catch_unwind_maps_panic_to_error() {
        // Ensure the panic boundary converts a panic into the
        // appropriate external error code instead of letting the
        // panic unwind across the FFI boundary.
        let _rc = ffi_catch_unwind_int("test_panic", NFC_COMMON_ERROR, || panic!("boom"));
        #[cfg(not(feature = "test_no_catch"))]
        assert_eq!(_rc, NFC_COMMON_ERROR);
    }

    #[test]
    #[cfg_attr(feature = "test_no_catch", should_panic(expected = "boom"))]
    fn ffi_catch_unwind_ptr_maps_panic_to_null() {
        reset_last_error();
        let _ptr = ffi_catch_unwind_ptr::<c_char, _>("test_ptr_panic", || panic!("boom"));
        #[cfg(not(feature = "test_no_catch"))]
        {
            assert!(_ptr.is_null());

            let err = nfc_get_last_error();
            assert!(!err.is_null());
            let recovered = unsafe { CStr::from_ptr(err) }.to_str().unwrap();
            assert!(recovered.contains("panic in test_ptr_panic"));
        }
    }

    #[test]
    #[cfg_attr(feature = "test_no_catch", should_panic(expected = "boom"))]
    fn ffi_catch_unwind_void_maps_panic_to_last_error() {
        reset_last_error();
        ffi_catch_unwind_void("test_void_panic", || panic!("boom"));

        #[cfg(not(feature = "test_no_catch"))]
        {
            let err = nfc_get_last_error();
            assert!(!err.is_null());
            let recovered = unsafe { CStr::from_ptr(err) }.to_str().unwrap();
            assert!(recovered.contains("panic in test_void_panic"));
        }
    }
}
