// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Ported from libnfc/nfc.c, libnfc/nfc-device.c, and libnfc/conf.c.
//
// Libnfc historical contributors:
// Copyright (C) 2009      Roel Verdult
// Copyright (C) 2009-2013 Romuald Conty
// Copyright (C) 2010-2012 Romain Tartiere
// Copyright (C) 2010-2013 Philippe Teuwen
// Copyright (C) 2012-2013 Ludovic Rousseau
// Copyright (C) 2020      Adam Laurie
// See AUTHORS file for a more comprehensive list of contributors.

use crate::{
    LOG_PRIORITY_DEBUG, LOG_PRIORITY_NONE, NFC_BUFSIZE_CONNSTRING, ffi_catch_unwind_ptr,
    ffi_catch_unwind_void, log_error, log_message, release_allocated_ptr, reset_last_error,
    set_last_error_message,
};
use libc::{c_char, c_int, c_uint, c_void};
use std::mem::size_of;
use std::ptr;

const DEVICE_NAME_LENGTH: usize = 256;
const MAX_USER_DEFINED_DEVICES: usize = 4;
const DEFAULT_CONTEXT_LOG_LEVEL: u32 = if cfg!(libnfc_debug) { 3 } else { 1 };
const USER_DEFINED_DEFAULT_DEVICE_NAME: &[u8] = b"user defined default device";
const USER_DEFINED_DEVICE_NAME: &[u8] = b"user defined device";
const ENV_LIBNFC_DEFAULT_DEVICE: &[u8] = b"LIBNFC_DEFAULT_DEVICE\0";
const ENV_LIBNFC_DEVICE: &[u8] = b"LIBNFC_DEVICE\0";
const ENV_LIBNFC_AUTO_SCAN: &[u8] = b"LIBNFC_AUTO_SCAN\0";
const ENV_LIBNFC_INTRUSIVE_SCAN: &[u8] = b"LIBNFC_INTRUSIVE_SCAN\0";
const ENV_LIBNFC_LOG_LEVEL: &[u8] = b"LIBNFC_LOG_LEVEL\0";

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct nfc_driver {
    _private: [u8; 0],
}

#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[repr(C)]
pub struct nfc_user_defined_device {
    pub name: [c_char; DEVICE_NAME_LENGTH],
    pub connstring: [c_char; NFC_BUFSIZE_CONNSTRING],
    pub optional: bool,
}

#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[repr(C)]
pub struct nfc_context {
    pub allow_autoscan: bool,
    pub allow_intrusive_scan: bool,
    pub log_level: u32,
    pub user_defined_devices: [nfc_user_defined_device; MAX_USER_DEFINED_DEVICES],
    pub user_defined_device_count: c_uint,
}

#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[repr(C)]
pub struct nfc_device {
    pub context: *const nfc_context,
    pub driver: *const nfc_driver,
    pub driver_data: *mut c_void,
    pub chip_data: *mut c_void,
    pub name: [c_char; DEVICE_NAME_LENGTH],
    pub connstring: [c_char; NFC_BUFSIZE_CONNSTRING],
    pub bCrc: bool,
    pub bPar: bool,
    pub bEasyFraming: bool,
    pub bInfiniteSelect: bool,
    pub bAutoIso14443_4: bool,
    pub btSupportByte: u8,
    pub last_error: c_int,
}

#[cfg(all(not(test), libnfc_external_bridges))]
unsafe extern "C" {
    fn nfc_rs_context_conf_load(context: *mut nfc_context);
    fn nfc_rs_context_log_init(context: *const nfc_context);
}

#[cfg(all(not(test), libnfc_external_bridges))]
unsafe fn bridge_context_conf_load(context: *mut nfc_context) {
    unsafe { nfc_rs_context_conf_load(context) };
}

#[cfg(any(test, not(libnfc_external_bridges)))]
unsafe fn bridge_context_conf_load(context: *mut nfc_context) {
    #[cfg(test)]
    unsafe {
        nfc_rs_context_conf_load(context);
    }

    #[cfg(not(test))]
    let _ = context;
}

#[cfg(all(not(test), libnfc_external_bridges))]
unsafe fn bridge_context_log_init(context: *const nfc_context) {
    unsafe { nfc_rs_context_log_init(context) };
}

