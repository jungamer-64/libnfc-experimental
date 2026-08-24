use super::*;

pub(super) fn missing_capability(operation: &'static str) -> rt::Error {
    rt::Error::MissingCapability(operation)
}

pub(super) fn sync_bool_property(device: *mut nfc_device, property: rt::Property, value: bool) {
    let Some(device) = (unsafe { optional_mut(device) }) else {
        return;
    };

    match property {
        rt::Property::HandleCrc => device.bCrc = value,
        rt::Property::HandleParity => device.bPar = value,
        rt::Property::EasyFraming => device.bEasyFraming = value,
        rt::Property::InfiniteSelect => device.bInfiniteSelect = value,
        rt::Property::AutoIso14443_4 => device.bAutoIso14443_4 = value,
        _ => {}
    }
}

pub(super) fn sync_property_mirrors(device: *mut nfc_device, handle: &dyn rt::PropertyBackend) {
    for property in [
        rt::Property::HandleCrc,
        rt::Property::HandleParity,
        rt::Property::EasyFraming,
        rt::Property::InfiniteSelect,
        rt::Property::AutoIso14443_4,
    ] {
        if let Some(value) = handle.property_bool_state(property) {
            sync_bool_property(device, property, value);
        }
    }
}

pub(super) fn copy_device_identity(
    device: *mut nfc_device,
    name: &str,
    connstring: &rt::ConnectionString,
) -> bool {
    let Some(device) = (unsafe { optional_mut(device) }) else {
        return false;
    };

    let copied_name = unsafe {
        copy_bytes_to_c_buffer(
            device.name.as_mut_ptr(),
            DEVICE_NAME_LENGTH,
            name.as_bytes(),
        )
    };
    let copied_connstring = unsafe {
        copy_bytes_to_c_buffer(
            device.connstring.as_mut_ptr(),
            NFC_BUFSIZE_CONNSTRING,
            connstring.as_str().as_bytes(),
        )
    };
    copied_name && copied_connstring
}

pub(super) fn bytes_ptr(bytes: &[u8]) -> *const u8 {
    if bytes.is_empty() {
        ptr::null()
    } else {
        bytes.as_ptr()
    }
}

pub(super) fn bytes_mut_ptr(bytes: &mut [u8]) -> *mut u8 {
    if bytes.is_empty() {
        ptr::null_mut()
    } else {
        bytes.as_mut_ptr()
    }
}

pub(super) fn optional_bytes_ptr(bytes: Option<&[u8]>) -> *const u8 {
    match bytes {
        Some(value) if !value.is_empty() => value.as_ptr(),
        _ => ptr::null(),
    }
}

pub(super) fn optional_bytes_mut_ptr(bytes: Option<&mut [u8]>) -> *mut u8 {
    match bytes {
        Some(value) if !value.is_empty() => value.as_mut_ptr(),
        _ => ptr::null_mut(),
    }
}
