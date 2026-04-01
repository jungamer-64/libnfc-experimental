// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Ported from libnfc/nfc.c.

use crate::c_api_impl::{
    LOG_GROUP_GENERAL, LOG_PRIORITY_DEBUG, LOG_PRIORITY_ERROR, NFC_BUFSIZE_CONNSTRING,
};
use crate::ffi_support::{as_ref, bounded_strlen, c_string_ptr_to_string, copy_bytes_to_c_buffer};
#[cfg(test)]
use crate::ffi_support::{copy_c_string_to_c_buffer, fixed_c_buffer_to_string};
#[cfg(test)]
use crate::lifecycle::{DEVICE_NAME_LENGTH, NFC_DRIVER_NAME_MAX, scan_type_enum};
use crate::lifecycle::{nfc_connstring, nfc_context, nfc_context_new, nfc_device, nfc_driver};
use crate::runtime_bridge::{attach_rust_device, context_from_c, register_external_drivers};
use crate::{
    MALLOC_LABEL, emit_log_message, ffi_catch_unwind_int, ffi_catch_unwind_ptr,
    ffi_catch_unwind_void,
};
use libc::{c_char, c_int, size_t};
use proximate::rust_api as rt;
use std::ffi::CString;
use std::ptr;
use std::sync::{Mutex, OnceLock};

const NFC_SUCCESS: c_int = 0;
const NFC_EINVARG: c_int = -2;
const NFC_ESOFT: c_int = -80;

const LOG_PRIORITY_INFO: u8 = 2;
const GENERAL_LOG_CATEGORY: *const c_char = b"libnfc.general\0" as *const u8 as *const c_char;
#[cfg(test)]
const ENV_LIBNFC_LOG_LEVEL: &[u8] = b"LIBNFC_LOG_LEVEL\0";
#[cfg(test)]
const ENV_LIBNFC_LOG_LEVEL_NAME: &str = "LIBNFC_LOG_LEVEL";

#[derive(Clone, Copy)]
pub(crate) struct DriverHandle(pub(crate) *const nfc_driver);

unsafe impl Send for DriverHandle {}

static DRIVER_REGISTRY: OnceLock<Mutex<rt::RegisteredDriverSet<DriverHandle>>> = OnceLock::new();

fn driver_registry() -> &'static Mutex<rt::RegisteredDriverSet<DriverHandle>> {
    DRIVER_REGISTRY.get_or_init(|| Mutex::new(rt::RegisteredDriverSet::new()))
}

fn with_registry<R>(f: impl FnOnce(&mut rt::RegisteredDriverSet<DriverHandle>) -> R) -> R {
    let mut registry = driver_registry()
        .lock()
        .expect("driver registry mutex should not be poisoned");
    f(&mut registry)
}

pub(crate) fn registry_snapshot() -> Vec<DriverHandle> {
    with_registry(|registry| registry.snapshot())
}

pub(crate) fn clear_registry() {
    with_registry(|registry| registry.clear());
}

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

fn log_general_error(message: &str) {
    log_general_message(LOG_PRIORITY_ERROR, message);
}

fn log_general_info(message: &str) {
    log_general_message(LOG_PRIORITY_INFO, message);
}

#[cfg(test)]
fn string_contains_control_chars(value: *const c_char, length: usize) -> bool {
    if value.is_null() {
        return false;
    }

    for index in 0..length {
        let byte = unsafe { *value.add(index) as u8 };
        if unsafe { libc::isprint(byte as c_int) } == 0 {
            return true;
        }
    }

    false
}

#[cfg(test)]
unsafe fn write_bytes_to_char_buffer(dst: *mut c_char, dst_size: usize, src: &[u8]) -> bool {
    unsafe { copy_bytes_to_c_buffer(dst, dst_size, src) }
}

#[cfg(test)]
unsafe fn copy_connstring_safely(source: *const c_char, destination: *mut nfc_connstring) -> bool {
    if source.is_null() || destination.is_null() {
        return false;
    }

    let length = bounded_strlen(source, NFC_BUFSIZE_CONNSTRING);

    if string_contains_control_chars(source, length) {
        log_general_error("Connection string contains control characters");
        return false;
    }

    if length >= NFC_BUFSIZE_CONNSTRING {
        log_general_error("Connection string exceeds maximum length");
        return false;
    }

    let Some(destination): Option<&nfc_connstring> = (unsafe { as_ref(destination.cast_const()) })
    else {
        return false;
    };
    unsafe {
        copy_c_string_to_c_buffer(
            destination.as_ptr().cast_mut(),
            NFC_BUFSIZE_CONNSTRING,
            source,
        )
    }
}

