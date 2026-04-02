// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Ported from libnfc/nfc.c.

use crate::c_api_impl::{LOG_GROUP_GENERAL, LOG_PRIORITY_DEBUG};
use crate::ffi_strings::device_error_message_cstr;
use crate::ffi_support::{
    as_mut, as_ref, bounded_strlen, c_string_ptr_to_string, copy_bytes_to_c_buffer,
};
use crate::ffi_types::{
    nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_mode, nfc_modulation, nfc_modulation_type,
    nfc_property, nfc_target,
};
use crate::lifecycle::nfc_device;
use crate::runtime_bridge::{
    baud_rate_from_c, borrowed_device, dep_mode_from_c, error_to_status, is_rust_shim_device,
    modulation_from_c, property_from_c, target_from_c, write_target_to_c,
};
use crate::{emit_log_message, ffi_catch_unwind_int, ffi_catch_unwind_ptr, ffi_catch_unwind_void};
use libc::{c_char, c_int, c_void, size_t};
use proximate::rust_api as rt;
use std::ffi::CString;
#[cfg(test)]
use std::mem::size_of;
use std::ptr;
use std::slice;

const NFC_EINVARG: c_int = -2;
const NFC_EDEVNOTSUPP: c_int = -3;
#[cfg(test)]
const NFC_ETIMEOUT: c_int = -6;
const NFC_ESOFT: c_int = -80;
const ISO7816_SHORT_C_APDU_MAX_LEN: usize = 261;
const ISO7816_SHORT_R_APDU_MAX_LEN: usize = 258;

const GENERAL_LOG_CATEGORY: *const c_char = b"libnfc.general\0" as *const u8 as *const c_char;
const NULL_ERROR_PREFIX: *const c_char = b"(null)\0" as *const u8 as *const c_char;

#[allow(non_camel_case_types)]
pub type nfc_emulation_io_fn = Option<
    unsafe extern "C" fn(
        emulator: *mut nfc_emulator,
        data_in: *const u8,
        data_in_len: size_t,
        data_out: *mut u8,
        data_out_len: size_t,
    ) -> c_int,
>;

#[repr(C)]
pub struct nfc_emulator {
    pub target: *mut nfc_target,
    pub state_machine: *mut nfc_emulation_state_machine,
    pub user_data: *mut c_void,
}

#[repr(C)]
pub struct nfc_emulation_state_machine {
    pub io: nfc_emulation_io_fn,
    pub data: *mut c_void,
}

unsafe extern "C" {
    static mut stderr: *mut libc::FILE;
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

fn error_message_ptr(code: c_int) -> *const c_char {
    device_error_message_cstr(code).as_ptr()
}

unsafe fn device_last_error(device: *const nfc_device) -> c_int {
    unsafe { as_ref(device) }
        .map(|device| device.last_error)
        .unwrap_or(0)
}

unsafe fn set_device_last_error(device: *mut nfc_device, value: c_int) {
    if let Some(device) = unsafe { as_mut(device) } {
        device.last_error = value;
    }
}

unsafe fn reset_device_last_error(device: *mut nfc_device) {
    unsafe { set_device_last_error(device, 0) };
}

fn runtime_result_status(error: &rt::Error, unsupported_as_zero: bool) -> c_int {
    let status = error_to_status(error);
    if unsupported_as_zero && status == NFC_EDEVNOTSUPP {
        0
    } else {
        status
    }
}

unsafe fn unsupported_driver_operation(device: *mut nfc_device) -> c_int {
    unsafe { set_device_last_error(device, NFC_EDEVNOTSUPP) };
    0
}

unsafe fn dispatch_driver_call(
    device: *mut nfc_device,
    call: impl FnOnce(&crate::lifecycle::nfc_driver) -> Option<c_int>,
) -> c_int {
    unsafe { reset_device_last_error(device) };
    let Some(device_ref) = (unsafe { as_ref(device) }) else {
        return 0;
    };
    let Some(driver_ref) = (unsafe { as_ref(device_ref.driver) }) else {
        return unsafe { unsupported_driver_operation(device) };
    };

    match call(driver_ref) {
        Some(result) => result,
        None => unsafe { unsupported_driver_operation(device) },
    }
}

unsafe fn input_bytes<'a>(
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

unsafe fn output_bytes<'a>(
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

unsafe fn marker_bytes<'a>(bytes: *const u8) -> Option<&'a [u8]> {
    if bytes.is_null() {
        None
    } else {
        Some(unsafe { slice::from_raw_parts(bytes, 1) })
    }
}

unsafe fn marker_bytes_mut<'a>(bytes: *mut u8) -> Option<&'a mut [u8]> {
    if bytes.is_null() {
        None
    } else {
        Some(unsafe { slice::from_raw_parts_mut(bytes, 1) })
    }
}

fn property_name(property: nfc_property) -> &'static str {
    property_from_c(property).name()
}

unsafe fn call_initiator_poll_target_impl(
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

unsafe fn get_supported_modulation_impl(
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

unsafe fn get_supported_baud_rate_impl(
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

unsafe fn get_information_about_impl(device: *mut nfc_device, buf: *mut *mut c_char) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .device_get_information_about
                .map(|callback| callback(device, buf))
        })
    }
}

unsafe fn call_abort_command_impl(device: *mut nfc_device) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver.abort_command.map(|callback| callback(device))
        })
    }
}

unsafe fn call_idle_impl(device: *mut nfc_device) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver.idle.map(|callback| callback(device))
        })
    }
}

#[cfg(test)]
unsafe fn copy_target_bytes(dst: *mut nfc_target, src: *const nfc_target) {
    unsafe {
        ptr::copy_nonoverlapping(src as *const u8, dst as *mut u8, size_of::<nfc_target>());
    }
}

pub unsafe fn nfc_device_set_property_int(
    device: *mut nfc_device,
    property: nfc_property,
    value: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_device_set_property_int", NFC_ESOFT, || {
        log_general_debug(&format!(
            "set_property_int {} {}",
            property_name(property),
            if value != 0 { "True" } else { "False" }
        ));
        let mut adapter = borrowed_device(device);
        match adapter.set_property_int(property_from_c(property), value) {
            Ok(()) => 0,
            Err(error) => runtime_result_status(&error, true),
        }
    })
}

pub unsafe fn nfc_device_set_property_bool(
    device: *mut nfc_device,
    property: nfc_property,
    enable: bool,
) -> c_int {
    ffi_catch_unwind_int("nfc_device_set_property_bool", NFC_ESOFT, || {
        log_general_debug(&format!(
            "set_property_bool {} {}",
            property_name(property),
            if enable { "True" } else { "False" }
        ));
        let mut adapter = borrowed_device(device);
        match adapter.set_property_bool(property_from_c(property), enable) {
            Ok(()) => 0,
            Err(error) => runtime_result_status(&error, true),
        }
    })
}

pub unsafe fn nfc_initiator_init(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_init", NFC_ESOFT, || {
        let mut adapter = borrowed_device(device);
        match adapter.initiator_init() {
            Ok(status) => status,
            Err(error) => runtime_result_status(&error, true),
        }
    })
}

pub unsafe fn nfc_initiator_init_secure_element(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_init_secure_element", NFC_ESOFT, || {
        let mut adapter = borrowed_device(device);
        match adapter.initiator_init_secure_element() {
            Ok(status) => status,
            Err(error) => runtime_result_status(&error, true),
        }
    })
}

pub unsafe fn nfc_initiator_select_passive_target(
    device: *mut nfc_device,
    nm: nfc_modulation,
    init_data: *const u8,
    init_data_len: size_t,
    target: *mut nfc_target,
) -> c_int {
    ffi_catch_unwind_int(
        "nfc_initiator_select_passive_target",
        NFC_ESOFT,
        || unsafe {
            let payload = match input_bytes(device, init_data, init_data_len) {
                Ok(bytes) if bytes.is_empty() => None,
                Ok(bytes) => Some(bytes),
                Err(status) => return status,
            };

            let mut adapter = borrowed_device(device);
            match adapter.select_passive_target(modulation_from_c(nm), payload) {
                Ok(Some(runtime_target)) => {
                    if !target.is_null() {
                        write_target_to_c(&runtime_target, target);
                    }
                    1
                }
                Ok(None) => 0,
                Err(error) => runtime_result_status(&error, true),
            }
        },
    )
}

