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

use crate::ffi_support::{
    as_mut, bounded_strlen, copy_bytes_to_c_buffer, copy_c_string_to_c_buffer,
    fixed_c_buffer_to_string,
};
use crate::ffi_types::{
    nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_mode, nfc_modulation, nfc_modulation_type,
    nfc_property, nfc_target,
};
use crate::{
    LOG_PRIORITY_DEBUG, LOG_PRIORITY_NONE, NFC_BUFSIZE_CONNSTRING, ffi_catch_unwind_ptr,
    ffi_catch_unwind_void, log_error, log_message, release_allocated_ptr, reset_last_error,
    set_last_error_message,
};
use libc::{c_char, c_int, c_uint, c_void};
use std::mem::size_of;
use std::ptr;

pub const DEVICE_NAME_LENGTH: usize = 256;
pub const MAX_USER_DEFINED_DEVICES: usize = 4;
pub const NFC_DRIVER_NAME_MAX: usize = 64;
const DEFAULT_CONTEXT_LOG_LEVEL: u32 = if cfg!(libnfc_debug) { 3 } else { 1 };
const USER_DEFINED_DEFAULT_DEVICE_NAME: &[u8] = b"user defined default device";
const USER_DEFINED_DEVICE_NAME: &[u8] = b"user defined device";
const ENV_LIBNFC_DEFAULT_DEVICE: &[u8] = b"LIBNFC_DEFAULT_DEVICE\0";
const ENV_LIBNFC_DEVICE: &[u8] = b"LIBNFC_DEVICE\0";
const ENV_LIBNFC_AUTO_SCAN: &[u8] = b"LIBNFC_AUTO_SCAN\0";
const ENV_LIBNFC_INTRUSIVE_SCAN: &[u8] = b"LIBNFC_INTRUSIVE_SCAN\0";
const ENV_LIBNFC_LOG_LEVEL: &[u8] = b"LIBNFC_LOG_LEVEL\0";

#[allow(non_camel_case_types)]
pub type nfc_connstring = [c_char; NFC_BUFSIZE_CONNSTRING];

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub enum scan_type_enum {
    NOT_INTRUSIVE = 0,
    INTRUSIVE = 1,
    NOT_AVAILABLE = 2,
}

#[allow(non_camel_case_types)]
type nfc_driver_scan_fn =
    unsafe extern "C" fn(*const nfc_context, *mut nfc_connstring, usize) -> usize;
#[allow(non_camel_case_types)]
type nfc_driver_open_fn =
    unsafe extern "C" fn(*const nfc_context, *const c_char) -> *mut nfc_device;
#[allow(non_camel_case_types)]
type nfc_driver_close_fn = unsafe extern "C" fn(*mut nfc_device);
#[allow(non_camel_case_types)]
type nfc_driver_strerror_fn = unsafe extern "C" fn(*const nfc_device) -> *const c_char;
#[allow(non_camel_case_types)]
type nfc_driver_initiator_init_fn = unsafe extern "C" fn(*mut nfc_device) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_initiator_init_secure_element_fn = unsafe extern "C" fn(*mut nfc_device) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_initiator_select_passive_target_fn = unsafe extern "C" fn(
    *mut nfc_device,
    nfc_modulation,
    *const u8,
    usize,
    *mut nfc_target,
) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_initiator_poll_target_fn = unsafe extern "C" fn(
    *mut nfc_device,
    *const nfc_modulation,
    usize,
    u8,
    u8,
    *mut nfc_target,
) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_initiator_select_dep_target_fn = unsafe extern "C" fn(
    *mut nfc_device,
    nfc_dep_mode,
    nfc_baud_rate,
    *const nfc_dep_info,
    *mut nfc_target,
    c_int,
) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_initiator_deselect_target_fn = unsafe extern "C" fn(*mut nfc_device) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_initiator_transceive_bytes_fn =
    unsafe extern "C" fn(*mut nfc_device, *const u8, usize, *mut u8, usize, c_int) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_initiator_transceive_bits_fn =
    unsafe extern "C" fn(*mut nfc_device, *const u8, usize, *const u8, *mut u8, *mut u8) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_initiator_transceive_bytes_timed_fn =
    unsafe extern "C" fn(*mut nfc_device, *const u8, usize, *mut u8, usize, *mut u32) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_initiator_transceive_bits_timed_fn = unsafe extern "C" fn(
    *mut nfc_device,
    *const u8,
    usize,
    *const u8,
    *mut u8,
    *mut u8,
    *mut u32,
) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_initiator_target_is_present_fn =
    unsafe extern "C" fn(*mut nfc_device, *const nfc_target) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_target_init_fn =
    unsafe extern "C" fn(*mut nfc_device, *mut nfc_target, *mut u8, usize, c_int) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_target_send_bytes_fn =
    unsafe extern "C" fn(*mut nfc_device, *const u8, usize, c_int) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_target_receive_bytes_fn =
    unsafe extern "C" fn(*mut nfc_device, *mut u8, usize, c_int) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_target_send_bits_fn =
    unsafe extern "C" fn(*mut nfc_device, *const u8, usize, *const u8) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_target_receive_bits_fn =
    unsafe extern "C" fn(*mut nfc_device, *mut u8, usize, *mut u8) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_device_set_property_bool_fn =
    unsafe extern "C" fn(*mut nfc_device, nfc_property, bool) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_device_set_property_int_fn =
    unsafe extern "C" fn(*mut nfc_device, nfc_property, c_int) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_get_supported_modulation_fn =
    unsafe extern "C" fn(*mut nfc_device, nfc_mode, *mut *const nfc_modulation_type) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_get_supported_baud_rate_fn = unsafe extern "C" fn(
    *mut nfc_device,
    nfc_mode,
    nfc_modulation_type,
    *mut *const nfc_baud_rate,
) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_device_get_information_about_fn =
    unsafe extern "C" fn(*mut nfc_device, *mut *mut c_char) -> c_int;