#[cfg(any(test, not(libnfc_external_bridges)))]
unsafe fn bridge_context_log_init(context: *const nfc_context) {
    #[cfg(test)]
    unsafe {
        nfc_rs_context_log_init(context);
    }

    #[cfg(not(test))]
    let _ = context;
}

unsafe fn allocate_zeroed<T>(label: &str) -> *mut T {
    let ptr = unsafe { libc::calloc(1, size_of::<T>()) as *mut T };
    if ptr.is_null() {
        let message = format!("Unable to allocate {}", label);
        log_error(&message);
        set_last_error_message(message);
    }
    ptr
}

unsafe fn nfc_context_alloc_defaults_impl() -> *mut nfc_context {
    let context = unsafe { allocate_zeroed::<nfc_context>("nfc_context") };
    if context.is_null() {
        return ptr::null_mut();
    }

    unsafe {
        (*context).allow_autoscan = true;
        (*context).allow_intrusive_scan = false;
        (*context).log_level = DEFAULT_CONTEXT_LOG_LEVEL;
    }

    reset_last_error();
    context
}

unsafe fn nfc_device_new_impl(
    context: *const nfc_context,
    connstring: *const c_char,
) -> *mut nfc_device {
    if connstring.is_null() {
        let message = "NULL connstring in nfc_device_new".to_string();
        log_error(&message);
        set_last_error_message(message);
        return ptr::null_mut();
    }

    let device = unsafe { allocate_zeroed::<nfc_device>("nfc_device") };
    if device.is_null() {
        return ptr::null_mut();
    }

    let copy_len = crate::bounded_strlen(connstring, NFC_BUFSIZE_CONNSTRING.saturating_sub(1));
    unsafe {
        (*device).context = context;
        if copy_len > 0 {
            ptr::copy_nonoverlapping(connstring, (*device).connstring.as_mut_ptr(), copy_len);
        }
        (*device).connstring[copy_len] = 0;
    }

    reset_last_error();
    device
}

fn set_copy_error(message: &str) {
    log_error(message);
    set_last_error_message(message.to_string());
}

unsafe fn copy_bytes_to_buffer(
    dst: *mut c_char,
    dst_size: usize,
    src: &[u8],
    error_message: &str,
) -> bool {
    if src.len() >= dst_size {
        set_copy_error(error_message);
        return false;
    }

    unsafe {
        if !src.is_empty() {
            ptr::copy_nonoverlapping(src.as_ptr() as *const c_char, dst, src.len());
        }
        *dst.add(src.len()) = 0;
    }

    true
}

