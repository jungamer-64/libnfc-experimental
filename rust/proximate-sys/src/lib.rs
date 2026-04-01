#[cfg(any(feature = "orchestration", cbindgen))]
/// cbindgen:ignore
mod buses;
/// cbindgen:ignore
mod c_api_impl;
#[cfg(any(feature = "lifecycle", cbindgen))]
/// cbindgen:ignore
mod compat;
#[cfg(any(feature = "lifecycle", cbindgen))]
/// cbindgen:ignore
mod core;
#[cfg(any(feature = "orchestration", cbindgen))]
/// cbindgen:ignore
mod drivers;
/// cbindgen:ignore
mod ffi_support;
#[cfg(any(feature = "lifecycle", cbindgen))]
mod ffi_types;
#[cfg(any(feature = "orchestration", cbindgen))]
/// cbindgen:ignore
mod initiator;
#[cfg(any(feature = "lifecycle", cbindgen))]
mod lifecycle;
#[cfg(any(feature = "lifecycle", feature = "orchestration", cbindgen))]
/// cbindgen:ignore
mod runtime_bridge;
#[cfg(any(feature = "secure", cbindgen))]
/// cbindgen:ignore
mod secure_ffi;
#[cfg(any(feature = "orchestration", feature = "usb_helper", cbindgen))]
mod usbbus;

