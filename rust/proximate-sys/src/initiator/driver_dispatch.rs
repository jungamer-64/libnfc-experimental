use super::*;

pub(super) unsafe fn dispatch_driver_call(
    device: *mut nfc_device,
    call: impl FnOnce(&crate::lifecycle::nfc_driver) -> Option<c_int>,
) -> c_int {
    reset_device_last_error(device);
    let Some(device_ref) = (unsafe { as_ref(device) }) else {
        return 0;
    };
    let Some(driver_ref) = (unsafe { as_ref(device_ref.driver) }) else {
        return unsupported_driver_operation(device);
    };

    match call(driver_ref) {
        Some(result) => result,
        None => unsupported_driver_operation(device),
    }
}

pub(super) unsafe fn call_initiator_poll_target_impl(
    device: *mut nfc_device,
    modulations: *const nfc_modulation,
    modulations_len: usize,
    poll_nr: u8,
    period: u8,
    target: *mut nfc_target,
) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver.initiator_poll_target.map(|callback| {
                callback(
                    device,
                    modulations,
                    modulations_len,
                    poll_nr,
                    period,
                    target,
                )
            })
        })
    }
}

pub(super) unsafe fn get_supported_modulation_impl(
    device: *mut nfc_device,
    mode: nfc_mode,
    supported: *mut *const nfc_modulation_type,
) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .get_supported_modulation
                .map(|callback| callback(device, mode, supported))
        })
    }
}

pub(super) unsafe fn get_supported_baud_rate_impl(
    device: *mut nfc_device,
    mode: nfc_mode,
    modulation_type: nfc_modulation_type,
    supported: *mut *const nfc_baud_rate,
) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .get_supported_baud_rate
                .map(|callback| callback(device, mode, modulation_type, supported))
        })
    }
}

pub(super) unsafe fn get_information_about_impl(
    device: *mut nfc_device,
    buf: *mut *mut c_char,
) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .device_get_information_about
                .map(|callback| callback(device, buf))
        })
    }
}

pub(super) unsafe fn call_abort_command_impl(device: *mut nfc_device) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver.abort_command.map(|callback| callback(device))
        })
    }
}

pub(super) unsafe fn call_idle_impl(device: *mut nfc_device) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver.idle.map(|callback| callback(device))
        })
    }
}

#[cfg(test)]
pub(super) unsafe fn copy_target_bytes(dst: *mut nfc_target, src: *const nfc_target) {
    unsafe {
        ptr::copy_nonoverlapping(src as *const u8, dst as *mut u8, size_of::<nfc_target>());
    }
}
