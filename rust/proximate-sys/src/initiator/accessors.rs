use crate::bridge::decode::modulation_type_from_c;
use crate::bridge::driver_shim::is_rust_shim_device;
use crate::bridge::encode::{CStringOut, SupportedBaudRatesOut, SupportedModulationsOut};
use crate::bridge::status::{
    NFC_ESOFT, device_last_error, error_message_ptr, runtime_result_status,
};
use crate::ffi_catch_unwind_int;
use crate::ffi_catch_unwind_ptr;
use crate::ffi_catch_unwind_void;
use crate::ffi_support::{as_ref, bounded_strlen, c_string_ptr_to_string, copy_bytes_to_c_buffer};
use crate::ffi_types::{nfc_baud_rate, nfc_mode, nfc_modulation_type};
use crate::initiator::driver_dispatch::{
    get_information_about_impl, get_supported_baud_rate_impl, get_supported_modulation_impl,
};
use crate::initiator::runtime;
use crate::lifecycle::nfc_device;
use libc::{c_char, c_int, size_t};
use proximate_driver as rt;
use std::ffi::CString;
use std::ptr;
use std::slice;

const NULL_ERROR_PREFIX: *const c_char = b"(null)\0" as *const u8 as *const c_char;

unsafe extern "C" {
    static mut stderr: *mut libc::FILE;
}

fn mode_from_c(mode: nfc_mode) -> rt::Mode {
    match mode {
        nfc_mode::N_INITIATOR => rt::Mode::Initiator,
        nfc_mode::N_TARGET => rt::Mode::Target,
    }
}

fn get_supported_modulation_direct_impl(
    device: *mut nfc_device,
    mode: nfc_mode,
    supported: *mut *const nfc_modulation_type,
) -> c_int {
    let supported = match unsafe { SupportedModulationsOut::from_raw(device, supported) } {
        Ok(supported) => supported,
        Err(status) => return status,
    };

    match runtime::supported_modulations(device, mode_from_c(mode)) {
        Ok(values) => supported.write_back(values),
        Err(error) => runtime_result_status(device, &error, true),
    }
}

fn get_supported_baud_rate_direct_impl(
    device: *mut nfc_device,
    mode: nfc_mode,
    modulation_type: nfc_modulation_type,
    supported: *mut *const nfc_baud_rate,
) -> c_int {
    let supported = match unsafe { SupportedBaudRatesOut::from_raw(device, supported) } {
        Ok(supported) => supported,
        Err(status) => return status,
    };

    match runtime::supported_baud_rates(
        device,
        mode_from_c(mode),
        modulation_type_from_c(modulation_type),
    ) {
        Ok(values) => supported.write_back(values),
        Err(error) => runtime_result_status(device, &error, true),
    }
}

fn get_information_about_direct_impl(device: *mut nfc_device, buf: *mut *mut c_char) -> c_int {
    let output = match unsafe { CStringOut::from_raw(device, buf) } {
        Ok(output) => output,
        Err(status) => return status,
    };

    match runtime::information_about(device) {
        Ok(value) => output.write_back(device, &value),
        Err(error) => runtime_result_status(device, &error, true),
    }
}

pub(crate) unsafe fn nfc_device_get_name(device: *mut nfc_device) -> *const c_char {
    ffi_catch_unwind_ptr("nfc_device_get_name", || unsafe {
        as_ref(device)
            .map(|device| device.name.as_ptr().cast_mut())
            .unwrap_or(ptr::null_mut())
    }) as *const c_char
}

pub(crate) unsafe fn nfc_device_get_connstring(device: *mut nfc_device) -> *const c_char {
    ffi_catch_unwind_ptr("nfc_device_get_connstring", || unsafe {
        as_ref(device)
            .map(|device| device.connstring.as_ptr().cast_mut())
            .unwrap_or(ptr::null_mut())
    }) as *const c_char
}

pub(crate) unsafe fn nfc_device_get_supported_modulation(
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

pub(crate) unsafe fn nfc_device_get_supported_baud_rate(
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

pub(crate) unsafe fn nfc_device_get_supported_baud_rate_target_mode(
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

pub(crate) unsafe fn nfc_device_get_information_about(
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

pub(crate) unsafe fn nfc_device_get_last_error(device: *const nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_device_get_last_error", NFC_ESOFT, || unsafe {
        device_last_error(device)
    })
}

pub(crate) unsafe fn nfc_strerror(device: *const nfc_device) -> *const c_char {
    ffi_catch_unwind_ptr("nfc_strerror", || unsafe {
        error_message_ptr(device_last_error(device)).cast_mut()
    }) as *const c_char
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
        if let Ok(rendered) = CString::new(format!("{}: {}\n", prefix, error)) {
            libc::fputs(rendered.as_ptr(), stderr);
        }
    });
}