#[allow(non_camel_case_types)]
type nfc_driver_device_control_fn = unsafe extern "C" fn(*mut nfc_device) -> c_int;

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct nfc_driver {
    pub name: *const c_char,
    pub scan_type: scan_type_enum,
    pub scan: Option<nfc_driver_scan_fn>,
    pub open: Option<nfc_driver_open_fn>,
    pub close: Option<nfc_driver_close_fn>,
    pub strerror: Option<nfc_driver_strerror_fn>,
    pub initiator_init: Option<nfc_driver_initiator_init_fn>,
    pub initiator_init_secure_element: Option<nfc_driver_initiator_init_secure_element_fn>,
    pub initiator_select_passive_target: Option<nfc_driver_initiator_select_passive_target_fn>,
    pub initiator_poll_target: Option<nfc_driver_initiator_poll_target_fn>,
    pub initiator_select_dep_target: Option<nfc_driver_initiator_select_dep_target_fn>,
    pub initiator_deselect_target: Option<nfc_driver_initiator_deselect_target_fn>,
    pub initiator_transceive_bytes: Option<nfc_driver_initiator_transceive_bytes_fn>,
    pub initiator_transceive_bits: Option<nfc_driver_initiator_transceive_bits_fn>,
    pub initiator_transceive_bytes_timed: Option<nfc_driver_initiator_transceive_bytes_timed_fn>,
    pub initiator_transceive_bits_timed: Option<nfc_driver_initiator_transceive_bits_timed_fn>,
    pub initiator_target_is_present: Option<nfc_driver_initiator_target_is_present_fn>,
    pub target_init: Option<nfc_driver_target_init_fn>,
    pub target_send_bytes: Option<nfc_driver_target_send_bytes_fn>,
    pub target_receive_bytes: Option<nfc_driver_target_receive_bytes_fn>,
    pub target_send_bits: Option<nfc_driver_target_send_bits_fn>,
    pub target_receive_bits: Option<nfc_driver_target_receive_bits_fn>,
    pub device_set_property_bool: Option<nfc_driver_device_set_property_bool_fn>,
    pub device_set_property_int: Option<nfc_driver_device_set_property_int_fn>,
    pub get_supported_modulation: Option<nfc_driver_get_supported_modulation_fn>,
    pub get_supported_baud_rate: Option<nfc_driver_get_supported_baud_rate_fn>,
    pub device_get_information_about: Option<nfc_driver_device_get_information_about_fn>,
    pub abort_command: Option<nfc_driver_device_control_fn>,
    pub idle: Option<nfc_driver_device_control_fn>,
    pub powerdown: Option<nfc_driver_device_control_fn>,
}

unsafe impl Sync for nfc_driver {}

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
    fn nfc_rs_context_log_init(context: *const nfc_context);
    fn nfc_rs_context_log_exit();
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

#[cfg(all(not(test), libnfc_external_bridges))]
unsafe fn bridge_context_log_exit() {
    unsafe { nfc_rs_context_log_exit() };
}