#[cfg(any(feature = "c_ffi", cbindgen))]
pub use c_api_impl::{
    LOG_GROUP_GENERAL, LOG_PRIORITY_DEBUG, LOG_PRIORITY_ERROR, NFC_BUFSIZE_CONNSTRING,
    NFC_COMMON_ERROR, NFC_COMMON_INVALID, NFC_COMMON_SUCCESS,
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
#[cfg(any(all(feature = "secure", feature = "c_ffi"), cbindgen))]
pub use secure_ffi::{
    NFC_SECURE_ERROR_INVALID, NFC_SECURE_ERROR_OVERFLOW, NFC_SECURE_ERROR_RANGE,
    NFC_SECURE_ERROR_ZERO_SIZE, NFC_SECURE_SUCCESS,
};
#[cfg(any(all(feature = "usb_helper", feature = "c_ffi"), cbindgen))]
pub use usbbus::{
    usb_bulk_endpoints, usb_dev_handle, usb_device, usb_device_list, usb_endpoint_descriptor,
    usb_interface_descriptor,
};

#[cfg(any(test, libnfc_driver_pn71xx))]
pub(crate) use c_api_impl::log_debug;
pub(crate) use c_api_impl::{
    MALLOC_LABEL, emit_log_message, ffi_catch_unwind_int, ffi_catch_unwind_ptr,
    ffi_catch_unwind_void, log_error, log_message, release_allocated_ptr, reset_last_error,
    set_last_error_message,
};
#[cfg(test)]
pub(crate) use c_api_impl::{test_clear_last_log, test_get_last_log, test_get_logs};

#[cfg(any(feature = "c_ffi", cbindgen))]
/// cbindgen:ignore
mod proximate {
    #[cfg(any(feature = "orchestration", cbindgen))]
    pub use crate::buses::i2c::{i2c_close, i2c_list_ports, i2c_open, i2c_read, i2c_write};
    #[cfg(any(feature = "orchestration", cbindgen))]
    pub use crate::buses::spi::{
        spi_close, spi_get_speed, spi_list_ports, spi_open, spi_receive, spi_send,
        spi_send_receive, spi_set_mode, spi_set_speed,
    };
    #[cfg(any(feature = "orchestration", cbindgen))]
    pub use crate::buses::uart::{
        uart_close, uart_flush_input, uart_get_speed, uart_list_ports, uart_open, uart_receive,
        uart_send, uart_set_speed,
    };
    pub use crate::c_api_impl::connstring_decode;
    pub use crate::c_api_impl::{
        nfc_build_connstring, nfc_clear_last_error, nfc_get_last_error, nfc_parse_connstring,
        nfc_rs_free, nfc_set_last_error,
    };
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
    #[cfg(any(feature = "secure", cbindgen))]
    pub use crate::secure_ffi::{
        nfc_ensure_null_terminated, nfc_is_null_terminated, nfc_safe_memcpy, nfc_safe_memmove,
        nfc_safe_strlen, nfc_secure_memset, nfc_secure_strerror, nfc_secure_zero,
    };
    #[cfg(any(feature = "orchestration", feature = "usb_helper", cbindgen))]
    pub use crate::usbbus::{
        usb_bulk_read, usb_bulk_write, usb_claim_interface, usb_close,
        usb_device_get_bulk_endpoints, usb_error_is_access, usb_error_is_timeout,
        usb_free_device_list, usb_get_bus_device_strings, usb_get_device_list,
        usb_get_string_simple, usb_open, usb_prepare, usb_release_interface, usb_reset,
        usb_set_altinterface, usb_set_configuration, usb_strerror,
    };
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_parse_connstring(
    connstring: *const libc::c_char,
    prefix: *const libc::c_char,
    param_name: *const libc::c_char,
    param_value: *mut libc::c_char,
    param_value_size: libc::size_t,
) -> libc::c_int {
    unsafe {
        proximate::nfc_parse_connstring(
            connstring,
            prefix,
            param_name,
            param_value,
            param_value_size,
        )
    }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_build_connstring(
    dest: *mut libc::c_char,
    dest_size: libc::size_t,
    driver_name: *const libc::c_char,
    param_name: *const libc::c_char,
    param_value: *const libc::c_char,
) -> libc::c_int {
    unsafe {
        proximate::nfc_build_connstring(dest, dest_size, driver_name, param_name, param_value)
    }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_set_last_error(message: *const libc::c_char) {
    unsafe { proximate::nfc_set_last_error(message) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_rs_free(ptr: *mut libc::c_void) {
    unsafe { proximate::nfc_rs_free(ptr) }
}

#[cfg(all(feature = "usb_helper", any(feature = "c_ffi", cbindgen)))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn usb_prepare() -> libc::c_int {
    unsafe { proximate::usb_prepare() }
}

#[cfg(all(feature = "usb_helper", any(feature = "c_ffi", cbindgen)))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn usb_get_device_list(list: *mut usb_device_list) -> libc::c_int {
    unsafe { proximate::usb_get_device_list(list) }
}

#[cfg(all(feature = "usb_helper", any(feature = "c_ffi", cbindgen)))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn usb_free_device_list(list: *mut usb_device_list) {
    unsafe { proximate::usb_free_device_list(list) }
}

#[cfg(all(feature = "usb_helper", any(feature = "c_ffi", cbindgen)))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn usb_get_bus_device_strings(
    device: *const usb_device,
    bus_buffer: *mut libc::c_char,
    bus_buffer_size: libc::size_t,
    device_buffer: *mut libc::c_char,
    device_buffer_size: libc::size_t,
) -> libc::c_int {
    unsafe {
        proximate::usb_get_bus_device_strings(
            device,
            bus_buffer,
            bus_buffer_size,
            device_buffer,
            device_buffer_size,
        )
    }
}

#[cfg(all(feature = "usb_helper", any(feature = "c_ffi", cbindgen)))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn usb_device_get_bulk_endpoints(
    device: *const usb_device,
    endpoints: *mut usb_bulk_endpoints,
) -> bool {
    unsafe { proximate::usb_device_get_bulk_endpoints(device, endpoints) }
}

#[cfg(all(feature = "usb_helper", any(feature = "c_ffi", cbindgen)))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn usb_open(
    device: *const usb_device,
    handle: *mut *mut usb_dev_handle,
) -> libc::c_int {
    unsafe { proximate::usb_open(device, handle) }
}

#[cfg(all(feature = "usb_helper", any(feature = "c_ffi", cbindgen)))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn usb_close(handle: *mut usb_dev_handle) -> libc::c_int {
    unsafe { proximate::usb_close(handle) }
}

#[cfg(all(feature = "usb_helper", any(feature = "c_ffi", cbindgen)))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn usb_set_configuration(
    handle: *mut usb_dev_handle,
    configuration_value: libc::c_int,
) -> libc::c_int {
    unsafe { proximate::usb_set_configuration(handle, configuration_value) }
}

#[cfg(all(feature = "usb_helper", any(feature = "c_ffi", cbindgen)))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn usb_claim_interface(
    handle: *mut usb_dev_handle,
    interface_number: libc::c_int,
) -> libc::c_int {
    unsafe { proximate::usb_claim_interface(handle, interface_number) }
}