pub unsafe fn nfc_initiator_list_passive_targets(
    device: *mut nfc_device,
    nm: nfc_modulation,
    targets: *mut nfc_target,
    targets_len: size_t,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_list_passive_targets", NFC_ESOFT, || unsafe {
        if targets_len == 0 {
            return 0;
        }
        if targets.is_null() {
            set_device_last_error(device, NFC_EINVARG);
            return NFC_EINVARG;
        }

        let mut adapter = borrowed_device(device);
        match adapter.list_passive_targets(modulation_from_c(nm), targets_len as usize) {
            Ok(runtime_targets) => {
                for (index, runtime_target) in runtime_targets.iter().enumerate() {
                    write_target_to_c(runtime_target, targets.add(index));
                }
                runtime_targets.len() as c_int
            }
            Err(error) => runtime_result_status(&error, true),
        }
    })
}

pub unsafe fn nfc_initiator_poll_target(
    device: *mut nfc_device,
    modulations: *const nfc_modulation,
    modulations_len: size_t,
    poll_nr: u8,
    period: u8,
    target: *mut nfc_target,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_poll_target", NFC_ESOFT, || unsafe {
        call_initiator_poll_target_impl(
            device,
            modulations,
            modulations_len,
            poll_nr,
            period,
            target,
        )
    })
}

pub unsafe fn nfc_initiator_select_dep_target(
    device: *mut nfc_device,
    ndm: nfc_dep_mode,
    nbr: nfc_baud_rate,
    initiator: *const nfc_dep_info,
    target: *mut nfc_target,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_select_dep_target", NFC_ESOFT, || unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .initiator_select_dep_target
                .map(|callback| callback(device, ndm, nbr, initiator, target, timeout))
        })
    })
}

pub unsafe fn nfc_initiator_poll_dep_target(
    device: *mut nfc_device,
    ndm: nfc_dep_mode,
    nbr: nfc_baud_rate,
    initiator: *const nfc_dep_info,
    target: *mut nfc_target,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_poll_dep_target", NFC_ESOFT, || unsafe {
        let initiator_info = if initiator.is_null() {
            None
        } else {
            Some(crate::runtime_bridge::dep_info_from_c(ptr::read_unaligned(
                initiator,
            )))
        };
        let mut adapter = borrowed_device(device);
        match adapter.poll_dep_target(
            dep_mode_from_c(ndm),
            baud_rate_from_c(nbr),
            initiator_info.as_ref(),
            timeout,
        ) {
            Ok(Some(runtime_target)) => {
                if !target.is_null() {
                    write_target_to_c(&runtime_target, target);
                }
                1
            }
            Ok(None) => 0,
            Err(error) => runtime_result_status(&error, true),
        }
    })
}

pub unsafe fn nfc_initiator_deselect_target(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_deselect_target", NFC_ESOFT, || {
        let mut adapter = borrowed_device(device);
        match adapter.deselect_target() {
            Ok(()) => 0,
            Err(error) => runtime_result_status(&error, true),
        }
    })
}

pub unsafe fn nfc_initiator_target_is_present(
    device: *mut nfc_device,
    target: *const nfc_target,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_target_is_present", NFC_ESOFT, || {
        let runtime_target = (!target.is_null()).then(|| target_from_c(target));
        let mut adapter = borrowed_device(device);
        match adapter.target_is_present(runtime_target.as_ref()) {
            Ok(true) => 1,
            Ok(false) => 0,
            Err(error) => runtime_result_status(&error, true),
        }
    })
}

pub unsafe fn nfc_target_init(
    device: *mut nfc_device,
    target: *mut nfc_target,
    rx: *mut u8,
    rx_len: size_t,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_target_init", NFC_ESOFT, || unsafe {
        if target.is_null() {
            set_device_last_error(device, NFC_EINVARG);
            return NFC_EINVARG;
        }
        let mut runtime_target = target_from_c(target.cast_const());
        let mut adapter = borrowed_device(device);
        let rx = match output_bytes(device, rx, rx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };

        match adapter.target_init(&mut runtime_target, rx, timeout) {
            Ok(count) => {
                write_target_to_c(&runtime_target, target);
                count as c_int
            }
            Err(error) => runtime_result_status(&error, true),
        }
    })
}

pub unsafe fn nfc_initiator_transceive_bytes(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: size_t,
    rx: *mut u8,
    rx_len: size_t,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_transceive_bytes", NFC_ESOFT, || unsafe {
        let tx = match input_bytes(device, tx, tx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        let rx = match output_bytes(device, rx, rx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        let mut adapter = borrowed_device(device);
        match adapter.transceive_bytes(tx, rx, timeout) {
            Ok(count) => count as c_int,
            Err(error) => runtime_result_status(&error, true),
        }
    })
}

pub unsafe fn nfc_initiator_transceive_bits(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: size_t,
    tx_parity: *const u8,
    rx: *mut u8,
    rx_len: size_t,
    rx_parity: *mut u8,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_transceive_bits", NFC_ESOFT, || unsafe {
        let tx_bytes_len = tx_bits_len.div_ceil(8);
        if tx_bytes_len > 0 && tx.is_null() {
            set_device_last_error(device, NFC_EINVARG);
            return NFC_EINVARG;
        }
        if !is_rust_shim_device(device) {
            return dispatch_driver_call(device, |driver| {
                driver
                    .initiator_transceive_bits
                    .map(|callback| callback(device, tx, tx_bits_len, tx_parity, rx, rx_parity))
            });
        }
        let tx = match input_bytes(device, tx, tx_bytes_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        let rx = match output_bytes(device, rx, rx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        let mut adapter = borrowed_device(device);
        match adapter.transceive_bits(
            tx,
            tx_bits_len,
            marker_bytes(tx_parity),
            rx,
            marker_bytes_mut(rx_parity),
        ) {
            Ok(count) => count as c_int,
            Err(error) => runtime_result_status(&error, true),
        }
    })
}

pub unsafe fn nfc_initiator_transceive_bytes_timed(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: size_t,
    rx: *mut u8,
    rx_len: size_t,
    cycles: *mut u32,
) -> c_int {
    ffi_catch_unwind_int(
        "nfc_initiator_transceive_bytes_timed",
        NFC_ESOFT,
        || unsafe {
            let tx = match input_bytes(device, tx, tx_len) {
                Ok(bytes) => bytes,
                Err(status) => return status,
            };
            let rx = match output_bytes(device, rx, rx_len) {
                Ok(bytes) => bytes,
                Err(status) => return status,
            };
            let mut adapter = borrowed_device(device);
            match adapter.transceive_bytes_timed(tx, rx) {
                Ok((count, measured_cycles)) => {
                    if let Some(cycles) = as_mut(cycles) {
                        *cycles = measured_cycles;
                    }
                    count as c_int
                }
                Err(error) => runtime_result_status(&error, true),
            }
        },
    )
}

pub unsafe fn nfc_initiator_transceive_bits_timed(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: size_t,
    tx_parity: *const u8,
    rx: *mut u8,
    rx_len: size_t,
    rx_parity: *mut u8,
    cycles: *mut u32,
) -> c_int {
    ffi_catch_unwind_int(
        "nfc_initiator_transceive_bits_timed",
        NFC_ESOFT,
        || unsafe {
            let tx_bytes_len = tx_bits_len.div_ceil(8);
            if tx_bytes_len > 0 && tx.is_null() {
                set_device_last_error(device, NFC_EINVARG);
                return NFC_EINVARG;
            }
            if !is_rust_shim_device(device) {
                return dispatch_driver_call(device, |driver| {
                    driver.initiator_transceive_bits_timed.map(|callback| {
                        callback(device, tx, tx_bits_len, tx_parity, rx, rx_parity, cycles)
                    })
                });
            }
            let tx = match input_bytes(device, tx, tx_bytes_len) {
                Ok(bytes) => bytes,
                Err(status) => return status,
            };
            let rx = match output_bytes(device, rx, rx_len) {
                Ok(bytes) => bytes,
                Err(status) => return status,
            };
            let mut adapter = borrowed_device(device);
            match adapter.transceive_bits_timed(
                tx,
                tx_bits_len,
                marker_bytes(tx_parity),
                rx,
                marker_bytes_mut(rx_parity),
            ) {
                Ok((count, measured_cycles)) => {
                    if let Some(cycles) = as_mut(cycles) {
                        *cycles = measured_cycles;
                    }
                    count as c_int
                }
                Err(error) => runtime_result_status(&error, true),
            }
        },
    )
}

