use super::*;
use crate::bridge::push_driver as bridge_push_driver;

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
pub(super) unsafe fn write_bytes_to_char_buffer(
    dst: *mut c_char,
    dst_size: usize,
    src: &[u8],
) -> bool {
    unsafe { copy_bytes_to_c_buffer(dst, dst_size, src) }
}

#[cfg(test)]
pub(super) unsafe fn copy_connstring_safely(
    source: *const c_char,
    destination: *mut nfc_connstring,
) -> bool {
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
    }
    unsafe { bridge_push_driver(driver) }
}

#[cfg(all(not(test), libnfc_external_bridges))]
unsafe extern "C" {
    fn nfc_close(device: *mut nfc_device);
}

#[cfg(any(test, not(libnfc_external_bridges)))]
pub(super) unsafe fn invoke_driver_close(device: *mut nfc_device) {
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
    }

    #[cfg(not(test))]
    unsafe {
        invoke_driver_close(device);
    }
}

pub unsafe fn nfc_register_driver(driver: *const nfc_driver) -> c_int {
    ffi_catch_unwind_int("nfc_register_driver", NFC_ESOFT, || unsafe {
        push_driver(driver)
    })
}

#[cfg(test)]
#[derive(Clone, Default)]
pub(super) struct CoreBridgeTestState {
    pub(super) close_calls: usize,
}

#[cfg(test)]
thread_local! {
    static CORE_BRIDGE_TEST_STATE: std::cell::RefCell<CoreBridgeTestState> =
        std::cell::RefCell::new(CoreBridgeTestState::default());
}

#[cfg(test)]
pub(super) fn reset_core_bridge_test_state() {
    CORE_BRIDGE_TEST_STATE.with(|cell| {
        *cell.borrow_mut() = CoreBridgeTestState::default();
    });
}

#[cfg(test)]
pub(super) fn snapshot_core_bridge_test_state() -> CoreBridgeTestState {
    CORE_BRIDGE_TEST_STATE.with(|cell| cell.borrow().clone())
}

#[cfg(test)]
pub unsafe fn nfc_close(device: *mut nfc_device) {
    CORE_BRIDGE_TEST_STATE.with(|cell| {
        cell.borrow_mut().close_calls += 1;
    });

    unsafe { invoke_driver_close(device) };
}