#[cfg(all(feature = "usb_helper", any(feature = "c_ffi", cbindgen)))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn usb_release_interface(
    handle: *mut usb_dev_handle,
    interface_number: libc::c_int,
) -> libc::c_int {
    unsafe { proximate::usb_release_interface(handle, interface_number) }
}

#[cfg(all(feature = "usb_helper", any(feature = "c_ffi", cbindgen)))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn usb_set_altinterface(
    handle: *mut usb_dev_handle,
    interface_number: libc::c_int,
    alternate_setting: libc::c_int,
) -> libc::c_int {
    unsafe { proximate::usb_set_altinterface(handle, interface_number, alternate_setting) }
}

#[cfg(all(feature = "usb_helper", any(feature = "c_ffi", cbindgen)))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn usb_reset(handle: *mut usb_dev_handle) -> libc::c_int {
    unsafe { proximate::usb_reset(handle) }
}

#[cfg(all(feature = "usb_helper", any(feature = "c_ffi", cbindgen)))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn usb_bulk_read(
    handle: *mut usb_dev_handle,
    endpoint: u8,
    data: *mut u8,
    size: libc::size_t,
    timeout: libc::c_int,
) -> libc::c_int {
    unsafe { proximate::usb_bulk_read(handle, endpoint, data, size, timeout) }
}

#[cfg(all(feature = "usb_helper", any(feature = "c_ffi", cbindgen)))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn usb_bulk_write(
    handle: *mut usb_dev_handle,
    endpoint: u8,
    data: *const u8,
    size: libc::size_t,
    timeout: libc::c_int,
) -> libc::c_int {
    unsafe { proximate::usb_bulk_write(handle, endpoint, data, size, timeout) }
}

#[cfg(all(feature = "usb_helper", any(feature = "c_ffi", cbindgen)))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn usb_get_string_simple(
    handle: *mut usb_dev_handle,
    string_index: libc::c_int,
    buffer: *mut libc::c_char,
    buffer_size: libc::size_t,
) -> libc::c_int {
    unsafe { proximate::usb_get_string_simple(handle, string_index, buffer, buffer_size) }
}

#[cfg(all(feature = "usb_helper", any(feature = "c_ffi", cbindgen)))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn usb_strerror(result: libc::c_int) -> *const libc::c_char {
    unsafe { proximate::usb_strerror(result) }
}

#[cfg(all(feature = "usb_helper", any(feature = "c_ffi", cbindgen)))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn usb_error_is_timeout(result: libc::c_int) -> bool {
    unsafe { proximate::usb_error_is_timeout(result) }
}