unsafe fn push_driver(driver: *const nfc_driver) -> c_int {
    if driver.is_null() {
        log_general_debug("nfc_register_driver: NULL driver");
        return NFC_EINVARG;
    }

    with_registry(|registry| {
        if registry.register(DriverHandle(driver)).is_err() {
            return NFC_ESOFT;
        }
        NFC_SUCCESS
    })
}

#[cfg(all(not(test), libnfc_external_bridges))]
unsafe extern "C" {
    fn nfc_close(device: *mut nfc_device);
}

#[cfg(any(test, not(libnfc_external_bridges)))]
unsafe fn invoke_driver_close(device: *mut nfc_device) {
    let Some(device_ref) = (unsafe { as_ref(device) }) else {
        return;
    };

    let Some(driver_ref) = (unsafe { as_ref(device_ref.driver) }) else {
        return;
    };

    if let Some(close) = driver_ref.close {
        unsafe { close(device) };
    }
}

#[cfg(all(not(test), libnfc_external_bridges))]
pub(crate) unsafe fn bridge_close_device(device: *mut nfc_device) {
    unsafe { nfc_close(device) };
}

#[cfg(any(test, not(libnfc_external_bridges)))]
pub(crate) unsafe fn bridge_close_device(device: *mut nfc_device) {
    #[cfg(test)]
    unsafe {
        nfc_close(device);
        return;
    }

    #[cfg(not(test))]
    unsafe {
        invoke_driver_close(device);
    }
}

#[cfg(all(not(test), libnfc_external_bridges, libnfc_driver_pcsc))]
unsafe extern "C" {
    static pcsc_driver: nfc_driver;
}
#[cfg(all(not(test), libnfc_external_bridges, libnfc_driver_acr122_pcsc))]
unsafe extern "C" {
    static acr122_pcsc_driver: nfc_driver;
}
#[cfg(all(not(test), libnfc_external_bridges, libnfc_driver_acr122_usb))]
unsafe extern "C" {
    static acr122_usb_driver: nfc_driver;
}
#[cfg(all(not(test), libnfc_external_bridges, libnfc_driver_acr122s))]
unsafe extern "C" {
    static acr122s_driver: nfc_driver;
}
#[cfg(all(not(test), libnfc_external_bridges, libnfc_driver_arygon))]
unsafe extern "C" {
    static arygon_driver: nfc_driver;
}
fn register_compiled_bridge_drivers(_registry: &mut rt::DriverRegistry) {
    #[cfg(all(not(test), libnfc_external_bridges, libnfc_driver_pcsc))]
    _registry.register_driver(rt::wrap_driver_backend(Box::new(
        crate::runtime_bridge::DriverAdapter::new(ptr::addr_of!(pcsc_driver)),
    )));
    #[cfg(all(not(test), libnfc_external_bridges, libnfc_driver_acr122_pcsc))]
    _registry.register_driver(rt::wrap_driver_backend(Box::new(
        crate::runtime_bridge::DriverAdapter::new(ptr::addr_of!(acr122_pcsc_driver)),
    )));
    #[cfg(all(not(test), libnfc_external_bridges, libnfc_driver_acr122_usb))]
    _registry.register_driver(rt::wrap_driver_backend(Box::new(
        crate::runtime_bridge::DriverAdapter::new(ptr::addr_of!(acr122_usb_driver)),
    )));
    #[cfg(all(not(test), libnfc_external_bridges, libnfc_driver_acr122s))]
    _registry.register_driver(rt::wrap_driver_backend(Box::new(
        crate::runtime_bridge::DriverAdapter::new(ptr::addr_of!(acr122s_driver)),
    )));
    #[cfg(all(not(test), libnfc_external_bridges, libnfc_driver_arygon))]
    _registry.register_driver(rt::wrap_driver_backend(Box::new(
        crate::runtime_bridge::DriverAdapter::new(ptr::addr_of!(arygon_driver)),
    )));
}

fn create_runtime_registry() -> rt::DriverRegistry {
    let mut registry = rt::DriverRegistry::new();
    registry.register_builtin_drivers();
    register_compiled_bridge_drivers(&mut registry);
    register_external_drivers(&mut registry, &registry_snapshot());
    registry
}

