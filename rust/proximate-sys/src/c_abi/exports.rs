use crate::c_abi::types::{
    nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_mode, nfc_modulation, nfc_modulation_type,
    nfc_property, nfc_target,
};
use crate::lifecycle::{nfc_connstring, nfc_context, nfc_device, nfc_driver};

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_register_driver(driver: *const nfc_driver) -> libc::c_int {
    unsafe { crate::core::driver_registration::nfc_register_driver(driver) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_open(
    context: *mut nfc_context,
    connstring: *const libc::c_char,
) -> *mut nfc_device {
    unsafe { crate::core::runtime::nfc_open(context, connstring) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_list_devices(
    context: *mut nfc_context,
    connstrings: *mut nfc_connstring,
    connstrings_len: libc::size_t,
) -> libc::size_t {
    unsafe { crate::core::runtime::nfc_list_devices(context, connstrings, connstrings_len) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_init(context: *mut *mut nfc_context) {
    unsafe { crate::core::context::nfc_init(context) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_exit(context: *mut nfc_context) {
    unsafe { crate::core::context::nfc_exit(context) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_close(device: *mut nfc_device) {
    unsafe { crate::c_abi::misc_exports::nfc_close(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_free(ptr: *mut libc::c_void) {
    unsafe { crate::c_abi::misc_exports::nfc_free(ptr) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_version() -> *const libc::c_char {
    unsafe { crate::c_abi::misc_exports::nfc_version() }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn str_nfc_baud_rate(value: nfc_baud_rate) -> *const libc::c_char {
    unsafe { crate::c_abi::misc_exports::str_nfc_baud_rate(value) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn str_nfc_modulation_type(
    value: nfc_modulation_type,
) -> *const libc::c_char {
    unsafe { crate::c_abi::misc_exports::str_nfc_modulation_type(value) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn str_nfc_target(
    buf: *mut *mut libc::c_char,
    target: *const nfc_target,
    verbose: bool,
) -> libc::c_int {
    unsafe { crate::c_abi::misc_exports::str_nfc_target(buf, target, verbose) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_set_property_int(
    device: *mut nfc_device,
    property: nfc_property,
    value: libc::c_int,
) -> libc::c_int {
    unsafe { crate::initiator::operations::nfc_device_set_property_int(device, property, value) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_set_property_bool(
    device: *mut nfc_device,
    property: nfc_property,
    enable: bool,
) -> libc::c_int {
    unsafe { crate::initiator::operations::nfc_device_set_property_bool(device, property, enable) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_init(device: *mut nfc_device) -> libc::c_int {
    unsafe { crate::initiator::operations::nfc_initiator_init(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_init_secure_element(device: *mut nfc_device) -> libc::c_int {
    unsafe { crate::initiator::operations::nfc_initiator_init_secure_element(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_select_passive_target(
    device: *mut nfc_device,
    nm: nfc_modulation,
    init_data: *const u8,
    init_data_len: libc::size_t,
    target: *mut nfc_target,
) -> libc::c_int {
    unsafe {
        crate::initiator::operations::nfc_initiator_select_passive_target(
            device,
            nm,
            init_data,
            init_data_len,
            target,
        )
    }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_list_passive_targets(
    device: *mut nfc_device,
    nm: nfc_modulation,
    targets: *mut nfc_target,
    targets_len: libc::size_t,
) -> libc::c_int {
    unsafe {
        crate::initiator::operations::nfc_initiator_list_passive_targets(
            device,
            nm,
            targets,
            targets_len,
        )
    }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
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
        crate::initiator::operations::nfc_initiator_poll_target(
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
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
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
        crate::initiator::operations::nfc_initiator_select_dep_target(
            device, dep_mode, baud_rate, initiator, target, timeout,
        )
    }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
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
        crate::initiator::operations::nfc_initiator_poll_dep_target(
            device, dep_mode, baud_rate, initiator, target, timeout,
        )
    }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_deselect_target(device: *mut nfc_device) -> libc::c_int {
    unsafe { crate::initiator::operations::nfc_initiator_deselect_target(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_target_is_present(
    device: *mut nfc_device,
    target: *const nfc_target,
) -> libc::c_int {
    unsafe { crate::initiator::operations::nfc_initiator_target_is_present(device, target) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_target_init(
    device: *mut nfc_device,
    target: *mut nfc_target,
    rx: *mut u8,
    rx_len: libc::size_t,
    timeout: libc::c_int,
) -> libc::c_int {
    unsafe { crate::initiator::operations::nfc_target_init(device, target, rx, rx_len, timeout) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_transceive_bytes(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: libc::size_t,
    rx: *mut u8,
    rx_len: libc::size_t,
    timeout: libc::c_int,
) -> libc::c_int {
    unsafe {
        crate::initiator::operations::nfc_initiator_transceive_bytes(
            device, tx, tx_len, rx, rx_len, timeout,
        )
    }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
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
        crate::initiator::operations::nfc_initiator_transceive_bits(
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
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
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
        crate::initiator::operations::nfc_initiator_transceive_bytes_timed(
            device, tx, tx_len, rx, rx_len, cycles,
        )
    }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
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
        crate::initiator::operations::nfc_initiator_transceive_bits_timed(
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
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_target_send_bytes(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: libc::size_t,
    timeout: libc::c_int,
) -> libc::c_int {
    unsafe { crate::initiator::operations::nfc_target_send_bytes(device, tx, tx_len, timeout) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_target_receive_bytes(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: libc::size_t,
    timeout: libc::c_int,
) -> libc::c_int {
    unsafe { crate::initiator::operations::nfc_target_receive_bytes(device, rx, rx_len, timeout) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_target_send_bits(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: libc::size_t,
    tx_parity: *const u8,
) -> libc::c_int {
    unsafe {
        crate::initiator::operations::nfc_target_send_bits(device, tx, tx_bits_len, tx_parity)
    }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_target_receive_bits(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: libc::size_t,
    rx_parity: *mut u8,
) -> libc::c_int {
    unsafe { crate::initiator::operations::nfc_target_receive_bits(device, rx, rx_len, rx_parity) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_emulate_target(
    device: *mut nfc_device,
    emulator: *mut libc::c_void,
    timeout: libc::c_int,
) -> libc::c_int {
    unsafe { crate::initiator::emulation::nfc_emulate_target(device, emulator.cast(), timeout) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iso14443a_crc(data: *mut u8, len: libc::size_t, crc: *mut u8) {
    unsafe { crate::c_abi::misc_exports::iso14443a_crc(data, len, crc) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iso14443a_crc_append(data: *mut u8, len: libc::size_t) {
    unsafe { crate::c_abi::misc_exports::iso14443a_crc_append(data, len) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iso14443b_crc(data: *mut u8, len: libc::size_t, crc: *mut u8) {
    unsafe { crate::c_abi::misc_exports::iso14443b_crc(data, len, crc) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iso14443b_crc_append(data: *mut u8, len: libc::size_t) {
    unsafe { crate::c_abi::misc_exports::iso14443b_crc_append(data, len) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iso14443a_locate_historical_bytes(
    ats: *mut u8,
    ats_len: libc::size_t,
    tk_len: *mut libc::size_t,
) -> *mut u8 {
    unsafe { crate::c_abi::misc_exports::iso14443a_locate_historical_bytes(ats, ats_len, tk_len) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_abort_command(device: *mut nfc_device) -> libc::c_int {
    unsafe { crate::initiator::operations::nfc_abort_command(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_idle(device: *mut nfc_device) -> libc::c_int {
    unsafe { crate::initiator::operations::nfc_idle(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_name(device: *mut nfc_device) -> *const libc::c_char {
    unsafe { crate::initiator::accessors::nfc_device_get_name(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_connstring(device: *mut nfc_device) -> *const libc::c_char {
    unsafe { crate::initiator::accessors::nfc_device_get_connstring(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_supported_modulation(
    device: *mut nfc_device,
    mode: nfc_mode,
    supported: *mut *const nfc_modulation_type,
) -> libc::c_int {
    unsafe {
        crate::initiator::accessors::nfc_device_get_supported_modulation(device, mode, supported)
    }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_supported_baud_rate(
    device: *mut nfc_device,
    modulation_type: nfc_modulation_type,
    supported: *mut *const nfc_baud_rate,
) -> libc::c_int {
    unsafe {
        crate::initiator::accessors::nfc_device_get_supported_baud_rate(
            device,
            modulation_type,
            supported,
        )
    }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_supported_baud_rate_target_mode(
    device: *mut nfc_device,
    modulation_type: nfc_modulation_type,
    supported: *mut *const nfc_baud_rate,
) -> libc::c_int {
    unsafe {
        crate::initiator::accessors::nfc_device_get_supported_baud_rate_target_mode(
            device,
            modulation_type,
            supported,
        )
    }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_information_about(
    device: *mut nfc_device,
    buf: *mut *mut libc::c_char,
) -> libc::c_int {
    unsafe { crate::initiator::accessors::nfc_device_get_information_about(device, buf) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_last_error(device: *const nfc_device) -> libc::c_int {
    unsafe { crate::initiator::accessors::nfc_device_get_last_error(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_strerror(device: *const nfc_device) -> *const libc::c_char {
    unsafe { crate::initiator::accessors::nfc_strerror(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_strerror_r(
    device: *const nfc_device,
    buf: *mut libc::c_char,
    buflen: libc::size_t,
) -> libc::c_int {
    unsafe { crate::initiator::accessors::nfc_strerror_r(device, buf, buflen) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
/// # Safety
/// The caller must uphold the libnfc C ABI requirements for all pointers, lengths, and output buffers passed to this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_perror(device: *const nfc_device, message: *const libc::c_char) {
    unsafe { crate::initiator::accessors::nfc_perror(device, message) }
}
