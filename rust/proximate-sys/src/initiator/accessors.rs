use super::*;

fn mode_from_c(mode: nfc_mode) -> rt::Mode {
    match mode {
        nfc_mode::N_INITIATOR => rt::Mode::Initiator,
        nfc_mode::N_TARGET => rt::Mode::Target,
    }
}

unsafe fn get_supported_modulation_direct_impl(
    device: *mut nfc_device,
    mode: nfc_mode,
    supported: *mut *const nfc_modulation_type,
) -> c_int {
    if supported.is_null() {
        unsafe { set_device_last_error(device, NFC_EINVARG) };
        return NFC_EINVARG;
    }

    let mut adapter = borrowed_device(device);
    match adapter.supported_modulations(mode_from_c(mode)) {
        Ok(values) => {
            let Some(state) = (unsafe { rust_device_state_mut(device) }) else {
                unsafe { set_device_last_error(device, NFC_EINVARG) };
                return NFC_EINVARG;
            };
            state.supported_modulations.clear();
            state
                .supported_modulations
                .extend(values.into_iter().map(modulation_type_to_c));
            state
                .supported_modulations
                .push(nfc_modulation_type::NMT_UNDEFINED);
            unsafe {
                *supported = state.supported_modulations.as_ptr();
            }
            unsafe { set_device_last_error(device, 0) };
            state.supported_modulations.len().saturating_sub(1) as c_int
        }
        Err(error) => runtime_result_status(device, &error, true),
    }
}

unsafe fn get_supported_baud_rate_direct_impl(
    device: *mut nfc_device,
    mode: nfc_mode,
    modulation_type: nfc_modulation_type,
    supported: *mut *const nfc_baud_rate,
) -> c_int {
    if supported.is_null() {
        unsafe { set_device_last_error(device, NFC_EINVARG) };
        return NFC_EINVARG;
    }

    let mut adapter = borrowed_device(device);
    match adapter.supported_baud_rates(mode_from_c(mode), modulation_type_from_c(modulation_type)) {
        Ok(values) => {
            let Some(state) = (unsafe { rust_device_state_mut(device) }) else {
                unsafe { set_device_last_error(device, NFC_EINVARG) };
                return NFC_EINVARG;
            };
            state.supported_baud_rates.clear();
            state
                .supported_baud_rates
                .extend(values.into_iter().map(baud_rate_to_c));
            state
                .supported_baud_rates
                .push(nfc_baud_rate::NBR_UNDEFINED);
            unsafe {
                *supported = state.supported_baud_rates.as_ptr();
            }
            unsafe { set_device_last_error(device, 0) };
            state.supported_baud_rates.len().saturating_sub(1) as c_int
        }
        Err(error) => runtime_result_status(device, &error, true),
    }
}

unsafe fn get_information_about_direct_impl(
    device: *mut nfc_device,
    buf: *mut *mut c_char,
) -> c_int {
    if buf.is_null() {
        unsafe { set_device_last_error(device, NFC_EINVARG) };
        return NFC_EINVARG;
    }

    let mut adapter = borrowed_device(device);
    match adapter.information_about() {
        Ok(value) => {
            let rendered = match CString::new(value) {
                Ok(value) => value,
                Err(_) => {
                    unsafe { set_device_last_error(device, NFC_ESOFT) };
                    return NFC_ESOFT;
                }
            };
            let bytes = rendered.as_bytes_with_nul();
            let allocation = unsafe { libc::malloc(bytes.len()) as *mut c_char };
            if allocation.is_null() {
                unsafe { set_device_last_error(device, NFC_ESOFT) };
                return NFC_ESOFT;
            }

            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr().cast::<c_char>(), allocation, bytes.len());
                *buf = allocation;
            }
            unsafe { set_device_last_error(device, 0) };
            rendered.as_bytes().len() as c_int
        }
        Err(error) => runtime_result_status(device, &error, true),
    }
}

pub unsafe fn nfc_device_get_name(device: *mut nfc_device) -> *const c_char {
    ffi_catch_unwind_ptr("nfc_device_get_name", || unsafe {
        as_ref(device)
            .map(|device| device.name.as_ptr().cast_mut())
            .unwrap_or(ptr::null_mut())
    }) as *const c_char
}

