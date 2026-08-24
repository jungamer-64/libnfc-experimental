use super::*;

pub(crate) struct RustDeviceState {
    pub(crate) handle: Box<dyn rt::DeviceHandle>,
    pub(crate) strerror: CString,
    pub(crate) supported_modulations: Vec<nfc_modulation_type>,
    pub(crate) supported_baud_rates: Vec<nfc_baud_rate>,
}

pub(crate) unsafe fn rust_device_state_mut<'a>(
    device: *mut nfc_device,
) -> Option<&'a mut RustDeviceState> {
    let device = unsafe { optional_mut(device) }?;
    unsafe { optional_mut(device.driver_data as *mut RustDeviceState) }
}

fn refresh_cached_strerror(state: &mut RustDeviceState) -> *const c_char {
    let message = CString::new(state.handle.strerror())
        .unwrap_or_else(|_| CString::new("invalid strerror").expect("static string is valid"));
    state.strerror = message;
    state.strerror.as_ptr()
}

pub(crate) unsafe fn free_rust_device(device: *mut nfc_device) {
    let Some(device_ref) = (unsafe { optional_mut(device) }) else {
        return;
    };

    let state_ptr = device_ref.driver_data as *mut RustDeviceState;
    let abort_ptr = device_ref.command_abort as *mut rt::CommandAbortHandle;
    device_ref.driver_data = ptr::null_mut();
    device_ref.command_abort = ptr::null_mut();
    device_ref.driver = ptr::null();

    if !state_ptr.is_null() {
        // SAFETY: attach_rust_device allocated this exact RustDeviceState and
        // clearing driver_data above prevents a second ownership recovery.
        unsafe { drop(Box::from_raw(state_ptr)) };
    }
    if !abort_ptr.is_null() {
        // SAFETY: attach_rust_device allocated this exact abort handle and
        // clearing command_abort above prevents a second ownership recovery.
        unsafe { drop(Box::from_raw(abort_ptr)) };
    }

    unsafe { release_allocated_ptr(device.cast()) };
}

pub(crate) fn attach_rust_device(
    device: rt::Device,
    context: *const nfc_context,
) -> Option<*mut nfc_device> {
    let name = device.name().to_string();
    let connstring = device.connstring().clone();
    let last_error = device.last_error();
    let command_abort = device.command_abort_handle();
    let connstring_c = CString::new(connstring.as_str()).ok()?;
    let raw = unsafe { nfc_device_new(context, connstring_c.as_ptr()) };
    if raw.is_null() {
        return None;
    }

    if !copy_device_identity(raw, &name, &connstring) {
        unsafe { release_allocated_ptr(raw.cast()) };
        return None;
    }

    let mut state = Box::new(RustDeviceState {
        handle: device.into_backend(),
        strerror: CString::new("success").expect("static string is valid"),
        supported_modulations: Vec::new(),
        supported_baud_rates: Vec::new(),
    });
    sync_property_mirrors(raw, state.handle.as_ref());
    let _ = refresh_cached_strerror(&mut state);

    unsafe {
        (*raw).driver = ptr::null();
        (*raw).driver_data = Box::into_raw(state).cast();
        (*raw).command_abort = command_abort
            .map(|handle| Box::into_raw(Box::new(handle)).cast())
            .unwrap_or(ptr::null_mut());
        set_device_last_error(raw, last_error);
    }

    Some(raw)
}

pub(crate) unsafe fn rust_command_abort_handle<'a>(
    device: *mut nfc_device,
) -> Option<&'a rt::CommandAbortHandle> {
    // SAFETY: the caller guarantees device remains live for the returned borrow.
    let device = unsafe { optional_ref(device) }?;
    // SAFETY: a non-null command_abort was allocated as CommandAbortHandle by
    // attach_rust_device and remains owned by the live nfc_device.
    unsafe { optional_ref(device.command_abort.cast::<rt::CommandAbortHandle>()) }
}

pub(crate) fn is_rust_shim_device(raw: *mut nfc_device) -> bool {
    let Some(device) = (unsafe { optional_ref(raw) }) else {
        return false;
    };

    if device.driver.is_null() {
        return !device.driver_data.is_null();
    }

    unsafe { optional_ref(device.driver) }
        .map(is_rust_device_driver)
        .unwrap_or(false)
}

#[cfg(test)]
unsafe extern "C" fn rust_test_close(device: *mut nfc_device) {
    unsafe { free_rust_device(device) };
}

#[cfg(test)]
pub(super) fn build_rust_device_shim_driver() -> nfc_driver {
    nfc_driver {
        name: RUST_DEVICE_DRIVER_NAME,
        scan_type: scan_type_enum::NOT_AVAILABLE,
        scan: None,
        open: None,
        close: Some(rust_test_close),
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
    }
}
