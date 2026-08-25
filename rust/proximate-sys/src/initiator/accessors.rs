use crate::c_abi::types::{nfc_baud_rate, nfc_mode, nfc_modulation_type};
use crate::c_boundary::raw::{bounded_strlen, c_string_ptr_to_string, copy_bytes_to_c_buffer};
use crate::c_boundary::status::{
    NFC_ESOFT, device_last_error, error_message_ptr, invalid_argument_status,
    reset_device_last_error, runtime_result_status,
};
use crate::domain_bridge::decode::modulation_type_from_c;
use crate::domain_bridge::encode::{CStringOut, baud_rate_to_c, modulation_type_to_c};
use crate::initiator::runtime;
use crate::lifecycle::{device_ref, nfc_device};
use crate::{ffi_catch_unwind_int, ffi_catch_unwind_ptr, ffi_catch_unwind_void};
use libc::{c_char, c_int, size_t};
use proximate_driver as rt;
use std::io::{self, Write};
use std::{ptr, slice};

const NULL_ERROR_PREFIX: *const c_char = b"(null)\0" as *const u8 as *const c_char;

fn mode_from_c(mode: nfc_mode) -> Result<rt::Mode, rt::Error> {
    match mode {
        nfc_mode::N_INITIATOR => Ok(rt::Mode::Initiator),
        nfc_mode::N_TARGET => Ok(rt::Mode::Target),
        _ => Err(rt::Error::InvalidArgument("mode")),
    }
}

fn get_supported_modulation_impl(
    device: *mut nfc_device,
    mode: nfc_mode,
    supported: *mut *const nfc_modulation_type,
) -> c_int {
    let Some(abi) = (unsafe { device_ref(device) }) else {
        return invalid_argument_status(device);
    };
    if supported.is_null() {
        return invalid_argument_status(device);
    }
    let mode = match mode_from_c(mode) {
        Ok(mode) => mode,
        Err(_) => return invalid_argument_status(device),
    };
    let values = match runtime::supported_modulations(device, mode) {
        Ok(values) => values,
        Err(error) => return runtime_result_status(device, &error, true),
    };
    let mut encoded: Vec<_> = values.into_iter().map(modulation_type_to_c).collect();
    encoded.push(nfc_modulation_type::NMT_UNDEFINED);
    match abi.cache_modulations(mode, encoded.into_boxed_slice()) {
        Ok(pointer) => {
            unsafe { *supported = pointer };
            reset_device_last_error(device);
            0
        }
        Err(error) => runtime_result_status(device, &error, true),
    }
}

fn get_supported_baud_rate_impl(
    device: *mut nfc_device,
    mode: nfc_mode,
    modulation_type: nfc_modulation_type,
    supported: *mut *const nfc_baud_rate,
) -> c_int {
    let Some(abi) = (unsafe { device_ref(device) }) else {
        return invalid_argument_status(device);
    };
    if supported.is_null() {
        return invalid_argument_status(device);
    }
    let modulation_type = match modulation_type_from_c(modulation_type) {
        Ok(value) => value,
        Err(_) => return invalid_argument_status(device),
    };
    let mode = match mode_from_c(mode) {
        Ok(mode) => mode,
        Err(_) => return invalid_argument_status(device),
    };
    let values = match runtime::supported_baud_rates(device, mode, modulation_type) {
        Ok(values) => values,
        Err(error) => return runtime_result_status(device, &error, true),
    };
    let mut encoded: Vec<_> = values.into_iter().map(baud_rate_to_c).collect();
    encoded.push(nfc_baud_rate::NBR_UNDEFINED);
    match abi.cache_baud_rates(mode, modulation_type, encoded.into_boxed_slice()) {
        Ok(pointer) => {
            unsafe { *supported = pointer };
            reset_device_last_error(device);
            0
        }
        Err(error) => runtime_result_status(device, &error, true),
    }
}

pub(crate) unsafe fn nfc_device_get_name(device: *mut nfc_device) -> *const c_char {
    ffi_catch_unwind_ptr("nfc_device_get_name", || unsafe {
        device_ref(device)
            .map(|device| device.name_ptr().cast_mut())
            .unwrap_or(ptr::null_mut())
    })
    .cast_const()
}

pub(crate) unsafe fn nfc_device_get_connstring(device: *mut nfc_device) -> *const c_char {
    ffi_catch_unwind_ptr("nfc_device_get_connstring", || unsafe {
        device_ref(device)
            .map(|device| device.connstring_ptr().cast_mut())
            .unwrap_or(ptr::null_mut())
    })
    .cast_const()
}

pub(crate) unsafe fn nfc_device_get_supported_modulation(
    device: *mut nfc_device,
    mode: nfc_mode,
    supported: *mut *const nfc_modulation_type,
) -> c_int {
    ffi_catch_unwind_int("nfc_device_get_supported_modulation", NFC_ESOFT, || {
        get_supported_modulation_impl(device, mode, supported)
    })
}

pub(crate) unsafe fn nfc_device_get_supported_baud_rate(
    device: *mut nfc_device,
    modulation_type: nfc_modulation_type,
    supported: *mut *const nfc_baud_rate,
) -> c_int {
    ffi_catch_unwind_int("nfc_device_get_supported_baud_rate", NFC_ESOFT, || {
        get_supported_baud_rate_impl(device, nfc_mode::N_INITIATOR, modulation_type, supported)
    })
}

pub(crate) unsafe fn nfc_device_get_supported_baud_rate_target_mode(
    device: *mut nfc_device,
    modulation_type: nfc_modulation_type,
    supported: *mut *const nfc_baud_rate,
) -> c_int {
    ffi_catch_unwind_int(
        "nfc_device_get_supported_baud_rate_target_mode",
        NFC_ESOFT,
        || get_supported_baud_rate_impl(device, nfc_mode::N_TARGET, modulation_type, supported),
    )
}

pub(crate) unsafe fn nfc_device_get_information_about(
    device: *mut nfc_device,
    buf: *mut *mut c_char,
) -> c_int {
    ffi_catch_unwind_int("nfc_device_get_information_about", NFC_ESOFT, || {
        let output = match unsafe { CStringOut::from_raw(device, buf) } {
            Ok(output) => output,
            Err(status) => return status,
        };
        match runtime::information_about(device) {
            Ok(value) => output.write_back(device, &value),
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub(crate) unsafe fn nfc_device_get_last_error(device: *const nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_device_get_last_error", NFC_ESOFT, || unsafe {
        device_last_error(device)
    })
}

pub(crate) unsafe fn nfc_strerror(device: *const nfc_device) -> *const c_char {
    ffi_catch_unwind_ptr("nfc_strerror", || unsafe {
        error_message_ptr(device_last_error(device)).cast_mut()
    })
    .cast_const()
}

pub(crate) unsafe fn nfc_strerror_r(
    device: *const nfc_device,
    buf: *mut c_char,
    buflen: size_t,
) -> c_int {
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

pub(crate) unsafe fn nfc_perror(device: *const nfc_device, message: *const c_char) {
    ffi_catch_unwind_void("nfc_perror", || unsafe {
        let prefix = if message.is_null() {
            c_string_ptr_to_string(NULL_ERROR_PREFIX, 6)
        } else {
            c_string_ptr_to_string(message, 4096)
        };
        let error = c_string_ptr_to_string(nfc_strerror(device), 128);
        let rendered = format!("{prefix}: {error}\n");
        let mut stderr = io::stderr().lock();
        let _ = stderr.write_all(rendered.as_bytes());
        let _ = stderr.flush();
    });
}