pub unsafe fn nfc_device_get_connstring(device: *mut nfc_device) -> *const c_char {
    ffi_catch_unwind_ptr("nfc_device_get_connstring", || unsafe {
        as_ref(device)
            .map(|device| device.connstring.as_ptr().cast_mut())
            .unwrap_or(ptr::null_mut())
    }) as *const c_char
}

pub unsafe fn nfc_device_get_supported_modulation(
    device: *mut nfc_device,
    mode: nfc_mode,
    supported: *mut *const nfc_modulation_type,
) -> c_int {
    ffi_catch_unwind_int(
        "nfc_device_get_supported_modulation",
        NFC_ESOFT,
        || unsafe {
            if is_rust_shim_device(device) {
                get_supported_modulation_direct_impl(device, mode, supported)
            } else {
                get_supported_modulation_impl(device, mode, supported)
            }
        },
    )
}

pub unsafe fn nfc_device_get_supported_baud_rate(
    device: *mut nfc_device,
    modulation_type: nfc_modulation_type,
    supported: *mut *const nfc_baud_rate,
) -> c_int {
    ffi_catch_unwind_int("nfc_device_get_supported_baud_rate", NFC_ESOFT, || unsafe {
        if is_rust_shim_device(device) {
            get_supported_baud_rate_direct_impl(
                device,
                nfc_mode::N_INITIATOR,
                modulation_type,
                supported,
            )
        } else {
            get_supported_baud_rate_impl(device, nfc_mode::N_INITIATOR, modulation_type, supported)
        }
    })
}

pub unsafe fn nfc_device_get_supported_baud_rate_target_mode(
    device: *mut nfc_device,
    modulation_type: nfc_modulation_type,
    supported: *mut *const nfc_baud_rate,
) -> c_int {
    ffi_catch_unwind_int(
        "nfc_device_get_supported_baud_rate_target_mode",
        NFC_ESOFT,
        || unsafe {
            if is_rust_shim_device(device) {
                get_supported_baud_rate_direct_impl(
                    device,
                    nfc_mode::N_TARGET,
                    modulation_type,
                    supported,
                )
            } else {
                get_supported_baud_rate_impl(device, nfc_mode::N_TARGET, modulation_type, supported)
            }
        },
    )
}

pub unsafe fn nfc_device_get_information_about(
    device: *mut nfc_device,
    buf: *mut *mut c_char,
) -> c_int {
    ffi_catch_unwind_int("nfc_device_get_information_about", NFC_ESOFT, || unsafe {
        if is_rust_shim_device(device) {
            get_information_about_direct_impl(device, buf)
        } else {
            get_information_about_impl(device, buf)
        }
    })
}

pub unsafe fn nfc_device_get_last_error(device: *const nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_device_get_last_error", NFC_ESOFT, || unsafe {
        device_last_error(device)
    })
}

pub unsafe fn nfc_strerror(device: *const nfc_device) -> *const c_char {
    ffi_catch_unwind_ptr("nfc_strerror", || unsafe {
        error_message_ptr(device_last_error(device)).cast_mut()
    }) as *const c_char
}

pub unsafe fn nfc_strerror_r(device: *const nfc_device, buf: *mut c_char, buflen: size_t) -> c_int {
    ffi_catch_unwind_int("nfc_strerror_r", NFC_ESOFT, || unsafe {
        if buflen == 0 {
            return 0;
        }

        if buf.is_null() {
            return -1;
        }

        let message = nfc_strerror(device);
        let max_copy = buflen.saturating_sub(1);
        let message_len = bounded_strlen(message, max_copy.saturating_add(1));
        let copy_len = message_len.min(max_copy);
        let bytes = slice::from_raw_parts(message.cast::<u8>(), copy_len);

        if copy_bytes_to_c_buffer(buf, buflen, bytes) {
            0
        } else {
            -1
        }
    })
}

pub unsafe fn nfc_perror(device: *const nfc_device, message: *const c_char) {
    ffi_catch_unwind_void("nfc_perror", || unsafe {
        let prefix = if message.is_null() {
            c_string_ptr_to_string(NULL_ERROR_PREFIX, 6)
        } else {
            c_string_ptr_to_string(message, 4096)
        };
        let error = c_string_ptr_to_string(nfc_strerror(device), 128);
        if let Ok(rendered) = CString::new(format!("{}: {}\n", prefix, error)) {
            libc::fputs(rendered.as_ptr(), stderr);
        }
    });
}