unsafe fn nfc_open_impl(context: *mut nfc_context, connstring: *const c_char) -> *mut nfc_device {
    let runtime_context = context_from_c(context.cast_const());
    let registry = create_runtime_registry();
    let requested = if connstring.is_null() {
        None
    } else {
        let value = c_string_ptr_to_string(
            connstring,
            bounded_strlen(connstring, NFC_BUFSIZE_CONNSTRING),
        );
        match proximate::ConnectionString::new(value) {
            Ok(connstring) => Some(connstring),
            Err(_) => return ptr::null_mut(),
        }
    };

    match registry.open(&runtime_context, requested.as_ref()) {
        Ok(device) => attach_rust_device(device, context.cast_const()).unwrap_or(ptr::null_mut()),
        Err(error) => {
            log_general_debug(&format!("nfc_open failed: {:?}", error));
            ptr::null_mut()
        }
    }
}

unsafe fn nfc_list_devices_impl(
    context: *mut nfc_context,
    connstrings: *mut nfc_connstring,
    connstrings_len: usize,
) -> usize {
    if connstrings.is_null() || connstrings_len == 0 {
        return 0;
    }
    let runtime_context = context_from_c(context.cast_const());
    let registry = create_runtime_registry();
    let Ok(outcome) = registry.list_devices_outcome(&runtime_context) else {
        return 0;
    };
    if outcome.warn_manual_selection {
        log_general_info("Warning: user must specify device(s) manually when autoscan is disabled");
    }

    let mut written = 0usize;
    for connstring in outcome.devices.into_iter().take(connstrings_len) {
        let value = connstring.as_str();
        let destination = unsafe { connstrings.add(written) };
        if unsafe {
            copy_bytes_to_c_buffer(destination.cast(), NFC_BUFSIZE_CONNSTRING, value.as_bytes())
        } {
            written += 1;
        }
    }
    written
}

unsafe fn nfc_init_impl(context: *mut *mut nfc_context) {
    if context.is_null() {
        log_general_error("nfc_init: NULL context pointer");
        return;
    }

    unsafe {
        *context = nfc_context_new();
        if (*context).is_null() {
            libc::perror(MALLOC_LABEL);
            return;
        }
    }
}

pub unsafe fn nfc_register_driver(driver: *const nfc_driver) -> c_int {
    ffi_catch_unwind_int("nfc_register_driver", NFC_ESOFT, || unsafe {
        push_driver(driver)
    })
}

pub unsafe fn nfc_open(context: *mut nfc_context, connstring: *const c_char) -> *mut nfc_device {
    ffi_catch_unwind_ptr("nfc_open", || unsafe { nfc_open_impl(context, connstring) })
}

fn ffi_catch_unwind_size_t<F>(context: &str, operation: F) -> size_t
where
    F: FnOnce() -> size_t,
    F: std::panic::UnwindSafe,
{
    #[cfg(not(feature = "test_no_catch"))]
    {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)) {
            Ok(result) => result,
            Err(_) => {
                log_general_error(&format!("panic in {}", context));
                0
            }
        }
    }

    #[cfg(feature = "test_no_catch")]
    {
        let _ = context;
        operation()
    }
}

pub unsafe fn nfc_list_devices(
    context: *mut nfc_context,
    connstrings: *mut nfc_connstring,
    connstrings_len: size_t,
) -> size_t {
    ffi_catch_unwind_size_t("nfc_list_devices", || unsafe {
        nfc_list_devices_impl(context, connstrings, connstrings_len)
    })
}

pub unsafe fn nfc_init(context: *mut *mut nfc_context) {
    ffi_catch_unwind_void("nfc_init", || unsafe { nfc_init_impl(context) });
}

pub unsafe fn nfc_exit(context: *mut nfc_context) {
    ffi_catch_unwind_void("nfc_exit", || unsafe {
        clear_registry();
        crate::lifecycle::nfc_context_free(context);
    });
}

#[cfg(test)]
#[derive(Clone, Default)]
struct CoreBridgeTestState {
    close_calls: usize,
}

#[cfg(test)]
thread_local! {
    static CORE_BRIDGE_TEST_STATE: std::cell::RefCell<CoreBridgeTestState> =
        std::cell::RefCell::new(CoreBridgeTestState::default());
}

#[cfg(test)]
fn reset_core_bridge_test_state() {
    CORE_BRIDGE_TEST_STATE.with(|cell| {
        *cell.borrow_mut() = CoreBridgeTestState::default();
    });
}

#[cfg(test)]
fn snapshot_core_bridge_test_state() -> CoreBridgeTestState {
    CORE_BRIDGE_TEST_STATE.with(|cell| cell.borrow().clone())
}

