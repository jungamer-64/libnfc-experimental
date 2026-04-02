use super::*;

pub(super) fn log_general_message(priority: u8, message: &str) {
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

pub(super) fn log_general_debug(message: &str) {
    log_general_message(LOG_PRIORITY_DEBUG, message);
}

pub(super) fn error_message_ptr(code: c_int) -> *const c_char {
    device_error_message_cstr(code).as_ptr()
}

pub(super) unsafe fn device_last_error(device: *const nfc_device) -> c_int {
    unsafe { as_ref(device) }
        .map(|device| device.last_error)
        .unwrap_or(0)
}

pub(super) unsafe fn set_device_last_error(device: *mut nfc_device, value: c_int) {
    if let Some(device) = unsafe { as_mut(device) } {
        device.last_error = value;
    }
}

pub(super) unsafe fn reset_device_last_error(device: *mut nfc_device) {
    unsafe { set_device_last_error(device, 0) };
}

pub(super) fn runtime_result_status(
    device: *mut nfc_device,
    error: &rt::Error,
    unsupported_as_zero: bool,
) -> c_int {
    let status = error_to_status(error);
    unsafe { set_device_last_error(device, status) };
    if unsupported_as_zero && status == NFC_EDEVNOTSUPP {
        0
    } else {
        status
    }
}

pub(super) unsafe fn unsupported_driver_operation(device: *mut nfc_device) -> c_int {
    unsafe { set_device_last_error(device, NFC_EDEVNOTSUPP) };
    0
}

pub(super) unsafe fn input_bytes<'a>(
    device: *mut nfc_device,
    bytes: *const u8,
    len: size_t,
) -> Result<&'a [u8], c_int> {
    if len == 0 {
        return Ok(&[]);
    }
    if bytes.is_null() {
        unsafe { set_device_last_error(device, NFC_EINVARG) };
        return Err(NFC_EINVARG);
    }
    Ok(unsafe { slice::from_raw_parts(bytes, len) })
}

pub(super) unsafe fn output_bytes<'a>(
    device: *mut nfc_device,
    bytes: *mut u8,
    len: size_t,
) -> Result<&'a mut [u8], c_int> {
    if len == 0 {
        return Ok(&mut []);
    }
    if bytes.is_null() {
        unsafe { set_device_last_error(device, NFC_EINVARG) };
        return Err(NFC_EINVARG);
    }
    Ok(unsafe { slice::from_raw_parts_mut(bytes, len) })
}

pub(super) unsafe fn marker_bytes<'a>(bytes: *const u8) -> Option<&'a [u8]> {
    if bytes.is_null() {
        None
    } else {
        Some(unsafe { slice::from_raw_parts(bytes, 1) })
    }
}

pub(super) unsafe fn marker_bytes_mut<'a>(bytes: *mut u8) -> Option<&'a mut [u8]> {
    if bytes.is_null() {
        None
    } else {
        Some(unsafe { slice::from_raw_parts_mut(bytes, 1) })
    }
}

pub(super) fn property_name(property: nfc_property) -> &'static str {
    property_from_c(property).name()
}