pub unsafe fn nfc_target_send_bytes(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: size_t,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_target_send_bytes", NFC_ESOFT, || unsafe {
        let tx = match input_bytes(device, tx, tx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        let mut adapter = borrowed_device(device);
        match adapter.target_send_bytes(tx, timeout) {
            Ok(count) => count as c_int,
            Err(error) => runtime_result_status(&error, true),
        }
    })
}

pub unsafe fn nfc_target_receive_bytes(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: size_t,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_target_receive_bytes", NFC_ESOFT, || unsafe {
        let rx = match output_bytes(device, rx, rx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        let mut adapter = borrowed_device(device);
        match adapter.target_receive_bytes(rx, timeout) {
            Ok(count) => count as c_int,
            Err(error) => runtime_result_status(&error, true),
        }
    })
}

pub unsafe fn nfc_target_send_bits(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: size_t,
    tx_parity: *const u8,
) -> c_int {
    ffi_catch_unwind_int("nfc_target_send_bits", NFC_ESOFT, || unsafe {
        let tx_bytes_len = tx_bits_len.div_ceil(8);
        let tx = match input_bytes(device, tx, tx_bytes_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        let mut adapter = borrowed_device(device);
        match adapter.target_send_bits(tx, tx_bits_len, marker_bytes(tx_parity)) {
            Ok(count) => count as c_int,
            Err(error) => runtime_result_status(&error, true),
        }
    })
}

pub unsafe fn nfc_target_receive_bits(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: size_t,
    rx_parity: *mut u8,
) -> c_int {
    ffi_catch_unwind_int("nfc_target_receive_bits", NFC_ESOFT, || unsafe {
        let rx = match output_bytes(device, rx, rx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        let mut adapter = borrowed_device(device);
        match adapter.target_receive_bits(rx, marker_bytes_mut(rx_parity)) {
            Ok(count) => count as c_int,
            Err(error) => runtime_result_status(&error, true),
        }
    })
}

pub unsafe fn nfc_emulate_target(
    device: *mut nfc_device,
    emulator: *mut nfc_emulator,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_emulate_target", NFC_ESOFT, || unsafe {
        let Some(emulator_ref) = as_mut(emulator) else {
            return NFC_EINVARG;
        };
        let Some(state_machine) = as_mut(emulator_ref.state_machine) else {
            return NFC_EINVARG;
        };
        let Some(callback) = state_machine.io else {
            return NFC_EINVARG;
        };
        if emulator_ref.target.is_null() {
            return NFC_EINVARG;
        }

        let mut rx = [0u8; ISO7816_SHORT_R_APDU_MAX_LEN];
        let mut tx = [0u8; ISO7816_SHORT_C_APDU_MAX_LEN];

        let init_len = nfc_target_init(
            device,
            emulator_ref.target,
            rx.as_mut_ptr(),
            rx.len(),
            timeout,
        );
        if init_len < 0 {
            return init_len;
        }

        let mut rx_len = init_len as usize;
        let mut io_res = init_len;
        while io_res >= 0 {
            io_res = callback(emulator, rx.as_ptr(), rx_len, tx.as_mut_ptr(), tx.len());
            if io_res > 0 {
                let sent = nfc_target_send_bytes(device, tx.as_ptr(), io_res as usize, timeout);
                if sent < 0 {
                    return sent;
                }
            }
            if io_res >= 0 {
                let received = nfc_target_receive_bytes(device, rx.as_mut_ptr(), rx.len(), timeout);
                if received < 0 {
                    return received;
                }
                rx_len = received as usize;
            }
        }

        io_res
    })
}

pub unsafe fn nfc_abort_command(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_abort_command", NFC_ESOFT, || unsafe {
        call_abort_command_impl(device)
    })
}

pub unsafe fn nfc_idle(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_idle", NFC_ESOFT, || unsafe { call_idle_impl(device) })
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
        || unsafe { get_supported_modulation_impl(device, mode, supported) },
    )
}

pub unsafe fn nfc_device_get_supported_baud_rate(
    device: *mut nfc_device,
    modulation_type: nfc_modulation_type,
    supported: *mut *const nfc_baud_rate,
) -> c_int {
    ffi_catch_unwind_int("nfc_device_get_supported_baud_rate", NFC_ESOFT, || unsafe {
        get_supported_baud_rate_impl(device, nfc_mode::N_INITIATOR, modulation_type, supported)
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
            get_supported_baud_rate_impl(device, nfc_mode::N_TARGET, modulation_type, supported)
        },
    )
}