#[cfg(test)]
pub unsafe fn nfc_close(device: *mut nfc_device) {
    CORE_BRIDGE_TEST_STATE.with(|cell| {
        cell.borrow_mut().close_calls += 1;
    });

    unsafe { invoke_driver_close(device) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::{
        nfc_context_alloc_defaults, nfc_device_free, reset_lifecycle_test_state,
        snapshot_lifecycle_test_state,
    };
    use crate::{test_clear_last_log, test_get_last_log};
    use std::ffi::CString;
    use std::sync::{Mutex, MutexGuard};

    #[derive(Clone, Default)]
    struct FakeDriverState {
        open_calls: Vec<String>,
        scan_calls: Vec<String>,
        close_calls: Vec<String>,
        failing_connstrings: Vec<String>,
        scan_results: Vec<(String, Vec<String>)>,
    }

    thread_local! {
        static FAKE_DRIVER_STATE: std::cell::RefCell<FakeDriverState> =
            std::cell::RefCell::new(FakeDriverState::default());
    }

    static CORE_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn core_test_guard() -> MutexGuard<'static, ()> {
        CORE_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("core test mutex should not be poisoned")
    }

    fn reset_fake_driver_state() {
        FAKE_DRIVER_STATE.with(|cell| {
            *cell.borrow_mut() = FakeDriverState::default();
        });
    }

    fn with_fake_driver_state<R>(f: impl FnOnce(&mut FakeDriverState) -> R) -> R {
        FAKE_DRIVER_STATE.with(|cell| f(&mut cell.borrow_mut()))
    }

    fn set_scan_results(driver: &str, results: &[&str]) {
        with_fake_driver_state(|state| {
            state
                .scan_results
                .retain(|(existing, _)| existing != driver);
            state.scan_results.push((
                driver.to_string(),
                results.iter().map(|value| (*value).to_string()).collect(),
            ));
        });
    }

    fn add_failing_connstring(connstring: &str) {
        with_fake_driver_state(|state| {
            state.failing_connstrings.push(connstring.to_string());
        });
    }

    fn fake_driver_snapshot() -> FakeDriverState {
        FAKE_DRIVER_STATE.with(|cell| cell.borrow().clone())
    }

    unsafe fn write_string_to_buffer(dst: *mut c_char, dst_size: usize, value: &str) {
        let bytes = value.as_bytes();
        assert!(unsafe { write_bytes_to_char_buffer(dst, dst_size, bytes) });
    }

    unsafe fn allocate_fake_device(
        driver: *const nfc_driver,
        driver_name: &str,
        context: *const nfc_context,
        connstring: *const c_char,
    ) -> *mut nfc_device {
        let device = unsafe { crate::lifecycle::nfc_device_new(context, connstring) };
        if device.is_null() {
            return ptr::null_mut();
        }

        unsafe {
            (*device).driver = driver;
            write_string_to_buffer(
                (*device).name.as_mut_ptr(),
                DEVICE_NAME_LENGTH,
                &format!("{}-device", driver_name),
            );
        }

        device
    }

    unsafe fn open_named_driver(
        driver_name: &str,
        driver: *const nfc_driver,
        context: *const nfc_context,
        connstring: *const c_char,
    ) -> *mut nfc_device {
        let conn = c_string_ptr_to_string(connstring, NFC_BUFSIZE_CONNSTRING);
        with_fake_driver_state(|state| {
            state.open_calls.push(driver_name.to_string());
        });

        let should_fail = with_fake_driver_state(|state| {
            state.failing_connstrings.iter().any(|value| value == &conn)
        });
        if should_fail {
            return ptr::null_mut();
        }

        unsafe { allocate_fake_device(driver, driver_name, context, connstring) }
    }

    unsafe fn scan_named_driver(
        driver_name: &str,
        connstrings: *mut nfc_connstring,
        connstrings_len: usize,
    ) -> usize {
        with_fake_driver_state(|state| {
            state.scan_calls.push(driver_name.to_string());
        });

        let configured = with_fake_driver_state(|state| {
            state
                .scan_results
                .iter()
                .find(|(name, _)| name == driver_name)
                .map(|(_, results)| results.clone())
                .unwrap_or_default()
        });

        let mut copied = 0usize;
        for result in configured.iter().take(connstrings_len) {
            let c_result = CString::new(result.as_bytes()).unwrap();
            if unsafe { copy_connstring_safely(c_result.as_ptr(), connstrings.add(copied)) } {
                copied += 1;
            }
        }

        copied
    }

    unsafe extern "C" fn alpha_scan(
        _context: *const nfc_context,
        connstrings: *mut nfc_connstring,
        connstrings_len: usize,
    ) -> usize {
        unsafe { scan_named_driver("alpha", connstrings, connstrings_len) }
    }

    unsafe extern "C" fn alpha_open(
        context: *const nfc_context,
        connstring: *const c_char,
    ) -> *mut nfc_device {
        unsafe {
            open_named_driver(
                "alpha",
                ptr::addr_of!(TEST_DRIVER_ALPHA),
                context,
                connstring,
            )
        }
    }

    unsafe extern "C" fn alpha_close(device: *mut nfc_device) {
        with_fake_driver_state(|state| {
            state.close_calls.push("alpha".to_string());
        });
        unsafe { nfc_device_free(device) };
    }

    static TEST_DRIVER_ALPHA_NAME: &[u8] = b"alpha\0";
    static TEST_DRIVER_ALPHA: nfc_driver = nfc_driver {
        name: TEST_DRIVER_ALPHA_NAME.as_ptr() as *const c_char,
        scan_type: scan_type_enum::NOT_INTRUSIVE,
        scan: Some(alpha_scan),
        open: Some(alpha_open),
        close: Some(alpha_close),
        strerror: None,
        initiator_init: None,
        initiator_init_secure_element: None,
        initiator_select_passive_target: None,
        initiator_poll_target: None,
        initiator_select_dep_target: None,
        initiator_deselect_target: None,
        initiator_transceive_bytes: None,
        initiator_transceive_bits: None,
        initiator_transceive_bytes_timed: None,
        initiator_transceive_bits_timed: None,
        initiator_target_is_present: None,
        target_init: None,
        target_send_bytes: None,
        target_receive_bytes: None,
        target_send_bits: None,
        target_receive_bits: None,
        device_set_property_bool: None,
        device_set_property_int: None,
        get_supported_modulation: None,
        get_supported_baud_rate: None,
        device_get_information_about: None,
        abort_command: None,
        idle: None,
        powerdown: None,
    };

    unsafe extern "C" fn beta_usb_open(
        context: *const nfc_context,
        connstring: *const c_char,
    ) -> *mut nfc_device {
        unsafe {
            open_named_driver(
                "beta_usb",
                ptr::addr_of!(TEST_DRIVER_BETA_USB),
                context,
                connstring,
            )
        }
    }

    unsafe extern "C" fn beta_usb_close(device: *mut nfc_device) {
        with_fake_driver_state(|state| {
            state.close_calls.push("beta_usb".to_string());
        });
        unsafe { nfc_device_free(device) };
    }

    static TEST_DRIVER_BETA_USB_NAME: &[u8] = b"beta_usb\0";
    static TEST_DRIVER_BETA_USB: nfc_driver = nfc_driver {
        name: TEST_DRIVER_BETA_USB_NAME.as_ptr() as *const c_char,
        scan_type: scan_type_enum::NOT_INTRUSIVE,
        scan: None,
        open: Some(beta_usb_open),
        close: Some(beta_usb_close),
        strerror: None,
        initiator_init: None,
        initiator_init_secure_element: None,
        initiator_select_passive_target: None,
        initiator_poll_target: None,
        initiator_select_dep_target: None,
        initiator_deselect_target: None,
        initiator_transceive_bytes: None,
        initiator_transceive_bits: None,
        initiator_transceive_bytes_timed: None,
        initiator_transceive_bits_timed: None,
        initiator_target_is_present: None,
        target_init: None,
        target_send_bytes: None,
        target_receive_bytes: None,
        target_send_bits: None,
        target_receive_bits: None,
        device_set_property_bool: None,
        device_set_property_int: None,
        get_supported_modulation: None,
        get_supported_baud_rate: None,
        device_get_information_about: None,
        abort_command: None,
        idle: None,
        powerdown: None,
    };

    unsafe extern "C" fn gamma_usb_open(
        _context: *const nfc_context,
        _connstring: *const c_char,
    ) -> *mut nfc_device {
        with_fake_driver_state(|state| {
            state.open_calls.push("gamma_usb".to_string());
        });
        ptr::null_mut()
    }

    unsafe extern "C" fn gamma_usb_close(device: *mut nfc_device) {
        with_fake_driver_state(|state| {
            state.close_calls.push("gamma_usb".to_string());
        });
        unsafe { nfc_device_free(device) };
    }

    static TEST_DRIVER_GAMMA_USB_NAME: &[u8] = b"gamma_usb\0";
    static TEST_DRIVER_GAMMA_USB: nfc_driver = nfc_driver {
        name: TEST_DRIVER_GAMMA_USB_NAME.as_ptr() as *const c_char,
        scan_type: scan_type_enum::NOT_INTRUSIVE,
        scan: None,
        open: Some(gamma_usb_open),
        close: Some(gamma_usb_close),
        strerror: None,
        initiator_init: None,
        initiator_init_secure_element: None,
        initiator_select_passive_target: None,
        initiator_poll_target: None,
        initiator_select_dep_target: None,
        initiator_deselect_target: None,
        initiator_transceive_bytes: None,
        initiator_transceive_bits: None,
        initiator_transceive_bytes_timed: None,
        initiator_transceive_bits_timed: None,
        initiator_target_is_present: None,
        target_init: None,
        target_send_bytes: None,
        target_receive_bytes: None,
        target_send_bits: None,
        target_receive_bits: None,
        device_set_property_bool: None,
        device_set_property_int: None,
        get_supported_modulation: None,
        get_supported_baud_rate: None,
        device_get_information_about: None,
        abort_command: None,
        idle: None,
        powerdown: None,
    };

    unsafe extern "C" fn intrusive_scan(
        _context: *const nfc_context,
        connstrings: *mut nfc_connstring,
        connstrings_len: usize,
    ) -> usize {
        unsafe { scan_named_driver("intrusive", connstrings, connstrings_len) }
    }

    unsafe extern "C" fn intrusive_open(
        context: *const nfc_context,
        connstring: *const c_char,
    ) -> *mut nfc_device {
        unsafe {
            open_named_driver(
                "intrusive",
                ptr::addr_of!(TEST_DRIVER_INTRUSIVE),
                context,
                connstring,
            )
        }
    }

    unsafe extern "C" fn intrusive_close(device: *mut nfc_device) {
        with_fake_driver_state(|state| {
            state.close_calls.push("intrusive".to_string());
        });
        unsafe { nfc_device_free(device) };
    }

    static TEST_DRIVER_INTRUSIVE_NAME: &[u8] = b"intrusive\0";
    static TEST_DRIVER_INTRUSIVE: nfc_driver = nfc_driver {
        name: TEST_DRIVER_INTRUSIVE_NAME.as_ptr() as *const c_char,
        scan_type: scan_type_enum::INTRUSIVE,
        scan: Some(intrusive_scan),
        open: Some(intrusive_open),
        close: Some(intrusive_close),
        strerror: None,
        initiator_init: None,
        initiator_init_secure_element: None,
        initiator_select_passive_target: None,
        initiator_poll_target: None,
        initiator_select_dep_target: None,
        initiator_deselect_target: None,
        initiator_transceive_bytes: None,
        initiator_transceive_bits: None,
        initiator_transceive_bytes_timed: None,
        initiator_transceive_bits_timed: None,
        initiator_target_is_present: None,
        target_init: None,
        target_send_bytes: None,
        target_receive_bytes: None,
        target_send_bits: None,
        target_receive_bits: None,
        device_set_property_bool: None,
        device_set_property_int: None,
        get_supported_modulation: None,
        get_supported_baud_rate: None,
        device_get_information_about: None,
        abort_command: None,
        idle: None,
        powerdown: None,
    };

    fn registry_probe_order_names() -> Vec<String> {
        registry_snapshot()
            .iter()
            .rev()
            .map(|handle| {
                let driver = unsafe { &*handle.0 };
                c_string_ptr_to_string(driver.name, NFC_DRIVER_NAME_MAX)
            })
            .collect()
    }

    fn reset_core_test_world() {
        clear_registry();
        reset_fake_driver_state();
        reset_core_bridge_test_state();
        reset_lifecycle_test_state();
        test_clear_last_log();
    }

    #[test]
    fn register_driver_preserves_existing_probe_order() {
        let _guard = core_test_guard();
        reset_core_test_world();

        unsafe {
            assert_eq!(
                nfc_register_driver(ptr::addr_of!(TEST_DRIVER_ALPHA)),
                NFC_SUCCESS
            );
            assert_eq!(
                nfc_register_driver(ptr::addr_of!(TEST_DRIVER_BETA_USB)),
                NFC_SUCCESS
            );
        }

        assert_eq!(
            registry_probe_order_names(),
            vec!["beta_usb".to_string(), "alpha".to_string()]
        );
    }

    #[test]
    fn init_registers_builtins_only_once() {
        let _guard = core_test_guard();
        reset_core_test_world();

        let mut context = ptr::null_mut();

        unsafe {
            nfc_init_impl(&mut context);
            assert!(!context.is_null());
            nfc_init_impl(&mut context);
            nfc_exit(context);
        }

        assert_eq!(
            registry_probe_order_names(),
            Vec::<String>::new(),
            "nfc_exit should clear the registry after the second init"
        );
    }

    #[test]
    fn init_skips_builtins_when_custom_driver_already_registered() {
        let _guard = core_test_guard();
        reset_core_test_world();

        let mut context = ptr::null_mut();

        unsafe {
            assert_eq!(
                nfc_register_driver(ptr::addr_of!(TEST_DRIVER_BETA_USB)),
                NFC_SUCCESS
            );
            nfc_init_impl(&mut context);
        }

        assert_eq!(registry_probe_order_names(), vec!["beta_usb".to_string()]);

        unsafe { nfc_exit(context) };
    }

    #[test]
    fn exit_clears_registry_and_frees_context() {
        let _guard = core_test_guard();
        reset_core_test_world();

        let context = unsafe { nfc_context_new() };
        assert!(!context.is_null());

        unsafe {
            assert_eq!(
                nfc_register_driver(ptr::addr_of!(TEST_DRIVER_ALPHA)),
                NFC_SUCCESS
            );
            nfc_exit(context);
        }

        assert!(registry_snapshot().is_empty());
        assert_eq!(snapshot_lifecycle_test_state().context_free_calls, 1);
    }

    #[test]
    fn open_matches_exact_driver_name_and_usb_suffix() {
        let _guard = core_test_guard();
        reset_core_test_world();

        unsafe {
            assert_eq!(
                nfc_register_driver(ptr::addr_of!(TEST_DRIVER_ALPHA)),
                NFC_SUCCESS
            );
            assert_eq!(
                nfc_register_driver(ptr::addr_of!(TEST_DRIVER_BETA_USB)),
                NFC_SUCCESS
            );
            assert_eq!(
                nfc_register_driver(ptr::addr_of!(TEST_DRIVER_GAMMA_USB)),
                NFC_SUCCESS
            );
        }

        let context = unsafe { nfc_context_new() };
        let exact = CString::new("alpha:port=1").unwrap();
        let usb = CString::new("usb").unwrap();

        let exact_device = unsafe { nfc_open(context, exact.as_ptr()) };
        assert!(!exact_device.is_null());
        unsafe { bridge_close_device(exact_device) };

        let usb_device = unsafe { nfc_open(context, usb.as_ptr()) };
        assert!(!usb_device.is_null());
        unsafe { bridge_close_device(usb_device) };

        let snapshot = fake_driver_snapshot();
        assert_eq!(
            snapshot.open_calls,
            vec![
                "alpha".to_string(),
                "gamma_usb".to_string(),
                "beta_usb".to_string()
            ]
        );

        unsafe { nfc_exit(context) };
    }

    #[test]
    fn open_uses_list_devices_when_connstring_is_null() {
        let _guard = core_test_guard();
        reset_core_test_world();
        set_scan_results("alpha", &["alpha:port=1"]);

        unsafe {
            assert_eq!(
                nfc_register_driver(ptr::addr_of!(TEST_DRIVER_ALPHA)),
                NFC_SUCCESS
            );
        }

        let context = unsafe { nfc_context_alloc_defaults() };
        let device = unsafe { nfc_open(context, ptr::null()) };

        assert!(!device.is_null());
        assert_eq!(
            fixed_c_buffer_to_string(unsafe { &(*device).connstring }),
            "alpha:port=1".to_string()
        );

        unsafe {
            bridge_close_device(device);
            nfc_exit(context);
        }
    }

    #[test]
    fn open_applies_user_defined_device_name() {
        let _guard = core_test_guard();
        reset_core_test_world();

        unsafe {
            assert_eq!(
                nfc_register_driver(ptr::addr_of!(TEST_DRIVER_ALPHA)),
                NFC_SUCCESS
            );
        }

        let context = unsafe { nfc_context_alloc_defaults() };
        let conn = CString::new("alpha").unwrap();
        unsafe {
            (*context).user_defined_device_count = 1;
            assert!(write_bytes_to_char_buffer(
                (*context).user_defined_devices[0].name.as_mut_ptr(),
                DEVICE_NAME_LENGTH,
                b"my-reader"
            ));
            assert!(copy_connstring_safely(
                conn.as_ptr(),
                &mut (*context).user_defined_devices[0].connstring
            ));
        }

        let device = unsafe { nfc_open(context, conn.as_ptr()) };
        assert!(!device.is_null());
        assert_eq!(
            fixed_c_buffer_to_string(unsafe { &(*device).name }),
            "my-reader".to_string()
        );

        unsafe {
            bridge_close_device(device);
            nfc_exit(context);
        }
    }

    #[test]
    fn open_closes_device_when_name_override_copy_fails() {
        let _guard = core_test_guard();
        reset_core_test_world();

        unsafe {
            assert_eq!(
                nfc_register_driver(ptr::addr_of!(TEST_DRIVER_ALPHA)),
                NFC_SUCCESS
            );
        }

        let context = unsafe { nfc_context_alloc_defaults() };
        let conn = CString::new("alpha").unwrap();
        unsafe {
            (*context).user_defined_device_count = 1;
            for byte in (*context).user_defined_devices[0].name.iter_mut() {
                *byte = b'A' as c_char;
            }
            assert!(copy_connstring_safely(
                conn.as_ptr(),
                &mut (*context).user_defined_devices[0].connstring
            ));
        }

        let device = unsafe { nfc_open(context, conn.as_ptr()) };
        assert!(device.is_null());
        assert_eq!(snapshot_core_bridge_test_state().close_calls, 1);

        unsafe { nfc_exit(context) };
    }

    #[test]
    fn list_devices_skips_unavailable_optional_entries_and_restores_log_env() {
        let _guard = core_test_guard();
        reset_core_test_world();
        add_failing_connstring("alpha:optional");

        unsafe {
            assert_eq!(
                nfc_register_driver(ptr::addr_of!(TEST_DRIVER_ALPHA)),
                NFC_SUCCESS
            );
        }

        let context = unsafe { nfc_context_alloc_defaults() };
        let optional = CString::new("alpha:optional").unwrap();
        unsafe {
            (*context).user_defined_device_count = 1;
            (*context).allow_autoscan = false;
            assert!(write_bytes_to_char_buffer(
                (*context).user_defined_devices[0].name.as_mut_ptr(),
                DEVICE_NAME_LENGTH,
                b"optional-reader"
            ));
            assert!(copy_connstring_safely(
                optional.as_ptr(),
                &mut (*context).user_defined_devices[0].connstring
            ));
            (*context).user_defined_devices[0].optional = true;
        }

        let original = CString::new("7").unwrap();
        unsafe {
            std::env::set_var(
                ENV_LIBNFC_LOG_LEVEL_NAME,
                original.to_string_lossy().as_ref(),
            )
        };

        let mut connstrings = [[0 as c_char; NFC_BUFSIZE_CONNSTRING]; 2];
        let found =
            unsafe { nfc_list_devices(context, connstrings.as_mut_ptr(), connstrings.len()) };
        assert_eq!(found, 0);

        let restored = unsafe { libc::getenv(ENV_LIBNFC_LOG_LEVEL.as_ptr() as *const c_char) };
        assert_eq!(c_string_ptr_to_string(restored, 16), "7".to_string());

        unsafe {
            std::env::remove_var(ENV_LIBNFC_LOG_LEVEL_NAME);
            nfc_exit(context);
        }
    }

    #[test]
    fn list_devices_warns_when_autoscan_is_disabled_without_manual_devices() {
        let _guard = core_test_guard();
        reset_core_test_world();

        let context = unsafe { nfc_context_alloc_defaults() };
        unsafe {
            (*context).allow_autoscan = false;
        }

        let mut connstrings = [[0 as c_char; NFC_BUFSIZE_CONNSTRING]; 1];
        let found =
            unsafe { nfc_list_devices(context, connstrings.as_mut_ptr(), connstrings.len()) };
        assert_eq!(found, 0);
        assert_eq!(
            test_get_last_log(),
            Some(
                "Warning: user must specify device(s) manually when autoscan is disabled"
                    .to_string()
            )
        );

        unsafe { nfc_exit(context) };
    }

    #[test]
    fn list_devices_respects_intrusive_scan_flag() {
        let _guard = core_test_guard();
        reset_core_test_world();
        set_scan_results("intrusive", &["intrusive:device"]);

        unsafe {
            assert_eq!(
                nfc_register_driver(ptr::addr_of!(TEST_DRIVER_INTRUSIVE)),
                NFC_SUCCESS
            );
        }

        let context = unsafe { nfc_context_alloc_defaults() };
        let mut connstrings = [[0 as c_char; NFC_BUFSIZE_CONNSTRING]; 1];

        let without_intrusive =
            unsafe { nfc_list_devices(context, connstrings.as_mut_ptr(), connstrings.len()) };
        assert_eq!(without_intrusive, 0);

        unsafe {
            (*context).allow_intrusive_scan = true;
        }
        let with_intrusive =
            unsafe { nfc_list_devices(context, connstrings.as_mut_ptr(), connstrings.len()) };
        assert_eq!(with_intrusive, 1);
        assert_eq!(
            fixed_c_buffer_to_string(&connstrings[0]),
            "intrusive:device".to_string()
        );

        unsafe { nfc_exit(context) };
    }
}
