/// cbindgen:ignore
mod c_api_impl;
#[cfg(any(feature = "lifecycle", cbindgen))]
/// cbindgen:ignore
mod compat;
#[cfg(any(feature = "lifecycle", cbindgen))]
/// cbindgen:ignore
mod core;
/// cbindgen:ignore
mod ffi_strings;
/// cbindgen:ignore
mod ffi_support;
#[cfg(any(feature = "lifecycle", cbindgen))]
mod ffi_types;
#[cfg(any(feature = "orchestration", cbindgen))]
mod initiator;
#[cfg(any(feature = "lifecycle", cbindgen))]
mod lifecycle;
/// cbindgen:ignore
mod logger;
#[cfg(cbindgen)]
mod private_ffi;
#[cfg(any(feature = "lifecycle", feature = "orchestration", cbindgen))]
/// cbindgen:ignore
mod runtime_bridge;
#[cfg(any(feature = "c_ffi", cbindgen))]
pub use c_api_impl::{
    LOG_GROUP_GENERAL, LOG_PRIORITY_DEBUG, LOG_PRIORITY_ERROR, NFC_BUFSIZE_CONNSTRING,
    NFC_COMMON_ERROR, NFC_COMMON_INVALID, NFC_COMMON_SUCCESS,
};
pub(crate) use c_api_impl::{
    MALLOC_LABEL, emit_log_message, ffi_catch_unwind_int, ffi_catch_unwind_ptr,
    ffi_catch_unwind_void, log_error, log_message, release_allocated_ptr, reset_last_error,
    set_last_error_message,
};
#[cfg(any(feature = "c_ffi", cbindgen))]
pub use ffi_types::{
    nfc_barcode_info, nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_felica_info,
    nfc_iso14443a_info, nfc_iso14443b_info, nfc_iso14443b2ct_info, nfc_iso14443b2sr_info,
    nfc_iso14443bi_info, nfc_iso14443biclass_info, nfc_jewel_info, nfc_mode, nfc_modulation,
    nfc_modulation_type, nfc_property, nfc_target, nfc_target_info,
};
#[cfg(any(feature = "c_ffi", cbindgen))]
pub use lifecycle::{
    DEVICE_NAME_LENGTH, MAX_USER_DEFINED_DEVICES, NFC_DRIVER_NAME_MAX, nfc_context, nfc_device,
    nfc_driver, nfc_user_defined_device, scan_type_enum,
};
#[cfg(test)]
pub(crate) use logger::{
    test_clear_rendered_logs as test_clear_last_log, test_get_last_log, test_get_logs,
    test_reset_log_level,
};

