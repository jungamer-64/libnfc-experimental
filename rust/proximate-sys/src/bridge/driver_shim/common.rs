use super::*;

pub(super) fn missing_capability(operation: &'static str) -> rt::Error {
    rt::Error::MissingCapability(operation)
}

pub(super) fn driver_caps_from_raw(driver: &nfc_driver) -> rt::DriverCaps {
    let mut caps = rt::DriverCaps::NONE;
    if driver.scan.is_some() {
        caps |= rt::DriverCaps::SCAN;
    }
    if driver.open.is_some() {
        caps |= rt::DriverCaps::OPEN;
    }
    caps
}

pub(super) fn device_caps_from_raw(driver: &nfc_driver) -> rt::DeviceCaps {
    let mut caps = rt::DeviceCaps::NONE;
    if driver.device_get_information_about.is_some() {
        caps |= rt::DeviceCaps::INFO;
    }
    if driver.device_set_property_bool.is_some() {
        caps |= rt::DeviceCaps::SET_PROPERTY_BOOL;
    }
    if driver.device_set_property_int.is_some() {
        caps |= rt::DeviceCaps::SET_PROPERTY_INT;
    }
    if driver.get_supported_modulation.is_some() {
        caps |= rt::DeviceCaps::SUPPORTED_MODULATIONS;
    }
    if driver.get_supported_baud_rate.is_some() {
        caps |= rt::DeviceCaps::SUPPORTED_BAUD_RATES;
    }
    if driver.initiator_init.is_some() {
        caps |= rt::DeviceCaps::INITIATOR_INIT;
    }
    if driver.initiator_init_secure_element.is_some() {
        caps |= rt::DeviceCaps::INITIATOR_INIT_SECURE_ELEMENT;
    }
    if driver.initiator_select_passive_target.is_some() {
        caps |= rt::DeviceCaps::SELECT_PASSIVE_TARGET;
    }
    if driver.initiator_poll_target.is_some() {
        caps |= rt::DeviceCaps::POLL_TARGET;
    }
    if driver.initiator_select_dep_target.is_some() {
        caps |= rt::DeviceCaps::SELECT_DEP_TARGET;
    }
    if driver.initiator_deselect_target.is_some() {
        caps |= rt::DeviceCaps::DESELECT_TARGET;
    }
    if driver.initiator_target_is_present.is_some() {
        caps |= rt::DeviceCaps::TARGET_IS_PRESENT;
    }
    if driver.target_init.is_some() {
        caps |= rt::DeviceCaps::TARGET_INIT;
    }
    if driver.initiator_transceive_bytes.is_some() {
        caps |= rt::DeviceCaps::TRANSCEIVE_BYTES;
    }
    if driver.initiator_transceive_bits.is_some() {
        caps |= rt::DeviceCaps::TRANSCEIVE_BITS;
    }
    if driver.initiator_transceive_bytes_timed.is_some() {
        caps |= rt::DeviceCaps::TRANSCEIVE_BYTES_TIMED;
    }
    if driver.initiator_transceive_bits_timed.is_some() {
        caps |= rt::DeviceCaps::TRANSCEIVE_BITS_TIMED;
    }
    if driver.target_send_bytes.is_some() {
        caps |= rt::DeviceCaps::TARGET_SEND_BYTES;
    }
    if driver.target_receive_bytes.is_some() {
        caps |= rt::DeviceCaps::TARGET_RECEIVE_BYTES;
    }
    if driver.target_send_bits.is_some() {
        caps |= rt::DeviceCaps::TARGET_SEND_BITS;
    }
    if driver.target_receive_bits.is_some() {
        caps |= rt::DeviceCaps::TARGET_RECEIVE_BITS;
    }
    if driver.abort_command.is_some() {
        caps |= rt::DeviceCaps::ABORT_COMMAND;
    }
    if driver.idle.is_some() {
        caps |= rt::DeviceCaps::IDLE;
    }
    if driver.powerdown.is_some() {
        caps |= rt::DeviceCaps::POWERDOWN;
    }
    caps
}

pub(super) fn sync_bool_property(device: *mut nfc_device, property: rt::Property, value: bool) {
    let Some(device) = (unsafe { as_mut(device) }) else {
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

pub(super) fn sync_property_mirrors(device: *mut nfc_device, handle: &dyn rt::OpenedDevice) {
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
    let Some(device) = (unsafe { as_mut(device) }) else {
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