unsafe fn copy_c_string_to_buffer(
    dst: *mut c_char,
    dst_size: usize,
    src: *const c_char,
    error_message: &str,
) -> bool {
    let length = crate::bounded_strlen(src, dst_size);
    if length >= dst_size {
        set_copy_error(error_message);
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

unsafe fn apply_user_defined_device(
    context: *mut nfc_context,
    device_name: &[u8],
    connstring: *const c_char,
    count: c_uint,
    connstring_error_message: &str,
) -> bool {
    unsafe {
        let device = &mut (*context).user_defined_devices[0];

        if !copy_bytes_to_buffer(
            device.name.as_mut_ptr(),
            DEVICE_NAME_LENGTH,
            device_name,
            "Failed to copy device name",
        ) {
            return false;
        }

        if !copy_c_string_to_buffer(
            device.connstring.as_mut_ptr(),
            NFC_BUFSIZE_CONNSTRING,
            connstring,
            connstring_error_message,
        ) {
            return false;
        }

        (*context).user_defined_device_count = count;
    }

    true
}

fn apply_boolean_string(value: *const c_char, target: &mut bool) {
    if value.is_null() {
        return;
    }

    let bytes = unsafe { std::ffi::CStr::from_ptr(value).to_bytes() };
    if !(*target) {
        if matches!(bytes, b"yes" | b"true" | b"1") {
            *target = true;
        }
    } else if matches!(bytes, b"no" | b"false" | b"0") {
        *target = false;
    }
}

unsafe fn getenv(name: &[u8]) -> *mut c_char {
    unsafe { libc::getenv(name.as_ptr() as *const c_char) }
}

fn fixed_c_buffer_to_string(buffer: &[c_char]) -> String {
    let length = buffer
        .iter()
        .position(|&ch| ch == 0)
        .unwrap_or(buffer.len());
    let bytes: Vec<u8> = buffer[..length].iter().map(|&ch| ch as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

unsafe fn log_context_state(context: *const nfc_context) {
    let first_priority = if cfg!(libnfc_debug) {
        LOG_PRIORITY_NONE
    } else {
        LOG_PRIORITY_DEBUG
    };

    unsafe {
        log_message(
            first_priority,
            &format!("log_level is set to {}", (*context).log_level),
        );
        log_message(
            LOG_PRIORITY_DEBUG,
            &format!(
                "allow_autoscan is set to {}",
                if (*context).allow_autoscan {
                    "true"
                } else {
                    "false"
                }
            ),
        );
        log_message(
            LOG_PRIORITY_DEBUG,
            &format!(
                "allow_intrusive_scan is set to {}",
                if (*context).allow_intrusive_scan {
                    "true"
                } else {
                    "false"
                }
            ),
        );
        log_message(
            LOG_PRIORITY_DEBUG,
            &format!(
                "{} device(s) defined by user",
                (*context).user_defined_device_count
            ),
        );

        for index in 0..((*context).user_defined_device_count as usize) {
            let device = &(*context).user_defined_devices[index];
            log_message(
                LOG_PRIORITY_DEBUG,
                &format!(
                    "  #{} name: \"{}\", connstring: \"{}\"",
                    index,
                    fixed_c_buffer_to_string(&device.name),
                    fixed_c_buffer_to_string(&device.connstring)
                ),
            );
        }
    }
}

unsafe fn free_context_allocation(context: *mut nfc_context) {
    unsafe { release_allocated_ptr(context as *mut c_void) };
}

unsafe fn nfc_context_new_impl() -> *mut nfc_context {
    let context = unsafe { nfc_context_alloc_defaults_impl() };
    if context.is_null() {
        return ptr::null_mut();
    }

    if cfg!(libnfc_envvars) {
        let default_device = unsafe { getenv(ENV_LIBNFC_DEFAULT_DEVICE) };
        if !default_device.is_null()
            && !unsafe {
                apply_user_defined_device(
                    context,
                    USER_DEFINED_DEFAULT_DEVICE_NAME,
                    default_device,
                    1,
                    "Failed to copy LIBNFC_DEFAULT_DEVICE environment variable",
                )
            }
        {
            unsafe { free_context_allocation(context) };
            return ptr::null_mut();
        }
    }

    if cfg!(libnfc_conffiles) {
        unsafe { bridge_context_conf_load(context) };
    }

    if cfg!(libnfc_envvars) {
        let selected_device = unsafe { getenv(ENV_LIBNFC_DEVICE) };
        if !selected_device.is_null()
            && !unsafe {
                apply_user_defined_device(
                    context,
                    USER_DEFINED_DEVICE_NAME,
                    selected_device,
                    1,
                    "Failed to copy LIBNFC_DEVICE environment variable",
                )
            }
        {
            unsafe { free_context_allocation(context) };
            return ptr::null_mut();
        }

        unsafe {
            apply_boolean_string(
                getenv(ENV_LIBNFC_AUTO_SCAN) as *const c_char,
                &mut (*context).allow_autoscan,
            );
            apply_boolean_string(
                getenv(ENV_LIBNFC_INTRUSIVE_SCAN) as *const c_char,
                &mut (*context).allow_intrusive_scan,
            );

            let env_log_level = getenv(ENV_LIBNFC_LOG_LEVEL);
            if !env_log_level.is_null() {
                (*context).log_level = libc::atoi(env_log_level) as u32;
            }
        }
    }

    unsafe {
        bridge_context_log_init(context);
        log_context_state(context);
    }

    reset_last_error();
    context
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_context_alloc_defaults() -> *mut nfc_context {
    ffi_catch_unwind_ptr("nfc_context_alloc_defaults", || unsafe {
        nfc_context_alloc_defaults_impl()
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_context_new() -> *mut nfc_context {
    ffi_catch_unwind_ptr("nfc_context_new", || unsafe { nfc_context_new_impl() })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_new(
    context: *const nfc_context,
    connstring: *const c_char,
) -> *mut nfc_device {
    ffi_catch_unwind_ptr("nfc_device_new", || unsafe {
        nfc_device_new_impl(context, connstring)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_free(device: *mut nfc_device) {
    ffi_catch_unwind_void("nfc_device_free", || unsafe {
        if device.is_null() {
            return;
        }

        release_allocated_ptr((*device).driver_data);
        release_allocated_ptr(device as *mut c_void);
    });
}

#[cfg(test)]
#[derive(Clone, Default)]
struct BridgeTestState {
    conf_load_calls: usize,
    log_init_calls: usize,
    events: Vec<&'static str>,
    configured_devices: Vec<(Vec<u8>, Vec<u8>, bool)>,
    configured_allow_autoscan: Option<bool>,
    configured_allow_intrusive_scan: Option<bool>,
    configured_log_level: Option<u32>,
}

#[cfg(test)]
thread_local! {
    static TEST_BRIDGE_STATE: std::cell::RefCell<BridgeTestState> =
        std::cell::RefCell::new(BridgeTestState::default());
}

#[cfg(test)]
fn reset_test_bridge_state() {
    TEST_BRIDGE_STATE.with(|cell| {
        *cell.borrow_mut() = BridgeTestState::default();
    });
}

#[cfg(test)]
fn update_test_bridge_state<F>(update: F)
where
    F: FnOnce(&mut BridgeTestState),
{
    TEST_BRIDGE_STATE.with(|cell| update(&mut cell.borrow_mut()));
}

#[cfg(test)]
fn snapshot_test_bridge_state() -> BridgeTestState {
    TEST_BRIDGE_STATE.with(|cell| cell.borrow().clone())
}

#[cfg(test)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_rs_context_conf_load(context: *mut nfc_context) {
    let snapshot = TEST_BRIDGE_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        state.conf_load_calls += 1;
        state.events.push("conf_load");
        state.clone()
    });

    if context.is_null() {
        return;
    }

    unsafe {
        if let Some(value) = snapshot.configured_allow_autoscan {
            (*context).allow_autoscan = value;
        }
        if let Some(value) = snapshot.configured_allow_intrusive_scan {
            (*context).allow_intrusive_scan = value;
        }
        if let Some(value) = snapshot.configured_log_level {
            (*context).log_level = value;
        }

        for (index, (name, connstring, optional)) in snapshot
            .configured_devices
            .iter()
            .enumerate()
            .take(MAX_USER_DEFINED_DEVICES)
        {
            let device = &mut (*context).user_defined_devices[index];
            let _ = copy_bytes_to_buffer(
                device.name.as_mut_ptr(),
                DEVICE_NAME_LENGTH,
                name,
                "test name copy failed",
            );
            let _ = copy_bytes_to_buffer(
                device.connstring.as_mut_ptr(),
                NFC_BUFSIZE_CONNSTRING,
                connstring,
                "test connstring copy failed",
            );
            device.optional = *optional;
        }

        if !snapshot.configured_devices.is_empty() {
            (*context).user_defined_device_count = snapshot
                .configured_devices
                .len()
                .min(MAX_USER_DEFINED_DEVICES)
                as c_uint;
        }
    }
}

#[cfg(test)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_rs_context_log_init(_context: *const nfc_context) {
    TEST_BRIDGE_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        state.log_init_calls += 1;
        state.events.push("log_init");
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{CStr, CString, OsString};
    use std::sync::{Mutex, OnceLock};

    fn release_context(context: *mut nfc_context) {
        unsafe { release_allocated_ptr(context as *mut c_void) };
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct ScopedEnv {
        saved: Vec<(String, Option<OsString>)>,
    }

    impl ScopedEnv {
        fn new() -> Self {
            Self { saved: Vec::new() }
        }

        fn save(&mut self, key: &str) {
            if self.saved.iter().any(|(saved_key, _)| saved_key == key) {
                return;
            }
            self.saved.push((key.to_string(), std::env::var_os(key)));
        }

        fn set(&mut self, key: &str, value: &str) {
            self.save(key);
            unsafe { std::env::set_var(key, value) };
        }

        fn remove(&mut self, key: &str) {
            self.save(key);
            unsafe { std::env::remove_var(key) };
        }
    }

    impl Drop for ScopedEnv {
        fn drop(&mut self) {
            for (key, value) in self.saved.drain(..).rev() {
                match value {
                    Some(value) => unsafe { std::env::set_var(&key, value) },
                    None => unsafe { std::env::remove_var(&key) },
                }
            }
        }
    }

    fn clear_env(env: &mut ScopedEnv) {
        for key in [
            "LIBNFC_DEFAULT_DEVICE",
            "LIBNFC_DEVICE",
            "LIBNFC_AUTO_SCAN",
            "LIBNFC_INTRUSIVE_SCAN",
            "LIBNFC_LOG_LEVEL",
        ] {
            env.remove(key);
        }
    }

    #[test]
    fn context_alloc_defaults_matches_c_defaults() {
        let context = unsafe { nfc_context_alloc_defaults() };
        assert!(!context.is_null());

        unsafe {
            assert!((*context).allow_autoscan);
            assert!(!(*context).allow_intrusive_scan);
            assert_eq!((*context).log_level, DEFAULT_CONTEXT_LOG_LEVEL);
            assert_eq!((*context).user_defined_device_count, 0);
            assert_eq!((*context).user_defined_devices[0].name[0], 0);
            assert_eq!((*context).user_defined_devices[0].connstring[0], 0);
            assert!(!(*context).user_defined_devices[0].optional);
        }

        release_context(context);
    }

    #[test]
    fn context_new_applies_defaults_and_bridge_order() {
        let _env_guard = env_lock().lock().unwrap();
        let mut env = ScopedEnv::new();
        clear_env(&mut env);
        reset_test_bridge_state();
        crate::test_clear_last_log();

        let context = unsafe { nfc_context_new() };
        assert!(!context.is_null());

        unsafe {
            assert!((*context).allow_autoscan);
            assert!(!(*context).allow_intrusive_scan);
            assert_eq!((*context).log_level, DEFAULT_CONTEXT_LOG_LEVEL);
            assert_eq!((*context).user_defined_device_count, 0);
        }

        let bridge_state = snapshot_test_bridge_state();
        assert_eq!(bridge_state.conf_load_calls, 1);
        assert_eq!(bridge_state.log_init_calls, 1);
        assert_eq!(bridge_state.events, vec!["conf_load", "log_init"]);
        assert_eq!(
            crate::test_get_last_log().as_deref(),
            Some("0 device(s) defined by user")
        );

        release_context(context);
    }

    #[test]
    fn context_new_reflects_libnfc_default_device() {
        let _env_guard = env_lock().lock().unwrap();
        let mut env = ScopedEnv::new();
        clear_env(&mut env);
        env.set("LIBNFC_DEFAULT_DEVICE", "pn532_uart:/dev/ttyUSB0");
        reset_test_bridge_state();

        let context = unsafe { nfc_context_new() };
        assert!(!context.is_null());

        unsafe {
            assert_eq!((*context).user_defined_device_count, 1);
            let device = &(*context).user_defined_devices[0];
            assert_eq!(
                CStr::from_ptr(device.name.as_ptr()).to_bytes(),
                USER_DEFINED_DEFAULT_DEVICE_NAME
            );
            assert_eq!(
                CStr::from_ptr(device.connstring.as_ptr()).to_bytes(),
                b"pn532_uart:/dev/ttyUSB0"
            );
        }

        release_context(context);
    }

    #[test]
    fn context_new_libnfc_device_overrides_config_and_default_device() {
        let _env_guard = env_lock().lock().unwrap();
        let mut env = ScopedEnv::new();
        clear_env(&mut env);
        env.set("LIBNFC_DEFAULT_DEVICE", "pn532_uart:/dev/ttyUSB0");
        env.set("LIBNFC_DEVICE", "pn53x_usb:001:002");
        reset_test_bridge_state();
        update_test_bridge_state(|state| {
            state.configured_devices.push((
                b"config device".to_vec(),
                b"pn532_spi:/dev/spidev0.0".to_vec(),
                true,
            ));
            state.configured_devices.push((
                b"config device 2".to_vec(),
                b"pn532_i2c:/dev/i2c-1".to_vec(),
                false,
            ));
        });

        let context = unsafe { nfc_context_new() };
        assert!(!context.is_null());

        unsafe {
            assert_eq!((*context).user_defined_device_count, 1);
            let device = &(*context).user_defined_devices[0];
            assert_eq!(
                CStr::from_ptr(device.name.as_ptr()).to_bytes(),
                USER_DEFINED_DEVICE_NAME
            );
            assert_eq!(
                CStr::from_ptr(device.connstring.as_ptr()).to_bytes(),
                b"pn53x_usb:001:002"
            );
        }

        release_context(context);
    }

    #[test]
    fn context_new_applies_env_boolean_and_log_level_overrides() {
        let _env_guard = env_lock().lock().unwrap();
        let mut env = ScopedEnv::new();
        clear_env(&mut env);
        env.set("LIBNFC_AUTO_SCAN", "false");
        env.set("LIBNFC_INTRUSIVE_SCAN", "true");
        env.set("LIBNFC_LOG_LEVEL", "42");
        reset_test_bridge_state();
        update_test_bridge_state(|state| {
            state.configured_allow_autoscan = Some(true);
            state.configured_allow_intrusive_scan = Some(false);
            state.configured_log_level = Some(7);
        });

        let context = unsafe { nfc_context_new() };
        assert!(!context.is_null());

        unsafe {
            assert!(!(*context).allow_autoscan);
            assert!((*context).allow_intrusive_scan);
            assert_eq!((*context).log_level, 42);
        }

        release_context(context);
    }

    #[test]
    fn context_new_keeps_lowercase_only_boolean_semantics() {
        let _env_guard = env_lock().lock().unwrap();
        let mut env = ScopedEnv::new();
        clear_env(&mut env);
        env.set("LIBNFC_INTRUSIVE_SCAN", "True");
        reset_test_bridge_state();

        let context = unsafe { nfc_context_new() };
        assert!(!context.is_null());

        unsafe {
            assert!(!(*context).allow_intrusive_scan);
        }

        release_context(context);
    }

    #[test]
    fn device_new_initializes_expected_fields() {
        let connstring = CString::new("pn53x_usb:/dev/usb").unwrap();
        let device = unsafe { nfc_device_new(ptr::null(), connstring.as_ptr()) };
        assert!(!device.is_null());

        unsafe {
            assert!((*device).context.is_null());
            assert!((*device).driver.is_null());
            assert!((*device).driver_data.is_null());
            assert!((*device).chip_data.is_null());
            assert_eq!((*device).name[0], 0);
            assert_eq!(
                CStr::from_ptr((*device).connstring.as_ptr()).to_bytes(),
                connstring.as_bytes()
            );
            assert!(!(*device).bCrc);
            assert!(!(*device).bPar);
            assert!(!(*device).bEasyFraming);
            assert!(!(*device).bInfiniteSelect);
            assert!(!(*device).bAutoIso14443_4);
            assert_eq!((*device).btSupportByte, 0);
            assert_eq!((*device).last_error, 0);

            (*device).driver_data = libc::malloc(8);
            nfc_device_free(device);
        }
    }

    #[test]
    fn device_new_rejects_null_connstring() {
        reset_last_error();
        let device = unsafe { nfc_device_new(ptr::null(), ptr::null()) };
        assert!(device.is_null());

        let err = crate::nfc_get_last_error();
        assert!(!err.is_null());
        let recovered = unsafe { CStr::from_ptr(err) }.to_str().unwrap();
        assert!(recovered.contains("NULL connstring in nfc_device_new"));
    }

    #[test]
    fn lifecycle_pointer_panic_is_normalized_to_null() {
        reset_last_error();
        let ptr = ffi_catch_unwind_ptr::<nfc_context, _>("lifecycle_ptr_panic", || panic!("boom"));
        assert!(ptr.is_null());

        let err = crate::nfc_get_last_error();
        assert!(!err.is_null());
        let recovered = unsafe { CStr::from_ptr(err) }.to_str().unwrap();
        assert!(recovered.contains("panic in lifecycle_ptr_panic"));
    }

    #[test]
    fn lifecycle_void_panic_is_normalized_to_noop() {
        reset_last_error();
        ffi_catch_unwind_void("lifecycle_void_panic", || panic!("boom"));

        let err = crate::nfc_get_last_error();
        assert!(!err.is_null());
        let recovered = unsafe { CStr::from_ptr(err) }.to_str().unwrap();
        assert!(recovered.contains("panic in lifecycle_void_panic"));
    }
}