#[cfg(any(feature = "c_ffi", cbindgen))]
/// cbindgen:ignore
mod proximate {
    pub use crate::c_api_impl::{nfc_rs_free, nfc_set_last_error};
    #[cfg(any(feature = "lifecycle", cbindgen))]
    pub use crate::compat::{
        nfc_close, nfc_free, nfc_version, str_nfc_baud_rate, str_nfc_modulation_type,
        str_nfc_target,
    };
    #[cfg(any(feature = "orchestration", cbindgen))]
    pub use crate::core::{nfc_exit, nfc_init, nfc_list_devices, nfc_open, nfc_register_driver};
    #[cfg(any(feature = "orchestration", cbindgen))]
    pub use crate::initiator::{
        nfc_abort_command, nfc_device_get_connstring, nfc_device_get_information_about,
        nfc_device_get_last_error, nfc_device_get_name, nfc_device_get_supported_baud_rate,
        nfc_device_get_supported_baud_rate_target_mode, nfc_device_get_supported_modulation,
        nfc_device_set_property_bool, nfc_device_set_property_int, nfc_idle,
        nfc_initiator_deselect_target, nfc_initiator_init, nfc_initiator_init_secure_element,
        nfc_initiator_list_passive_targets, nfc_initiator_poll_dep_target,
        nfc_initiator_poll_target, nfc_initiator_select_dep_target,
        nfc_initiator_select_passive_target, nfc_initiator_target_is_present,
        nfc_initiator_transceive_bits, nfc_initiator_transceive_bits_timed,
        nfc_initiator_transceive_bytes, nfc_initiator_transceive_bytes_timed, nfc_perror,
        nfc_strerror, nfc_strerror_r, nfc_target_init, nfc_target_receive_bits,
        nfc_target_receive_bytes, nfc_target_send_bits, nfc_target_send_bytes,
    };
    #[cfg(any(feature = "lifecycle", cbindgen))]
    pub use crate::lifecycle::{
        nfc_connstring, nfc_context_alloc_defaults, nfc_context_free, nfc_context_new,
        nfc_device_free, nfc_device_new,
    };
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_set_last_error(message: *const libc::c_char) {
    unsafe { proximate::nfc_set_last_error(message) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_rs_log_message(
    group: u8,
    category: *const libc::c_char,
    priority: u8,
    message: *const libc::c_char,
) {
    unsafe { c_api_impl::nfc_rs_log_message(group, category, priority, message) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_rs_free(ptr: *mut libc::c_void) {
    unsafe { proximate::nfc_rs_free(ptr) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_context_alloc_defaults() -> *mut nfc_context {
    unsafe { proximate::nfc_context_alloc_defaults() }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_context_new() -> *mut nfc_context {
    unsafe { proximate::nfc_context_new() }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_new(
    context: *const nfc_context,
    connstring: *const libc::c_char,
) -> *mut nfc_device {
    unsafe { proximate::nfc_device_new(context, connstring) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_free(device: *mut nfc_device) {
    unsafe { proximate::nfc_device_free(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_context_free(context: *mut nfc_context) {
    unsafe { proximate::nfc_context_free(context) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_register_driver(driver: *const nfc_driver) -> libc::c_int {
    unsafe { proximate::nfc_register_driver(driver) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_open(
    context: *mut nfc_context,
    connstring: *const libc::c_char,
) -> *mut nfc_device {
    unsafe { proximate::nfc_open(context, connstring) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_list_devices(
    context: *mut nfc_context,
    connstrings: *mut proximate::nfc_connstring,
    connstrings_len: libc::size_t,
) -> libc::size_t {
    unsafe { proximate::nfc_list_devices(context, connstrings, connstrings_len) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_init(context: *mut *mut nfc_context) {
    unsafe { proximate::nfc_init(context) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_exit(context: *mut nfc_context) {
    unsafe { proximate::nfc_exit(context) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_close(device: *mut nfc_device) {
    unsafe { proximate::nfc_close(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_free(ptr: *mut libc::c_void) {
    unsafe { proximate::nfc_free(ptr) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_version() -> *const libc::c_char {
    unsafe { proximate::nfc_version() }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn str_nfc_baud_rate(value: nfc_baud_rate) -> *const libc::c_char {
    unsafe { proximate::str_nfc_baud_rate(value) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn str_nfc_modulation_type(
    value: nfc_modulation_type,
) -> *const libc::c_char {
    unsafe { proximate::str_nfc_modulation_type(value) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn str_nfc_target(
    buf: *mut *mut libc::c_char,
    target: *const nfc_target,
    verbose: bool,
) -> libc::c_int {
    unsafe { proximate::str_nfc_target(buf, target, verbose) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_set_property_int(
    device: *mut nfc_device,
    property: nfc_property,
    value: libc::c_int,
) -> libc::c_int {
    unsafe { proximate::nfc_device_set_property_int(device, property, value) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_set_property_bool(
    device: *mut nfc_device,
    property: nfc_property,
    enable: bool,
) -> libc::c_int {
    unsafe { proximate::nfc_device_set_property_bool(device, property, enable) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_init(device: *mut nfc_device) -> libc::c_int {
    unsafe { proximate::nfc_initiator_init(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_init_secure_element(device: *mut nfc_device) -> libc::c_int {
    unsafe { proximate::nfc_initiator_init_secure_element(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_select_passive_target(
    device: *mut nfc_device,
    nm: nfc_modulation,
    init_data: *const u8,
    init_data_len: libc::size_t,
    target: *mut nfc_target,
) -> libc::c_int {
    unsafe {
        proximate::nfc_initiator_select_passive_target(device, nm, init_data, init_data_len, target)
    }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_list_passive_targets(
    device: *mut nfc_device,
    nm: nfc_modulation,
    targets: *mut nfc_target,
    targets_len: libc::size_t,
) -> libc::c_int {
    unsafe { proximate::nfc_initiator_list_passive_targets(device, nm, targets, targets_len) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_poll_target(
    device: *mut nfc_device,
    modulation_types: *const nfc_modulation,
    modulation_types_len: libc::size_t,
    poll_nr: u8,
    period: u8,
    target: *mut nfc_target,
) -> libc::c_int {
    unsafe {
        proximate::nfc_initiator_poll_target(
            device,
            modulation_types,
            modulation_types_len,
            poll_nr,
            period,
            target,
        )
    }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_select_dep_target(
    device: *mut nfc_device,
    dep_mode: nfc_dep_mode,
    baud_rate: nfc_baud_rate,
    initiator: *const nfc_dep_info,
    target: *mut nfc_target,
    timeout: libc::c_int,
) -> libc::c_int {
    unsafe {
        proximate::nfc_initiator_select_dep_target(
            device, dep_mode, baud_rate, initiator, target, timeout,
        )
    }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_poll_dep_target(
    device: *mut nfc_device,
    dep_mode: nfc_dep_mode,
    baud_rate: nfc_baud_rate,
    initiator: *const nfc_dep_info,
    target: *mut nfc_target,
    timeout: libc::c_int,
) -> libc::c_int {
    unsafe {
        proximate::nfc_initiator_poll_dep_target(
            device, dep_mode, baud_rate, initiator, target, timeout,
        )
    }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_deselect_target(device: *mut nfc_device) -> libc::c_int {
    unsafe { proximate::nfc_initiator_deselect_target(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_target_is_present(
    device: *mut nfc_device,
    target: *const nfc_target,
) -> libc::c_int {
    unsafe { proximate::nfc_initiator_target_is_present(device, target) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_target_init(
    device: *mut nfc_device,
    target: *mut nfc_target,
    rx: *mut u8,
    rx_len: libc::size_t,
    timeout: libc::c_int,
) -> libc::c_int {
    unsafe { proximate::nfc_target_init(device, target, rx, rx_len, timeout) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_transceive_bytes(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: libc::size_t,
    rx: *mut u8,
    rx_len: libc::size_t,
    timeout: libc::c_int,
) -> libc::c_int {
    unsafe { proximate::nfc_initiator_transceive_bytes(device, tx, tx_len, rx, rx_len, timeout) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_transceive_bits(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: libc::size_t,
    tx_parity: *const u8,
    rx: *mut u8,
    rx_len: libc::size_t,
    rx_parity: *mut u8,
) -> libc::c_int {
    unsafe {
        proximate::nfc_initiator_transceive_bits(
            device,
            tx,
            tx_bits_len,
            tx_parity,
            rx,
            rx_len,
            rx_parity,
        )
    }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_transceive_bytes_timed(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: libc::size_t,
    rx: *mut u8,
    rx_len: libc::size_t,
    cycles: *mut u32,
) -> libc::c_int {
    unsafe {
        proximate::nfc_initiator_transceive_bytes_timed(device, tx, tx_len, rx, rx_len, cycles)
    }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_transceive_bits_timed(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: libc::size_t,
    tx_parity: *const u8,
    rx: *mut u8,
    rx_len: libc::size_t,
    rx_parity: *mut u8,
    cycles: *mut u32,
) -> libc::c_int {
    unsafe {
        proximate::nfc_initiator_transceive_bits_timed(
            device,
            tx,
            tx_bits_len,
            tx_parity,
            rx,
            rx_len,
            rx_parity,
            cycles,
        )
    }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_target_send_bytes(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: libc::size_t,
    timeout: libc::c_int,
) -> libc::c_int {
    unsafe { proximate::nfc_target_send_bytes(device, tx, tx_len, timeout) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_target_receive_bytes(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: libc::size_t,
    timeout: libc::c_int,
) -> libc::c_int {
    unsafe { proximate::nfc_target_receive_bytes(device, rx, rx_len, timeout) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_target_send_bits(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: libc::size_t,
    tx_parity: *const u8,
) -> libc::c_int {
    unsafe { proximate::nfc_target_send_bits(device, tx, tx_bits_len, tx_parity) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_target_receive_bits(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: libc::size_t,
    rx_parity: *mut u8,
) -> libc::c_int {
    unsafe { proximate::nfc_target_receive_bits(device, rx, rx_len, rx_parity) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_emulate_target(
    device: *mut nfc_device,
    emulator: *mut libc::c_void,
    timeout: libc::c_int,
) -> libc::c_int {
    unsafe { crate::initiator::nfc_emulate_target(device, emulator.cast(), timeout) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iso14443a_crc(data: *mut u8, len: libc::size_t, crc: *mut u8) {
    unsafe { crate::compat::iso14443a_crc(data, len, crc) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iso14443a_crc_append(data: *mut u8, len: libc::size_t) {
    unsafe { crate::compat::iso14443a_crc_append(data, len) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iso14443b_crc(data: *mut u8, len: libc::size_t, crc: *mut u8) {
    unsafe { crate::compat::iso14443b_crc(data, len, crc) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iso14443b_crc_append(data: *mut u8, len: libc::size_t) {
    unsafe { crate::compat::iso14443b_crc_append(data, len) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iso14443a_locate_historical_bytes(
    ats: *mut u8,
    ats_len: libc::size_t,
    tk_len: *mut libc::size_t,
) -> *mut u8 {
    unsafe { crate::compat::iso14443a_locate_historical_bytes(ats, ats_len, tk_len) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pn53x_transceive(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: libc::size_t,
    rx: *mut u8,
    rx_len: libc::size_t,
    timeout: libc::c_int,
) -> libc::c_int {
    ffi_catch_unwind_int("pn53x_transceive", -80, || unsafe {
        if device.is_null() || tx.is_null() || tx_len == 0 || (rx.is_null() && rx_len != 0) {
            crate::runtime_bridge::set_device_last_error(device, -2);
            return -2;
        }

        let tx = std::slice::from_raw_parts(tx, tx_len);
        let rx = if rx_len == 0 {
            &mut []
        } else {
            std::slice::from_raw_parts_mut(rx, rx_len)
        };
        let mut borrowed = crate::runtime_bridge::borrowed_device(device);
        match borrowed.pn53x_transceive(tx, rx, timeout) {
            Ok(count) => count as libc::c_int,
            Err(error) => crate::runtime_bridge::error_to_status(&error),
        }
    })
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pn53x_read_register(
    device: *mut nfc_device,
    register: u16,
    value: *mut u8,
) -> libc::c_int {
    ffi_catch_unwind_int("pn53x_read_register", -80, || unsafe {
        if device.is_null() || value.is_null() {
            crate::runtime_bridge::set_device_last_error(device, -2);
            return -2;
        }

        let mut borrowed = crate::runtime_bridge::borrowed_device(device);
        match borrowed.pn53x_read_register(register) {
            Ok(read_value) => {
                *value = read_value;
                0
            }
            Err(error) => crate::runtime_bridge::error_to_status(&error),
        }
    })
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pn53x_write_register(
    device: *mut nfc_device,
    register: u16,
    symbol_mask: u8,
    value: u8,
) -> libc::c_int {
    ffi_catch_unwind_int("pn53x_write_register", -80, || {
        if device.is_null() {
            crate::runtime_bridge::set_device_last_error(device, -2);
            return -2;
        }

        let mut borrowed = crate::runtime_bridge::borrowed_device(device);
        match borrowed.pn53x_write_register(register, symbol_mask, value) {
            Ok(()) => 0,
            Err(error) => crate::runtime_bridge::error_to_status(&error),
        }
    })
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pn532_SAMConfiguration(
    device: *mut nfc_device,
    mode: libc::c_int,
    timeout: libc::c_int,
) -> libc::c_int {
    ffi_catch_unwind_int("pn532_SAMConfiguration", -80, || {
        if device.is_null() {
            crate::runtime_bridge::set_device_last_error(device, -2);
            return -2;
        }

        let mut borrowed = crate::runtime_bridge::borrowed_device(device);
        match borrowed.pn532_sam_configuration(mode as u8, timeout) {
            Ok(status) => status,
            Err(error) => crate::runtime_bridge::error_to_status(&error),
        }
    })
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_abort_command(device: *mut nfc_device) -> libc::c_int {
    unsafe { proximate::nfc_abort_command(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_idle(device: *mut nfc_device) -> libc::c_int {
    unsafe { proximate::nfc_idle(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_name(device: *mut nfc_device) -> *const libc::c_char {
    unsafe { proximate::nfc_device_get_name(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_connstring(device: *mut nfc_device) -> *const libc::c_char {
    unsafe { proximate::nfc_device_get_connstring(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_supported_modulation(
    device: *mut nfc_device,
    mode: nfc_mode,
    supported: *mut *const nfc_modulation_type,
) -> libc::c_int {
    unsafe { proximate::nfc_device_get_supported_modulation(device, mode, supported) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_supported_baud_rate(
    device: *mut nfc_device,
    modulation_type: nfc_modulation_type,
    supported: *mut *const nfc_baud_rate,
) -> libc::c_int {
    unsafe { proximate::nfc_device_get_supported_baud_rate(device, modulation_type, supported) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_supported_baud_rate_target_mode(
    device: *mut nfc_device,
    modulation_type: nfc_modulation_type,
    supported: *mut *const nfc_baud_rate,
) -> libc::c_int {
    unsafe {
        proximate::nfc_device_get_supported_baud_rate_target_mode(
            device,
            modulation_type,
            supported,
        )
    }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_information_about(
    device: *mut nfc_device,
    buf: *mut *mut libc::c_char,
) -> libc::c_int {
    unsafe { proximate::nfc_device_get_information_about(device, buf) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_last_error(device: *const nfc_device) -> libc::c_int {
    unsafe { proximate::nfc_device_get_last_error(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_strerror(device: *const nfc_device) -> *const libc::c_char {
    unsafe { proximate::nfc_strerror(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_strerror_r(
    device: *const nfc_device,
    buf: *mut libc::c_char,
    buflen: libc::size_t,
) -> libc::c_int {
    unsafe { proximate::nfc_strerror_r(device, buf, buflen) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_perror(device: *const nfc_device, message: *const libc::c_char) {
    unsafe { proximate::nfc_perror(device, message) }
}