#[cfg(any(test, not(libnfc_external_bridges)))]
unsafe fn bridge_context_log_exit() {
    #[cfg(test)]
    unsafe {
        nfc_rs_context_log_exit();
        return;
    }

    #[cfg(not(test))]
    {}
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

    let Some(device_ref) = (unsafe { as_mut(device) }) else {
        return ptr::null_mut();
    };
    device_ref.context = context;
    let copy_len = bounded_strlen(connstring, NFC_BUFSIZE_CONNSTRING.saturating_sub(1));
    if copy_len > 0 {
        unsafe {
            ptr::copy_nonoverlapping(connstring, device_ref.connstring.as_mut_ptr(), copy_len);
        }
    }
    device_ref.connstring[copy_len] = 0;

    reset_last_error();
    device
}

fn set_copy_error(message: &str) {
    log_error(message);
    set_last_error_message(message.to_string());
}

unsafe fn apply_user_defined_device(
    context: *mut nfc_context,
    device_name: &[u8],
    connstring: *const c_char,
    count: c_uint,
    connstring_error_message: &str,
) -> bool {
    let Some(context) = (unsafe { as_mut(context) }) else {
        return false;
    };
    let device = &mut context.user_defined_devices[0];

    if !unsafe { copy_bytes_to_c_buffer(device.name.as_mut_ptr(), DEVICE_NAME_LENGTH, device_name) }
    {
        set_copy_error("Failed to copy device name");
        return false;
    }

    if !unsafe {
        copy_c_string_to_c_buffer(
            device.connstring.as_mut_ptr(),
            NFC_BUFSIZE_CONNSTRING,
            connstring,
        )
    } {
        set_copy_error(connstring_error_message);
        return false;
    }

    context.user_defined_device_count = count;
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

fn log_context_state(context: &nfc_context) {
    let first_priority = if cfg!(libnfc_debug) {
        LOG_PRIORITY_NONE
    } else {
        LOG_PRIORITY_DEBUG
    };

    log_message(
        first_priority,
        &format!("log_level is set to {}", context.log_level),
    );
    log_message(
        LOG_PRIORITY_DEBUG,
        &format!(
            "allow_autoscan is set to {}",
            if context.allow_autoscan {
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
            if context.allow_intrusive_scan {
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
            context.user_defined_device_count
        ),
    );

    for (index, device) in context.user_defined_devices
        [..context.user_defined_device_count as usize]
        .iter()
        .enumerate()
    {
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
        unsafe { crate::conf::load_context_config(context) };
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

        if let Some(context_ref) = unsafe { as_mut(context) } {
            apply_boolean_string(
                unsafe { getenv(ENV_LIBNFC_AUTO_SCAN) } as *const c_char,
                &mut context_ref.allow_autoscan,
            );
            apply_boolean_string(
                unsafe { getenv(ENV_LIBNFC_INTRUSIVE_SCAN) } as *const c_char,
                &mut context_ref.allow_intrusive_scan,
            );

            let env_log_level = unsafe { getenv(ENV_LIBNFC_LOG_LEVEL) };
            if !env_log_level.is_null() {
                context_ref.log_level = unsafe { libc::atoi(env_log_level) as u32 };
            }
        }
    }

    unsafe {
        bridge_context_log_init(context);
        if let Some(context_ref) = as_mut(context) {
            log_context_state(context_ref);
        }
    }

    reset_last_error();
    context
}

pub unsafe fn nfc_context_alloc_defaults() -> *mut nfc_context {
    ffi_catch_unwind_ptr("nfc_context_alloc_defaults", || unsafe {
        nfc_context_alloc_defaults_impl()
    })
}

pub unsafe fn nfc_context_new() -> *mut nfc_context {
    ffi_catch_unwind_ptr("nfc_context_new", || unsafe { nfc_context_new_impl() })
}

pub unsafe fn nfc_device_new(
    context: *const nfc_context,
    connstring: *const c_char,
) -> *mut nfc_device {
    ffi_catch_unwind_ptr("nfc_device_new", || unsafe {
        nfc_device_new_impl(context, connstring)
    })
}

pub unsafe fn nfc_device_free(device: *mut nfc_device) {
    ffi_catch_unwind_void("nfc_device_free", || unsafe {
        if device.is_null() {
            return;
        }

        release_allocated_ptr((*device).driver_data);
        release_allocated_ptr(device as *mut c_void);
    });
}

pub unsafe fn nfc_context_free(context: *mut nfc_context) {
    ffi_catch_unwind_void("nfc_context_free", || unsafe {
        increment_context_free_count_for_tests();
        bridge_context_log_exit();
        free_context_allocation(context);
    });
}

#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct LifecycleBridgeTestState {
    pub(crate) log_init_calls: usize,
    pub(crate) log_exit_calls: usize,
    pub(crate) context_free_calls: usize,
    pub(crate) events: Vec<&'static str>,
}

#[cfg(test)]
thread_local! {
    static TEST_LIFECYCLE_STATE: std::cell::RefCell<LifecycleBridgeTestState> =
        std::cell::RefCell::new(LifecycleBridgeTestState::default());
}

#[cfg(test)]
pub(crate) fn reset_lifecycle_test_state() {
    TEST_LIFECYCLE_STATE.with(|cell| {
        *cell.borrow_mut() = LifecycleBridgeTestState::default();
    });
}

#[cfg(test)]
pub(crate) fn snapshot_lifecycle_test_state() -> LifecycleBridgeTestState {
    TEST_LIFECYCLE_STATE.with(|cell| cell.borrow().clone())
}

#[cfg(test)]
fn increment_context_free_count_for_tests() {
    TEST_LIFECYCLE_STATE.with(|cell| {
        cell.borrow_mut().context_free_calls += 1;
    });
}

#[cfg(not(test))]
fn increment_context_free_count_for_tests() {}

#[cfg(test)]
pub unsafe fn nfc_rs_context_log_init(_context: *const nfc_context) {
    TEST_LIFECYCLE_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        state.log_init_calls += 1;
        state.events.push("log_init");
    });
}