pub unsafe fn nfc_device_get_information_about(
    device: *mut nfc_device,
    buf: *mut *mut c_char,
) -> c_int {
    ffi_catch_unwind_int("nfc_device_get_information_about", NFC_ESOFT, || unsafe {
        get_information_about_impl(device, buf)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::{nfc_context_alloc_defaults, nfc_device_free, nfc_device_new};
    use std::cell::RefCell;
    use std::ffi::{CStr, CString};
    use std::sync::{Mutex, MutexGuard, OnceLock};

    #[derive(Clone, Copy)]
    struct PassiveResponse {
        result: c_int,
        target: nfc_target,
    }

    #[derive(Clone, Default)]
    struct InitiatorTestState {
        property_bool_calls: Vec<(nfc_property, bool)>,
        property_int_calls: Vec<(nfc_property, c_int)>,
        initiator_init_calls: usize,
        initiator_init_secure_element_calls: usize,
        get_supported_modulation_modes: Vec<nfc_mode>,
        get_supported_baud_rate_modes: Vec<(nfc_mode, nfc_modulation_type)>,
        passive_init_payloads: Vec<Vec<u8>>,
        passive_calls: usize,
        passive_responses: Vec<PassiveResponse>,
        deselect_calls: usize,
        poll_target_calls: usize,
        poll_target_return: c_int,
        select_dep_calls: usize,
        select_dep_responses: Vec<c_int>,
        target_is_present_calls: usize,
        target_is_present_return: c_int,
        target_init_calls: Vec<(usize, c_int)>,
        initiator_transceive_bytes_calls: Vec<(usize, usize, c_int)>,
        initiator_transceive_bits_calls: Vec<usize>,
        initiator_transceive_bytes_timed_calls: Vec<(usize, usize)>,
        initiator_transceive_bits_timed_calls: Vec<usize>,
        target_send_bytes_calls: Vec<(usize, c_int)>,
        target_receive_bytes_calls: Vec<(usize, c_int)>,
        target_send_bits_calls: Vec<usize>,
        target_receive_bits_calls: Vec<usize>,
        device_get_information_about_calls: usize,
        abort_command_calls: usize,
        idle_calls: usize,
    }

    thread_local! {
        static INITIATOR_TEST_STATE: RefCell<InitiatorTestState> =
            RefCell::new(InitiatorTestState::default());
    }

    static INITIATOR_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn initiator_test_guard() -> MutexGuard<'static, ()> {
        INITIATOR_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn emulator_state() -> &'static Mutex<usize> {
        static STATE: OnceLock<Mutex<usize>> = OnceLock::new();
        STATE.get_or_init(|| Mutex::new(0))
    }

    fn reset_test_state() {
        INITIATOR_TEST_STATE.with(|cell| {
            *cell.borrow_mut() = InitiatorTestState::default();
        });
    }

    fn with_test_state<R>(f: impl FnOnce(&mut InitiatorTestState) -> R) -> R {
        INITIATOR_TEST_STATE.with(|cell| f(&mut cell.borrow_mut()))
    }

    fn snapshot_test_state() -> InitiatorTestState {
        INITIATOR_TEST_STATE.with(|cell| cell.borrow().clone())
    }

    unsafe fn make_device(driver: *const crate::lifecycle::nfc_driver) -> *mut nfc_device {
        let connstring = CString::new("test-driver").unwrap();
        let device = unsafe { nfc_device_new(ptr::null(), connstring.as_ptr()) };
        assert!(!device.is_null());
        unsafe {
            (*device).driver = driver;
        }
        device
    }

    unsafe fn destroy_device(device: *mut nfc_device) {
        unsafe { nfc_device_free(device) };
    }

    fn zeroed_target_with_marker(marker: u8) -> nfc_target {
        let mut target = unsafe { std::mem::zeroed::<nfc_target>() };
        unsafe {
            let bytes = &mut target as *mut nfc_target as *mut u8;
            *bytes = marker;
            ptr::addr_of_mut!(target.nm.nmt).write_unaligned(nfc_modulation_type::NMT_ISO14443A);
            ptr::addr_of_mut!(target.nm.nbr).write_unaligned(nfc_baud_rate::NBR_106);
        }
        target
    }

    unsafe extern "C" fn test_property_bool(
        _device: *mut nfc_device,
        property: nfc_property,
        enable: bool,
    ) -> c_int {
        with_test_state(|state| {
            state.property_bool_calls.push((property, enable));
        });
        0
    }

    unsafe extern "C" fn test_property_int(
        _device: *mut nfc_device,
        property: nfc_property,
        value: c_int,
    ) -> c_int {
        with_test_state(|state| {
            state.property_int_calls.push((property, value));
        });
        0
    }

    unsafe extern "C" fn test_initiator_init(_device: *mut nfc_device) -> c_int {
        with_test_state(|state| {
            state.initiator_init_calls += 1;
        });
        7
    }

    unsafe extern "C" fn test_initiator_init_secure_element(_device: *mut nfc_device) -> c_int {
        with_test_state(|state| {
            state.initiator_init_secure_element_calls += 1;
        });
        5
    }

    static SUPPORTED_MODULATIONS: [nfc_modulation_type; 5] = [
        nfc_modulation_type::NMT_ISO14443A,
        nfc_modulation_type::NMT_ISO14443B,
        nfc_modulation_type::NMT_ISO14443BI,
        nfc_modulation_type::NMT_FELICA,
        nfc_modulation_type::NMT_UNDEFINED,
    ];

    static SUPPORTED_RATES_106: [nfc_baud_rate; 2] =
        [nfc_baud_rate::NBR_106, nfc_baud_rate::NBR_UNDEFINED];
    static SUPPORTED_RATES_212: [nfc_baud_rate; 2] =
        [nfc_baud_rate::NBR_212, nfc_baud_rate::NBR_UNDEFINED];

    unsafe extern "C" fn test_get_supported_modulation(
        _device: *mut nfc_device,
        mode: nfc_mode,
        supported: *mut *const nfc_modulation_type,
    ) -> c_int {
        with_test_state(|state| {
            state.get_supported_modulation_modes.push(mode);
        });
        unsafe {
            *supported = SUPPORTED_MODULATIONS.as_ptr();
        }
        0
    }

    unsafe extern "C" fn test_get_supported_baud_rate(
        _device: *mut nfc_device,
        mode: nfc_mode,
        modulation_type: nfc_modulation_type,
        supported: *mut *const nfc_baud_rate,
    ) -> c_int {
        with_test_state(|state| {
            state
                .get_supported_baud_rate_modes
                .push((mode, modulation_type));
        });
        unsafe {
            *supported = match modulation_type {
                nfc_modulation_type::NMT_FELICA => SUPPORTED_RATES_212.as_ptr(),
                _ => SUPPORTED_RATES_106.as_ptr(),
            };
        }
        0
    }

    unsafe extern "C" fn test_select_passive_target(
        _device: *mut nfc_device,
        _nm: nfc_modulation,
        init_data: *const u8,
        init_data_len: usize,
        target: *mut nfc_target,
    ) -> c_int {
        let payload = if init_data.is_null() || init_data_len == 0 {
            Vec::new()
        } else {
            unsafe { slice::from_raw_parts(init_data, init_data_len) }.to_vec()
        };

        with_test_state(|state| {
            state.passive_calls += 1;
            state.passive_init_payloads.push(payload);
        });

        let response = with_test_state(|state| {
            if state.passive_responses.is_empty() {
                PassiveResponse {
                    result: 0,
                    target: zeroed_target_with_marker(0),
                }
            } else {
                state.passive_responses.remove(0)
            }
        });

        if response.result > 0 && !target.is_null() {
            unsafe {
                copy_target_bytes(target, ptr::addr_of!(response.target));
            }
        }

        response.result
    }

    unsafe extern "C" fn test_deselect_target(_device: *mut nfc_device) -> c_int {
        with_test_state(|state| {
            state.deselect_calls += 1;
        });
        0
    }

    unsafe extern "C" fn test_poll_target(
        _device: *mut nfc_device,
        _modulations: *const nfc_modulation,
        _modulations_len: usize,
        _poll_nr: u8,
        _period: u8,
        _target: *mut nfc_target,
    ) -> c_int {
        with_test_state(|state| {
            state.poll_target_calls += 1;
            state.poll_target_return
        })
    }

    unsafe extern "C" fn test_select_dep_target(
        _device: *mut nfc_device,
        _ndm: nfc_dep_mode,
        _nbr: nfc_baud_rate,
        _initiator: *const nfc_dep_info,
        _target: *mut nfc_target,
        _timeout: c_int,
    ) -> c_int {
        with_test_state(|state| {
            state.select_dep_calls += 1;
            if state.select_dep_responses.is_empty() {
                0
            } else {
                state.select_dep_responses.remove(0)
            }
        })
    }

    unsafe extern "C" fn test_target_is_present(
        _device: *mut nfc_device,
        _target: *const nfc_target,
    ) -> c_int {
        with_test_state(|state| {
            state.target_is_present_calls += 1;
            state.target_is_present_return
        })
    }

    unsafe extern "C" fn test_target_init(
        _device: *mut nfc_device,
        target: *mut nfc_target,
        rx: *mut u8,
        rx_len: usize,
        timeout: c_int,
    ) -> c_int {
        with_test_state(|state| {
            state.target_init_calls.push((rx_len, timeout));
        });
        if !target.is_null() {
            unsafe {
                *target = zeroed_target_with_marker(0x22);
            }
        }
        if !rx.is_null() && rx_len > 0 {
            unsafe {
                *rx = 0x44;
            }
        }
        11
    }

    unsafe extern "C" fn test_initiator_transceive_bytes(
        _device: *mut nfc_device,
        _tx: *const u8,
        tx_len: usize,
        rx: *mut u8,
        rx_len: usize,
        timeout: c_int,
    ) -> c_int {
        with_test_state(|state| {
            state
                .initiator_transceive_bytes_calls
                .push((tx_len, rx_len, timeout));
        });
        if !rx.is_null() && rx_len > 0 {
            unsafe {
                *rx = 0x51;
            }
        }
        12
    }

    unsafe extern "C" fn test_initiator_transceive_bits(
        _device: *mut nfc_device,
        _tx: *const u8,
        tx_bits_len: usize,
        _tx_parity: *const u8,
        rx: *mut u8,
        rx_parity: *mut u8,
    ) -> c_int {
        with_test_state(|state| {
            state.initiator_transceive_bits_calls.push(tx_bits_len);
        });
        if !rx.is_null() {
            unsafe {
                *rx = 0x61;
            }
        }
        if !rx_parity.is_null() {
            unsafe {
                *rx_parity = 0x01;
            }
        }
        13
    }

    unsafe extern "C" fn test_initiator_transceive_bytes_timed(
        _device: *mut nfc_device,
        _tx: *const u8,
        tx_len: usize,
        rx: *mut u8,
        rx_len: usize,
        cycles: *mut u32,
    ) -> c_int {
        with_test_state(|state| {
            state
                .initiator_transceive_bytes_timed_calls
                .push((tx_len, rx_len));
        });
        if !rx.is_null() && rx_len > 0 {
            unsafe {
                *rx = 0x71;
            }
        }
        if !cycles.is_null() {
            unsafe {
                *cycles = 1234;
            }
        }
        14
    }

    unsafe extern "C" fn test_initiator_transceive_bits_timed(
        _device: *mut nfc_device,
        _tx: *const u8,
        tx_bits_len: usize,
        _tx_parity: *const u8,
        rx: *mut u8,
        rx_parity: *mut u8,
        cycles: *mut u32,
    ) -> c_int {
        with_test_state(|state| {
            state
                .initiator_transceive_bits_timed_calls
                .push(tx_bits_len);
        });
        if !rx.is_null() {
            unsafe {
                *rx = 0x81;
            }
        }
        if !rx_parity.is_null() {
            unsafe {
                *rx_parity = 0x02;
            }
        }
        if !cycles.is_null() {
            unsafe {
                *cycles = 5678;
            }
        }
        15
    }

    unsafe extern "C" fn test_target_send_bytes(
        _device: *mut nfc_device,
        _tx: *const u8,
        tx_len: usize,
        timeout: c_int,
    ) -> c_int {
        with_test_state(|state| {
            state.target_send_bytes_calls.push((tx_len, timeout));
        });
        16
    }

    unsafe extern "C" fn test_target_receive_bytes(
        _device: *mut nfc_device,
        rx: *mut u8,
        rx_len: usize,
        timeout: c_int,
    ) -> c_int {
        with_test_state(|state| {
            state.target_receive_bytes_calls.push((rx_len, timeout));
        });
        if !rx.is_null() && rx_len > 0 {
            unsafe {
                *rx = 0x91;
            }
        }
        17
    }

    unsafe extern "C" fn test_target_send_bits(
        _device: *mut nfc_device,
        _tx: *const u8,
        tx_bits_len: usize,
        _tx_parity: *const u8,
    ) -> c_int {
        with_test_state(|state| {
            state.target_send_bits_calls.push(tx_bits_len);
        });
        18
    }

    unsafe extern "C" fn test_target_receive_bits(
        _device: *mut nfc_device,
        rx: *mut u8,
        rx_len: usize,
        rx_parity: *mut u8,
    ) -> c_int {
        with_test_state(|state| {
            state.target_receive_bits_calls.push(rx_len);
        });
        if !rx.is_null() && rx_len > 0 {
            unsafe {
                *rx = 0xa1;
            }
        }
        if !rx_parity.is_null() {
            unsafe {
                *rx_parity = 0x03;
            }
        }
        19
    }

    unsafe extern "C" fn test_emulator_io(
        _emulator: *mut nfc_emulator,
        data_in: *const u8,
        data_in_len: size_t,
        data_out: *mut u8,
        data_out_len: size_t,
    ) -> c_int {
        let mut state = emulator_state()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *state == 0 {
            assert!(!data_in.is_null());
            assert_eq!(data_in_len, 11);
            assert!(!data_out.is_null());
            assert!(data_out_len >= 2);
            unsafe {
                *data_out = 0xaa;
                *data_out.add(1) = 0xbb;
            }
            *state = 1;
            2
        } else {
            assert!(!data_in.is_null());
            assert_eq!(data_in_len, 17);
            -1
        }
    }

    unsafe extern "C" fn test_device_get_information_about(
        _device: *mut nfc_device,
        buf: *mut *mut c_char,
    ) -> c_int {
        static INFO: &[u8] = b"driver-info\0";
        with_test_state(|state| {
            state.device_get_information_about_calls += 1;
        });
        if !buf.is_null() {
            unsafe {
                *buf = INFO.as_ptr() as *mut c_char;
            }
        }
        20
    }

    unsafe extern "C" fn test_abort_command(_device: *mut nfc_device) -> c_int {
        with_test_state(|state| {
            state.abort_command_calls += 1;
        });
        21
    }

    unsafe extern "C" fn test_idle(_device: *mut nfc_device) -> c_int {
        with_test_state(|state| {
            state.idle_calls += 1;
        });
        22
    }

    static TEST_DRIVER_FULL_NAME: &[u8] = b"initiator_test\0";
    static TEST_DRIVER_FULL: crate::lifecycle::nfc_driver = crate::lifecycle::nfc_driver {
        name: TEST_DRIVER_FULL_NAME.as_ptr() as *const c_char,
        scan_type: crate::lifecycle::scan_type_enum::NOT_INTRUSIVE,
        scan: None,
        open: None,
        close: None,
        strerror: None,
        initiator_init: Some(test_initiator_init),
        initiator_init_secure_element: Some(test_initiator_init_secure_element),
        initiator_select_passive_target: Some(test_select_passive_target),
        initiator_poll_target: Some(test_poll_target),
        initiator_select_dep_target: Some(test_select_dep_target),
        initiator_deselect_target: Some(test_deselect_target),
        initiator_transceive_bytes: Some(test_initiator_transceive_bytes),
        initiator_transceive_bits: Some(test_initiator_transceive_bits),
        initiator_transceive_bytes_timed: Some(test_initiator_transceive_bytes_timed),
        initiator_transceive_bits_timed: Some(test_initiator_transceive_bits_timed),
        initiator_target_is_present: Some(test_target_is_present),
        target_init: Some(test_target_init),
        target_send_bytes: Some(test_target_send_bytes),
        target_receive_bytes: Some(test_target_receive_bytes),
        target_send_bits: Some(test_target_send_bits),
        target_receive_bits: Some(test_target_receive_bits),
        device_set_property_bool: Some(test_property_bool),
        device_set_property_int: Some(test_property_int),
        get_supported_modulation: Some(test_get_supported_modulation),
        get_supported_baud_rate: Some(test_get_supported_baud_rate),
        device_get_information_about: Some(test_device_get_information_about),
        abort_command: Some(test_abort_command),
        idle: Some(test_idle),
        powerdown: None,
    };

    static TEST_DRIVER_MISSING_BOOL_NAME: &[u8] = b"initiator_missing_bool\0";
    static TEST_DRIVER_MISSING_BOOL: crate::lifecycle::nfc_driver = crate::lifecycle::nfc_driver {
        name: TEST_DRIVER_MISSING_BOOL_NAME.as_ptr() as *const c_char,
        scan_type: crate::lifecycle::scan_type_enum::NOT_INTRUSIVE,
        scan: None,
        open: None,
        close: None,
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
        device_set_property_int: Some(test_property_int),
        get_supported_modulation: Some(test_get_supported_modulation),
        get_supported_baud_rate: Some(test_get_supported_baud_rate),
        device_get_information_about: None,
        abort_command: None,
        idle: None,
        powerdown: None,
    };

    static TEST_DRIVER_UNSUPPORTED_SELECT_NAME: &[u8] = b"initiator_unsupported_select\0";
    static TEST_DRIVER_UNSUPPORTED_SELECT: crate::lifecycle::nfc_driver =
        crate::lifecycle::nfc_driver {
            name: TEST_DRIVER_UNSUPPORTED_SELECT_NAME.as_ptr() as *const c_char,
            scan_type: crate::lifecycle::scan_type_enum::NOT_INTRUSIVE,
            scan: None,
            open: None,
            close: None,
            strerror: None,
            initiator_init: None,
            initiator_init_secure_element: None,
            initiator_select_passive_target: None,
            initiator_poll_target: Some(test_poll_target),
            initiator_select_dep_target: Some(test_select_dep_target),
            initiator_deselect_target: Some(test_deselect_target),
            initiator_transceive_bytes: None,
            initiator_transceive_bits: None,
            initiator_transceive_bytes_timed: None,
            initiator_transceive_bits_timed: None,
            initiator_target_is_present: Some(test_target_is_present),
            target_init: None,
            target_send_bytes: None,
            target_receive_bytes: None,
            target_send_bits: None,
            target_receive_bits: None,
            device_set_property_bool: Some(test_property_bool),
            device_set_property_int: Some(test_property_int),
            get_supported_modulation: Some(test_get_supported_modulation),
            get_supported_baud_rate: Some(test_get_supported_baud_rate),
            device_get_information_about: None,
            abort_command: None,
            idle: None,
            powerdown: None,
        };

    #[test]
    fn property_wrappers_preserve_hal_style_missing_callback_behavior() {
        let _guard = initiator_test_guard();
        reset_test_state();

        let device = unsafe { make_device(ptr::addr_of!(TEST_DRIVER_MISSING_BOOL)) };
        let result =
            unsafe { nfc_device_set_property_bool(device, nfc_property::NP_ACTIVATE_FIELD, true) };

        assert_eq!(result, 0);
        assert_eq!(unsafe { (*device).last_error }, NFC_EDEVNOTSUPP);

        unsafe { destroy_device(device) };
    }

    #[test]
    fn property_int_wrapper_logs_and_dispatches() {
        let _guard = initiator_test_guard();
        reset_test_state();

        let device = unsafe { make_device(ptr::addr_of!(TEST_DRIVER_FULL)) };
        let result =
            unsafe { nfc_device_set_property_int(device, nfc_property::NP_TIMEOUT_COMMAND, 42) };

        assert_eq!(result, 0);
        assert_eq!(
            snapshot_test_state().property_int_calls,
            vec![(nfc_property::NP_TIMEOUT_COMMAND, 42)]
        );

        unsafe { destroy_device(device) };
    }

    #[test]
    fn initiator_init_applies_expected_property_sequence() {
        let _guard = initiator_test_guard();
        reset_test_state();

        let device = unsafe { make_device(ptr::addr_of!(TEST_DRIVER_FULL)) };
        let result = unsafe { nfc_initiator_init(device) };
        assert_eq!(result, 7);

        let snapshot = snapshot_test_state();
        assert_eq!(snapshot.initiator_init_calls, 1);
        assert_eq!(
            snapshot.property_bool_calls,
            vec![
                (nfc_property::NP_ACTIVATE_FIELD, false),
                (nfc_property::NP_ACTIVATE_FIELD, true),
                (nfc_property::NP_INFINITE_SELECT, true),
                (nfc_property::NP_AUTO_ISO14443_4, true),
                (nfc_property::NP_FORCE_ISO14443_A, true),
                (nfc_property::NP_FORCE_SPEED_106, true),
                (nfc_property::NP_ACCEPT_INVALID_FRAMES, false),
                (nfc_property::NP_ACCEPT_MULTIPLE_FRAMES, false),
            ]
        );

        unsafe { destroy_device(device) };
    }

    #[test]
    fn initiator_init_secure_element_dispatches_to_driver() {
        let _guard = initiator_test_guard();
        reset_test_state();

        let device = unsafe { make_device(ptr::addr_of!(TEST_DRIVER_FULL)) };
        let result = unsafe { nfc_initiator_init_secure_element(device) };

        assert_eq!(result, 5);
        assert_eq!(snapshot_test_state().initiator_init_secure_element_calls, 1);

        unsafe { destroy_device(device) };
    }

    #[test]
    fn target_init_applies_expected_property_sequence() {
        let _guard = initiator_test_guard();
        reset_test_state();

        let device = unsafe { make_device(ptr::addr_of!(TEST_DRIVER_FULL)) };
        let mut target = zeroed_target_with_marker(0);
        let mut rx = [0u8; 4];

        let result = unsafe {
            nfc_target_init(
                device,
                ptr::addr_of_mut!(target),
                rx.as_mut_ptr(),
                rx.len(),
                250,
            )
        };

        assert_eq!(result, 11);
        assert_eq!(rx[0], 0x44);
        let snapshot = snapshot_test_state();
        assert_eq!(snapshot.target_init_calls, vec![(4, 250)]);
        assert_eq!(
            snapshot.property_bool_calls,
            vec![
                (nfc_property::NP_ACCEPT_INVALID_FRAMES, false),
                (nfc_property::NP_ACCEPT_MULTIPLE_FRAMES, false),
                (nfc_property::NP_HANDLE_CRC, true),
                (nfc_property::NP_HANDLE_PARITY, true),
                (nfc_property::NP_AUTO_ISO14443_4, true),
                (nfc_property::NP_EASY_FRAMING, true),
                (nfc_property::NP_ACTIVATE_CRYPTO1, false),
                (nfc_property::NP_ACTIVATE_FIELD, false),
            ]
        );

        unsafe { destroy_device(device) };
    }

    #[test]
    fn select_passive_target_rejects_unsupported_modulation_and_baud_rate() {
        let _guard = initiator_test_guard();
        reset_test_state();

        let device = unsafe { make_device(ptr::addr_of!(TEST_DRIVER_FULL)) };
        let unsupported_nm = nfc_modulation {
            nmt: nfc_modulation_type::NMT_DEP,
            nbr: nfc_baud_rate::NBR_847,
        };
        let mut target = zeroed_target_with_marker(0);

        let result = unsafe {
            nfc_initiator_select_passive_target(
                device,
                unsupported_nm,
                ptr::null(),
                0,
                ptr::addr_of_mut!(target),
            )
        };
        assert_eq!(result, NFC_EINVARG);

        unsafe { destroy_device(device) };
    }

    #[test]
    fn default_initiator_payloads_match_c_behavior() {
        let _guard = initiator_test_guard();
        reset_test_state();

        with_test_state(|state| {
            state.passive_responses = vec![
                PassiveResponse {
                    result: 1,
                    target: zeroed_target_with_marker(1),
                },
                PassiveResponse {
                    result: 1,
                    target: zeroed_target_with_marker(2),
                },
                PassiveResponse {
                    result: 1,
                    target: zeroed_target_with_marker(3),
                },
            ];
        });

        let device = unsafe { make_device(ptr::addr_of!(TEST_DRIVER_FULL)) };
        let mut target = zeroed_target_with_marker(0);

        assert_eq!(
            unsafe {
                nfc_initiator_select_passive_target(
                    device,
                    nfc_modulation {
                        nmt: nfc_modulation_type::NMT_ISO14443B,
                        nbr: nfc_baud_rate::NBR_106,
                    },
                    ptr::null(),
                    0,
                    ptr::addr_of_mut!(target),
                )
            },
            1
        );
        assert_eq!(
            unsafe {
                nfc_initiator_select_passive_target(
                    device,
                    nfc_modulation {
                        nmt: nfc_modulation_type::NMT_ISO14443BI,
                        nbr: nfc_baud_rate::NBR_106,
                    },
                    ptr::null(),
                    0,
                    ptr::addr_of_mut!(target),
                )
            },
            1
        );
        assert_eq!(
            unsafe {
                nfc_initiator_select_passive_target(
                    device,
                    nfc_modulation {
                        nmt: nfc_modulation_type::NMT_FELICA,
                        nbr: nfc_baud_rate::NBR_212,
                    },
                    ptr::null(),
                    0,
                    ptr::addr_of_mut!(target),
                )
            },
            1
        );

        assert_eq!(
            snapshot_test_state().passive_init_payloads,
            vec![
                vec![0x00],
                vec![0x01, 0x0b, 0x3f, 0x80],
                vec![0x00, 0xff, 0xff, 0x01, 0x00],
            ]
        );

        unsafe { destroy_device(device) };
    }

    #[test]
    fn iso14443a_select_passive_target_uses_cascade_uid_format() {
        let _guard = initiator_test_guard();
        reset_test_state();

        with_test_state(|state| {
            state.passive_responses.push(PassiveResponse {
                result: 1,
                target: zeroed_target_with_marker(4),
            });
        });

        let device = unsafe { make_device(ptr::addr_of!(TEST_DRIVER_FULL)) };
        let uid = [1u8, 2, 3, 4, 5, 6, 7];
        let mut target = zeroed_target_with_marker(0);

        let result = unsafe {
            nfc_initiator_select_passive_target(
                device,
                nfc_modulation {
                    nmt: nfc_modulation_type::NMT_ISO14443A,
                    nbr: nfc_baud_rate::NBR_106,
                },
                uid.as_ptr(),
                uid.len(),
                ptr::addr_of_mut!(target),
            )
        };

        assert_eq!(result, 1);
        assert_eq!(
            snapshot_test_state().passive_init_payloads,
            vec![vec![0x88, 1, 2, 3, 4, 5, 6, 7]]
        );

        unsafe { destroy_device(device) };
    }

    #[test]
    fn list_passive_targets_stops_on_duplicate_and_restores_infinite_select() {
        let _guard = initiator_test_guard();
        reset_test_state();

        with_test_state(|state| {
            state.passive_responses = vec![
                PassiveResponse {
                    result: 1,
                    target: zeroed_target_with_marker(9),
                },
                PassiveResponse {
                    result: 1,
                    target: zeroed_target_with_marker(9),
                },
            ];
        });

        let device = unsafe { make_device(ptr::addr_of!(TEST_DRIVER_FULL)) };
        unsafe {
            (*device).bInfiniteSelect = true;
        }
        let mut targets = [zeroed_target_with_marker(0), zeroed_target_with_marker(0)];

        let result = unsafe {
            nfc_initiator_list_passive_targets(
                device,
                nfc_modulation {
                    nmt: nfc_modulation_type::NMT_ISO14443A,
                    nbr: nfc_baud_rate::NBR_106,
                },
                targets.as_mut_ptr(),
                targets.len(),
            )
        };

        assert_eq!(result, 1);
        let snapshot = snapshot_test_state();
        assert_eq!(snapshot.passive_calls, 2);
        assert_eq!(snapshot.deselect_calls, 1);
        assert_eq!(
            snapshot.property_bool_calls,
            vec![
                (nfc_property::NP_INFINITE_SELECT, false),
                (nfc_property::NP_INFINITE_SELECT, true),
            ]
        );

        unsafe { destroy_device(device) };
    }

    #[test]
    fn list_passive_targets_single_attempt_modulations_do_not_deselect() {
        let _guard = initiator_test_guard();
        reset_test_state();

        with_test_state(|state| {
            state.passive_responses.push(PassiveResponse {
                result: 1,
                target: zeroed_target_with_marker(7),
            });
        });

        let device = unsafe { make_device(ptr::addr_of!(TEST_DRIVER_FULL)) };
        let mut targets = [zeroed_target_with_marker(0)];

        let result = unsafe {
            nfc_initiator_list_passive_targets(
                device,
                nfc_modulation {
                    nmt: nfc_modulation_type::NMT_FELICA,
                    nbr: nfc_baud_rate::NBR_212,
                },
                targets.as_mut_ptr(),
                targets.len(),
            )
        };

        assert_eq!(result, 1);
        assert_eq!(snapshot_test_state().deselect_calls, 0);

        unsafe { destroy_device(device) };
    }

    #[test]
    fn poll_target_and_target_is_present_dispatch() {
        let _guard = initiator_test_guard();
        reset_test_state();

        with_test_state(|state| {
            state.poll_target_return = 3;
            state.target_is_present_return = 1;
        });

        let device = unsafe { make_device(ptr::addr_of!(TEST_DRIVER_FULL)) };
        let modulations = [nfc_modulation {
            nmt: nfc_modulation_type::NMT_ISO14443A,
            nbr: nfc_baud_rate::NBR_106,
        }];
        let target = zeroed_target_with_marker(3);
        let mut output = zeroed_target_with_marker(0);

        assert_eq!(
            unsafe {
                nfc_initiator_poll_target(
                    device,
                    modulations.as_ptr(),
                    modulations.len(),
                    2,
                    1,
                    ptr::addr_of_mut!(output),
                )
            },
            3
        );
        assert_eq!(
            unsafe { nfc_initiator_target_is_present(device, ptr::addr_of!(target)) },
            1
        );

        let snapshot = snapshot_test_state();
        assert_eq!(snapshot.poll_target_calls, 1);
        assert_eq!(snapshot.target_is_present_calls, 1);

        unsafe { destroy_device(device) };
    }

    #[test]
    fn select_dep_target_preserves_positive_driver_status() {
        let _guard = initiator_test_guard();
        reset_test_state();

        with_test_state(|state| {
            state.select_dep_responses = vec![4];
        });

        let device = unsafe { make_device(ptr::addr_of!(TEST_DRIVER_FULL)) };
        let mut target = zeroed_target_with_marker(0);

        assert_eq!(
            unsafe {
                nfc_initiator_select_dep_target(
                    device,
                    nfc_dep_mode::NDM_PASSIVE,
                    nfc_baud_rate::NBR_106,
                    ptr::null(),
                    ptr::addr_of_mut!(target),
                    123,
                )
            },
            4
        );
        assert_eq!(snapshot_test_state().select_dep_calls, 1);

        unsafe { destroy_device(device) };
    }

    #[test]
    fn transceive_wrappers_dispatch_and_preserve_hal_style_szrx_behavior() {
        let _guard = initiator_test_guard();
        reset_test_state();

        let device = unsafe { make_device(ptr::addr_of!(TEST_DRIVER_FULL)) };
        let tx = [0xa5u8, 0x5a];
        let tx_parity = [0x01u8];
        let mut rx = [0u8; 2];
        let mut rx_bits = [0u8; 1];
        let mut rx_parity = [0u8; 1];
        let mut cycles = 0u32;

        assert_eq!(
            unsafe {
                nfc_initiator_transceive_bytes(
                    device,
                    tx.as_ptr(),
                    tx.len(),
                    rx.as_mut_ptr(),
                    rx.len(),
                    75,
                )
            },
            12
        );
        assert_eq!(
            unsafe {
                nfc_initiator_transceive_bits(
                    device,
                    tx.as_ptr(),
                    7,
                    tx_parity.as_ptr(),
                    rx_bits.as_mut_ptr(),
                    0,
                    rx_parity.as_mut_ptr(),
                )
            },
            13
        );
        assert_eq!(
            unsafe {
                nfc_initiator_transceive_bytes_timed(
                    device,
                    tx.as_ptr(),
                    tx.len(),
                    rx.as_mut_ptr(),
                    rx.len(),
                    ptr::addr_of_mut!(cycles),
                )
            },
            14
        );
        assert_eq!(
            unsafe {
                nfc_initiator_transceive_bits_timed(
                    device,
                    tx.as_ptr(),
                    5,
                    tx_parity.as_ptr(),
                    rx_bits.as_mut_ptr(),
                    0,
                    rx_parity.as_mut_ptr(),
                    ptr::addr_of_mut!(cycles),
                )
            },
            15
        );

        let snapshot = snapshot_test_state();
        assert_eq!(snapshot.initiator_transceive_bytes_calls, vec![(2, 2, 75)]);
        assert_eq!(snapshot.initiator_transceive_bits_calls, vec![7]);
        assert_eq!(
            snapshot.initiator_transceive_bytes_timed_calls,
            vec![(2, 2)]
        );
        assert_eq!(snapshot.initiator_transceive_bits_timed_calls, vec![5]);
        assert_eq!(rx[0], 0x71);
        assert_eq!(rx_bits[0], 0x81);
        assert_eq!(rx_parity[0], 0x02);
        assert_eq!(cycles, 5678);

        unsafe { destroy_device(device) };
    }

    #[test]
    fn poll_dep_target_retries_timeouts_and_restores_infinite_select() {
        let _guard = initiator_test_guard();
        reset_test_state();

        with_test_state(|state| {
            state.select_dep_responses = vec![NFC_ETIMEOUT, NFC_ETIMEOUT, 1];
        });

        let device = unsafe { make_device(ptr::addr_of!(TEST_DRIVER_FULL)) };
        unsafe {
            (*device).bInfiniteSelect = false;
        }
        let mut target = zeroed_target_with_marker(0);

        let result = unsafe {
            nfc_initiator_poll_dep_target(
                device,
                nfc_dep_mode::NDM_PASSIVE,
                nfc_baud_rate::NBR_106,
                ptr::null(),
                ptr::addr_of_mut!(target),
                1000,
            )
        };

        assert_eq!(result, 1);
        let snapshot = snapshot_test_state();
        assert_eq!(snapshot.select_dep_calls, 3);
        assert_eq!(
            snapshot.property_bool_calls,
            vec![
                (nfc_property::NP_INFINITE_SELECT, true),
                (nfc_property::NP_INFINITE_SELECT, false),
            ]
        );

        unsafe { destroy_device(device) };
    }

    #[test]
    fn unsupported_select_callback_preserves_hal_style_behavior() {
        let _guard = initiator_test_guard();
        reset_test_state();

        let device = unsafe { make_device(ptr::addr_of!(TEST_DRIVER_UNSUPPORTED_SELECT)) };
        let mut target = zeroed_target_with_marker(0);
        let result = unsafe {
            nfc_initiator_select_passive_target(
                device,
                nfc_modulation {
                    nmt: nfc_modulation_type::NMT_ISO14443A,
                    nbr: nfc_baud_rate::NBR_106,
                },
                ptr::null(),
                0,
                ptr::addr_of_mut!(target),
            )
        };

        assert_eq!(result, 0);
        assert_eq!(unsafe { (*device).last_error }, NFC_EDEVNOTSUPP);

        unsafe { destroy_device(device) };
    }

    #[test]
    fn missing_transceive_callback_preserves_hal_style_behavior() {
        let _guard = initiator_test_guard();
        reset_test_state();

        let device = unsafe { make_device(ptr::addr_of!(TEST_DRIVER_MISSING_BOOL)) };
        let tx = [0u8; 1];
        let mut rx = [0u8; 1];

        let result = unsafe {
            nfc_initiator_transceive_bytes(
                device,
                tx.as_ptr(),
                tx.len(),
                rx.as_mut_ptr(),
                rx.len(),
                10,
            )
        };

        assert_eq!(result, 0);
        assert_eq!(unsafe { (*device).last_error }, NFC_EDEVNOTSUPP);

        unsafe { destroy_device(device) };
    }

    #[test]
    fn target_and_control_wrappers_dispatch_to_driver() {
        let _guard = initiator_test_guard();
        reset_test_state();

        let device = unsafe { make_device(ptr::addr_of!(TEST_DRIVER_FULL)) };
        let tx = [0x11u8, 0x22];
        let tx_parity = [0x01u8];
        let mut rx = [0u8; 2];
        let mut rx_parity = [0u8; 1];
        let mut info = ptr::null_mut();

        assert_eq!(
            unsafe { nfc_target_send_bytes(device, tx.as_ptr(), tx.len(), 125) },
            16
        );
        assert_eq!(
            unsafe { nfc_target_receive_bytes(device, rx.as_mut_ptr(), rx.len(), 175) },
            17
        );
        assert_eq!(
            unsafe { nfc_target_send_bits(device, tx.as_ptr(), 9, tx_parity.as_ptr()) },
            18
        );
        assert_eq!(
            unsafe {
                nfc_target_receive_bits(device, rx.as_mut_ptr(), rx.len(), rx_parity.as_mut_ptr())
            },
            19
        );
        assert_eq!(
            unsafe { nfc_device_get_information_about(device, ptr::addr_of_mut!(info)) },
            20
        );
        assert_eq!(unsafe { nfc_abort_command(device) }, 21);
        assert_eq!(unsafe { nfc_idle(device) }, 22);

        let snapshot = snapshot_test_state();
        assert_eq!(snapshot.target_send_bytes_calls, vec![(2, 125)]);
        assert_eq!(snapshot.target_receive_bytes_calls, vec![(2, 175)]);
        assert_eq!(snapshot.target_send_bits_calls, vec![9]);
        assert_eq!(snapshot.target_receive_bits_calls, vec![2]);
        assert_eq!(snapshot.device_get_information_about_calls, 1);
        assert_eq!(snapshot.abort_command_calls, 1);
        assert_eq!(snapshot.idle_calls, 1);
        assert_eq!(rx[0], 0xa1);
        assert_eq!(rx_parity[0], 0x03);
        assert_eq!(
            unsafe { CStr::from_ptr(info.cast_const()) }
                .to_str()
                .unwrap(),
            "driver-info"
        );

        unsafe { destroy_device(device) };
    }

    #[test]
    fn accessors_and_error_helpers_match_c_behavior() {
        let _guard = initiator_test_guard();
        reset_test_state();

        let device = unsafe { make_device(ptr::addr_of!(TEST_DRIVER_FULL)) };
        unsafe {
            let name = b"demo-device\0";
            ptr::copy_nonoverlapping(
                name.as_ptr().cast::<c_char>(),
                (*device).name.as_mut_ptr(),
                name.len(),
            );
            (*device).last_error = NFC_EDEVNOTSUPP;
        }

        assert_eq!(
            unsafe { CStr::from_ptr(nfc_device_get_name(device)) }
                .to_str()
                .unwrap(),
            "demo-device"
        );
        assert_eq!(
            unsafe { CStr::from_ptr(nfc_device_get_connstring(device)) }
                .to_str()
                .unwrap(),
            "test-driver"
        );
        assert_eq!(
            unsafe { nfc_device_get_last_error(device) },
            NFC_EDEVNOTSUPP
        );
        assert_eq!(
            unsafe { CStr::from_ptr(nfc_strerror(device)) }
                .to_str()
                .unwrap(),
            "Not Supported by Device"
        );

        unsafe {
            (*device).last_error = -999;
        }
        assert_eq!(
            unsafe { CStr::from_ptr(nfc_strerror(device)) }
                .to_str()
                .unwrap(),
            "Unknown error"
        );

        let mut buffer = [0 as c_char; 8];
        unsafe {
            (*device).last_error = NFC_EDEVNOTSUPP;
        }
        assert_eq!(
            unsafe { nfc_strerror_r(device, buffer.as_mut_ptr(), buffer.len()) },
            0
        );
        assert_eq!(
            unsafe { CStr::from_ptr(buffer.as_ptr()) }.to_str().unwrap(),
            "Not Sup"
        );

        unsafe { destroy_device(device) };
    }

    #[test]
    fn supported_baud_rate_target_mode_dispatches_n_target() {
        let _guard = initiator_test_guard();
        reset_test_state();

        let device = unsafe { make_device(ptr::addr_of!(TEST_DRIVER_FULL)) };
        let mut supported = ptr::null();

        assert_eq!(
            unsafe {
                nfc_device_get_supported_baud_rate(
                    device,
                    nfc_modulation_type::NMT_ISO14443A,
                    ptr::addr_of_mut!(supported),
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                nfc_device_get_supported_baud_rate_target_mode(
                    device,
                    nfc_modulation_type::NMT_FELICA,
                    ptr::addr_of_mut!(supported),
                )
            },
            0
        );

        let snapshot = snapshot_test_state();
        assert_eq!(
            snapshot.get_supported_baud_rate_modes,
            vec![
                (nfc_mode::N_INITIATOR, nfc_modulation_type::NMT_ISO14443A),
                (nfc_mode::N_TARGET, nfc_modulation_type::NMT_FELICA),
            ]
        );

        unsafe { destroy_device(device) };
    }

    #[test]
    fn emulate_target_uses_target_byte_io_loop() {
        let _guard = initiator_test_guard();
        reset_test_state();
        *emulator_state()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = 0;

        let device = unsafe { make_device(ptr::addr_of!(TEST_DRIVER_FULL)) };
        let mut target = zeroed_target_with_marker(0);
        let mut state_machine = nfc_emulation_state_machine {
            io: Some(test_emulator_io),
            data: ptr::null_mut(),
        };
        let mut emulator = nfc_emulator {
            target: ptr::addr_of_mut!(target),
            state_machine: ptr::addr_of_mut!(state_machine),
            user_data: ptr::null_mut(),
        };

        assert_eq!(
            unsafe { nfc_emulate_target(device, ptr::addr_of_mut!(emulator), 250) },
            -1
        );

        let snapshot = snapshot_test_state();
        assert_eq!(
            snapshot.target_init_calls,
            vec![(ISO7816_SHORT_R_APDU_MAX_LEN, 250)]
        );
        assert_eq!(snapshot.target_send_bytes_calls, vec![(2, 250)]);
        assert_eq!(
            snapshot.target_receive_bytes_calls,
            vec![(ISO7816_SHORT_R_APDU_MAX_LEN, 250)]
        );

        unsafe { destroy_device(device) };
    }

    #[test]
    fn context_alloc_defaults_is_still_usable_with_initiator_types_loaded() {
        let _guard = initiator_test_guard();
        reset_test_state();

        let context = unsafe { nfc_context_alloc_defaults() };
        assert!(!context.is_null());
        unsafe {
            crate::lifecycle::nfc_context_free(context);
        }
    }
}