#[cfg(all(feature = "usb_helper", any(feature = "c_ffi", cbindgen)))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn usb_error_is_access(result: libc::c_int) -> bool {
    unsafe { proximate::usb_error_is_access(result) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uart_open(port_name: *const libc::c_char) -> *mut libc::c_void {
    unsafe { proximate::uart_open(port_name) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uart_close(port: *mut libc::c_void) {
    unsafe { proximate::uart_close(port) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uart_flush_input(port: *mut libc::c_void, wait: bool) {
    unsafe { proximate::uart_flush_input(port, wait) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uart_set_speed(port: *mut libc::c_void, speed: u32) {
    unsafe { proximate::uart_set_speed(port, speed) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uart_get_speed(port: *mut libc::c_void) -> u32 {
    unsafe { proximate::uart_get_speed(port) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uart_receive(
    port: *mut libc::c_void,
    rx: *mut u8,
    rx_len: libc::size_t,
    abort_p: *mut libc::c_void,
    timeout: libc::c_int,
) -> libc::c_int {
    unsafe { proximate::uart_receive(port, rx, rx_len, abort_p, timeout) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uart_send(
    port: *mut libc::c_void,
    tx: *const u8,
    tx_len: libc::size_t,
    timeout: libc::c_int,
) -> libc::c_int {
    unsafe { proximate::uart_send(port, tx, tx_len, timeout) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn uart_list_ports() -> *mut *mut libc::c_char {
    unsafe { proximate::uart_list_ports() }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spi_open(port_name: *const libc::c_char) -> *mut libc::c_void {
    unsafe { proximate::spi_open(port_name) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spi_close(port: *mut libc::c_void) {
    unsafe { proximate::spi_close(port) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spi_set_speed(port: *mut libc::c_void, speed: u32) {
    unsafe { proximate::spi_set_speed(port, speed) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spi_set_mode(port: *mut libc::c_void, mode: u32) {
    unsafe { proximate::spi_set_mode(port, mode) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spi_get_speed(port: *mut libc::c_void) -> u32 {
    unsafe { proximate::spi_get_speed(port) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spi_receive(
    port: *mut libc::c_void,
    rx: *mut u8,
    rx_len: libc::size_t,
    lsb_first: bool,
) -> libc::c_int {
    unsafe { proximate::spi_receive(port, rx, rx_len, lsb_first) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spi_send(
    port: *mut libc::c_void,
    tx: *const u8,
    tx_len: libc::size_t,
    lsb_first: bool,
) -> libc::c_int {
    unsafe { proximate::spi_send(port, tx, tx_len, lsb_first) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spi_send_receive(
    port: *mut libc::c_void,
    tx: *const u8,
    tx_len: libc::size_t,
    rx: *mut u8,
    rx_len: libc::size_t,
    lsb_first: bool,
) -> libc::c_int {
    unsafe { proximate::spi_send_receive(port, tx, tx_len, rx, rx_len, lsb_first) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spi_list_ports() -> *mut *mut libc::c_char {
    unsafe { proximate::spi_list_ports() }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn i2c_open(
    bus_name: *const libc::c_char,
    address: u32,
) -> *mut libc::c_void {
    unsafe { proximate::i2c_open(bus_name, address) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn i2c_close(device: *mut libc::c_void) {
    unsafe { proximate::i2c_close(device) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn i2c_read(
    device: *mut libc::c_void,
    rx: *mut u8,
    rx_len: libc::size_t,
) -> libc::ssize_t {
    unsafe { proximate::i2c_read(device, rx, rx_len) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn i2c_write(
    device: *mut libc::c_void,
    tx: *const u8,
    tx_len: libc::size_t,
) -> libc::c_int {
    unsafe { proximate::i2c_write(device, tx, tx_len) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn i2c_list_ports() -> *mut *mut libc::c_char {
    unsafe { proximate::i2c_list_ports() }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connstring_decode(
    connstring: *const libc::c_char,
    driver_name: *const libc::c_char,
    bus_name: *const libc::c_char,
    pparam1: *mut *mut libc::c_char,
    pparam2: *mut *mut libc::c_char,
) -> libc::c_int {
    unsafe { proximate::connstring_decode(connstring, driver_name, bus_name, pparam1, pparam2) }
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

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_safe_memcpy(
    dst: *mut libc::c_void,
    dst_size: libc::size_t,
    src: *const libc::c_void,
    src_size: libc::size_t,
) -> libc::c_int {
    unsafe { proximate::nfc_safe_memcpy(dst, dst_size, src, src_size) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_safe_memmove(
    dst: *mut libc::c_void,
    dst_size: libc::size_t,
    src: *const libc::c_void,
    src_size: libc::size_t,
) -> libc::c_int {
    unsafe { proximate::nfc_safe_memmove(dst, dst_size, src, src_size) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_secure_memset(
    ptr: *mut libc::c_void,
    val: libc::c_int,
    size: libc::size_t,
) -> libc::c_int {
    unsafe { proximate::nfc_secure_memset(ptr, val, size) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_secure_zero(
    ptr: *mut libc::c_void,
    size: libc::size_t,
) -> libc::c_int {
    unsafe { proximate::nfc_secure_zero(ptr, size) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_safe_strlen(
    str: *const libc::c_char,
    maxlen: libc::size_t,
) -> libc::size_t {
    unsafe { proximate::nfc_safe_strlen(str, maxlen) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_is_null_terminated(
    buf: *const libc::c_char,
    bufsize: libc::size_t,
) -> libc::c_int {
    unsafe { proximate::nfc_is_null_terminated(buf, bufsize) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_ensure_null_terminated(buf: *mut libc::c_char, bufsize: libc::size_t) {
    unsafe { proximate::nfc_ensure_null_terminated(buf, bufsize) }
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub extern "C" fn nfc_get_last_error() -> *const libc::c_char {
    proximate::nfc_get_last_error()
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub extern "C" fn nfc_clear_last_error() {
    proximate::nfc_clear_last_error()
}

#[cfg(any(feature = "c_ffi", cbindgen))]
#[unsafe(no_mangle)]
pub extern "C" fn nfc_secure_strerror(code: libc::c_int) -> *const libc::c_char {
    proximate::nfc_secure_strerror(code)
}