#[cfg(test)]
pub unsafe fn nfc_rs_context_log_exit() {
    TEST_LIFECYCLE_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        state.log_exit_calls += 1;
        state.events.push("log_exit");
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conf::set_test_conf_root;
    use std::ffi::{CStr, CString, OsString};
    use std::fs;
    use std::path::PathBuf;
    use std::process;
    use std::sync::atomic::{AtomicUsize, Ordering};
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

    struct TempConfigDir {
        root: PathBuf,
    }

    impl TempConfigDir {
        fn new() -> Self {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "proximate-conf-{}-{}",
                process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn install(&self) {
            set_test_conf_root(Some(self.root.clone()));
        }

        fn write_file(&self, relative: &str, contents: &str) {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, contents).unwrap();
        }
    }

    impl Drop for TempConfigDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
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

    fn reset_test_world() {
        reset_lifecycle_test_state();
        set_test_conf_root(None);
        crate::test_clear_last_log();
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
    fn context_new_applies_defaults_and_initializes_logging() {
        let _env_guard = env_lock().lock().unwrap();
        let mut env = ScopedEnv::new();
        clear_env(&mut env);
        reset_test_world();

        let context = unsafe { nfc_context_new() };
        assert!(!context.is_null());

        unsafe {
            assert!((*context).allow_autoscan);
            assert!(!(*context).allow_intrusive_scan);
            assert_eq!((*context).log_level, DEFAULT_CONTEXT_LOG_LEVEL);
            assert_eq!((*context).user_defined_device_count, 0);
        }

        let bridge_state = snapshot_lifecycle_test_state();
        assert_eq!(bridge_state.log_init_calls, 1);
        assert_eq!(bridge_state.log_exit_calls, 0);
        assert_eq!(bridge_state.events, vec!["log_init"]);
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
        reset_test_world();

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
    fn context_new_loads_config_files_and_devices_d_entries() {
        let _env_guard = env_lock().lock().unwrap();
        let mut env = ScopedEnv::new();
        clear_env(&mut env);
        reset_test_world();

        let confdir = TempConfigDir::new();
        confdir.write_file(
            "libnfc.conf",
            concat!(
                "allow_autoscan = false\n",
                "allow_intrusive_scan = true\n",
                "log_level = 7\n",
                "device.name = \"config device\"\n",
                "device.connstring = pn532_spi:/dev/spidev0.0\n",
                "device.optional = True\n"
            ),
        );
        confdir.write_file(
            "devices.d/extra.conf",
            concat!(
                "name = \"extra device\"\n",
                "connstring = pn532_i2c:/dev/i2c-1\n",
                "optional = 1\n"
            ),
        );
        confdir.install();

        let context = unsafe { nfc_context_new() };
        assert!(!context.is_null());

        unsafe {
            assert!(!(*context).allow_autoscan);
            assert!((*context).allow_intrusive_scan);
            assert_eq!((*context).log_level, 7);
            assert_eq!((*context).user_defined_device_count, 2);

            let first = &(*context).user_defined_devices[0];
            assert_eq!(
                CStr::from_ptr(first.name.as_ptr()).to_bytes(),
                b"config device"
            );
            assert_eq!(
                CStr::from_ptr(first.connstring.as_ptr()).to_bytes(),
                b"pn532_spi:/dev/spidev0.0"
            );
            assert!(first.optional);

            let second = &(*context).user_defined_devices[1];
            assert_eq!(
                CStr::from_ptr(second.name.as_ptr()).to_bytes(),
                b"extra device"
            );
            assert_eq!(
                CStr::from_ptr(second.connstring.as_ptr()).to_bytes(),
                b"pn532_i2c:/dev/i2c-1"
            );
            assert!(second.optional);
        }

        let logs = crate::test_get_logs();
        assert!(
            logs.iter()
                .any(|entry| entry.contains("key: [allow_autoscan], value: [false]"))
        );

        release_context(context);
    }

    #[test]
    fn context_new_logs_parse_errors_and_caps_device_count() {
        let _env_guard = env_lock().lock().unwrap();
        let mut env = ScopedEnv::new();
        clear_env(&mut env);
        reset_test_world();

        let confdir = TempConfigDir::new();
        confdir.write_file(
            "libnfc.conf",
            concat!(
                "unknown.key = value\n",
                "broken line\n",
                "device.name = first\n",
                "device.connstring = pn532_uart:/dev/ttyUSB0\n",
                "device.name = second\n",
                "device.connstring = pn53x_usb:001:002\n",
                "device.name = third\n",
                "device.connstring = pn532_spi:/dev/spidev0.0\n",
                "device.name = fourth\n",
                "device.connstring = pn532_i2c:/dev/i2c-1\n",
                "device.name = fifth\n",
                "device.connstring = pn71xx:/dev/nfc0\n"
            ),
        );
        confdir.install();

        let context = unsafe { nfc_context_new() };
        assert!(!context.is_null());

        unsafe {
            assert_eq!(
                (*context).user_defined_device_count as usize,
                MAX_USER_DEFINED_DEVICES
            );
        }

        let logs = crate::test_get_logs();
        assert!(
            logs.iter()
                .any(|entry| entry.contains("Unknown key in config line: unknown.key = value"))
        );
        assert!(
            logs.iter()
                .any(|entry| entry.contains("Parse error on line #2: broken line"))
        );
        assert!(logs
            .iter()
            .any(|entry| entry.contains("Configuration exceeded maximum user-defined devices.")));

        release_context(context);
    }

    #[test]
    fn context_new_libnfc_device_overrides_config_and_default_device() {
        let _env_guard = env_lock().lock().unwrap();
        let mut env = ScopedEnv::new();
        clear_env(&mut env);
        env.set("LIBNFC_DEFAULT_DEVICE", "pn532_uart:/dev/ttyUSB0");
        env.set("LIBNFC_DEVICE", "pn53x_usb:001:002");
        reset_test_world();

        let confdir = TempConfigDir::new();
        confdir.write_file(
            "libnfc.conf",
            concat!(
                "device.name = \"config device\"\n",
                "device.connstring = pn532_spi:/dev/spidev0.0\n",
                "device.optional = true\n"
            ),
        );
        confdir.install();

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
        reset_test_world();

        let confdir = TempConfigDir::new();
        confdir.write_file(
            "libnfc.conf",
            concat!(
                "allow_autoscan = true\n",
                "allow_intrusive_scan = false\n",
                "log_level = 7\n"
            ),
        );
        confdir.install();

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
        reset_test_world();

        let context = unsafe { nfc_context_new() };
        assert!(!context.is_null());

        unsafe {
            assert!(!(*context).allow_intrusive_scan);
        }

        release_context(context);
    }

    #[test]
    fn context_free_calls_log_exit_and_accepts_null() {
        let _env_guard = env_lock().lock().unwrap();
        let mut env = ScopedEnv::new();
        clear_env(&mut env);
        reset_test_world();

        let confdir = TempConfigDir::new();
        confdir.write_file("libnfc.conf", "");
        confdir.install();

        let context = unsafe { nfc_context_new() };
        assert!(!context.is_null());

        unsafe {
            nfc_context_free(context);
            nfc_context_free(ptr::null_mut());
        }

        let state = snapshot_lifecycle_test_state();
        assert_eq!(state.context_free_calls, 2);
        assert_eq!(state.log_exit_calls, 2);
        assert_eq!(state.events, vec!["log_init", "log_exit", "log_exit"]);
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
