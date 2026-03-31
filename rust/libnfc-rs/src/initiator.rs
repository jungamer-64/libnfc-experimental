// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Ported from libnfc/nfc.c.

use crate::ffi_support::{as_mut, as_ref, c_string_ptr_to_string};
use crate::ffi_types::{
    nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_mode, nfc_modulation, nfc_modulation_type,
    nfc_property, nfc_target,
};
use crate::lifecycle::nfc_device;
use crate::{
    LOG_GROUP_GENERAL, LOG_PRIORITY_DEBUG, LOG_PRIORITY_ERROR, emit_log_message,
    ffi_catch_unwind_int, ffi_catch_unwind_ptr, ffi_catch_unwind_void,
};
use libc::{c_char, c_int, size_t};
use std::ffi::CString;
use std::mem::size_of;
use std::ptr;
use std::slice;

const NFC_SUCCESS: c_int = 0;
const NFC_EINVARG: c_int = -2;
const NFC_EDEVNOTSUPP: c_int = -3;
const NFC_ENOTSUCHDEV: c_int = -4;
const NFC_EOVFLOW: c_int = -5;
const NFC_ETIMEOUT: c_int = -6;
const NFC_EOPABORTED: c_int = -7;
const NFC_ENOTIMPL: c_int = -8;
const NFC_ETGRELEASED: c_int = -10;
const NFC_ERFTRANS: c_int = -20;
const NFC_EMFCAUTHFAIL: c_int = -30;
const NFC_ESOFT: c_int = -80;
const NFC_ECHIP: c_int = -90;

const GENERAL_LOG_CATEGORY: *const c_char = b"libnfc.general\0" as *const u8 as *const c_char;
const SUCCESS_MESSAGE: *const c_char = b"Success\0" as *const u8 as *const c_char;
const IO_ERROR_MESSAGE: *const c_char = b"Input / Output Error\0" as *const u8 as *const c_char;
const INVALID_ARGUMENT_MESSAGE: *const c_char =
    b"Invalid argument(s)\0" as *const u8 as *const c_char;
const DEVICE_NOT_SUPPORTED_MESSAGE: *const c_char =
    b"Not Supported by Device\0" as *const u8 as *const c_char;
const NO_SUCH_DEVICE_MESSAGE: *const c_char = b"No Such Device\0" as *const u8 as *const c_char;
const BUFFER_OVERFLOW_MESSAGE: *const c_char = b"Buffer Overflow\0" as *const u8 as *const c_char;
const TIMEOUT_MESSAGE: *const c_char = b"Timeout\0" as *const u8 as *const c_char;
const OPERATION_ABORTED_MESSAGE: *const c_char =
    b"Operation Aborted\0" as *const u8 as *const c_char;
const NOT_IMPLEMENTED_MESSAGE: *const c_char =
    b"Not (yet) Implemented\0" as *const u8 as *const c_char;
const TARGET_RELEASED_MESSAGE: *const c_char = b"Target Released\0" as *const u8 as *const c_char;
const MIFARE_AUTH_FAILED_MESSAGE: *const c_char =
    b"Mifare Authentication Failed\0" as *const u8 as *const c_char;
const RF_TRANSMISSION_ERROR_MESSAGE: *const c_char =
    b"RF Transmission Error\0" as *const u8 as *const c_char;
const CHIP_ERROR_MESSAGE: *const c_char =
    b"Device's Internal Chip Error\0" as *const u8 as *const c_char;
const UNKNOWN_ERROR_MESSAGE: *const c_char = b"Unknown error\0" as *const u8 as *const c_char;
const NULL_ERROR_PREFIX: *const c_char = b"(null)\0" as *const u8 as *const c_char;
const PRINTF_STRING_FORMAT: *const c_char = b"%s\0" as *const u8 as *const c_char;

const PROPERTY_NAMES: [&str; 15] = [
    "NP_TIMEOUT_COMMAND",
    "NP_TIMEOUT_ATR",
    "NP_TIMEOUT_COM",
    "NP_HANDLE_CRC",
    "NP_HANDLE_PARITY",
    "NP_ACTIVATE_FIELD",
    "NP_ACTIVATE_CRYPTO1",
    "NP_INFINITE_SELECT",
    "NP_ACCEPT_INVALID_FRAMES",
    "NP_ACCEPT_MULTIPLE_FRAMES",
    "NP_AUTO_ISO14443_4",
    "NP_EASY_FRAMING",
    "NP_FORCE_ISO14443_A",
    "NP_FORCE_ISO14443_B",
    "NP_FORCE_SPEED_106",
];

struct PropertyBoolSetting {
    property: nfc_property,
    value: bool,
}

#[cfg(all(not(test), libnfc_external_bridges))]
unsafe extern "C" {
    fn iso14443_cascade_uid(
        abt_uid: *const u8,
        sz_uid: size_t,
        pbt_cascaded_uid: *mut u8,
        psz_cascaded_uid: *mut size_t,
    );
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

fn log_general_error(message: &str) {
    log_general_message(LOG_PRIORITY_ERROR, message);
}

fn error_message_ptr(code: c_int) -> *const c_char {
    match code {
        NFC_SUCCESS => SUCCESS_MESSAGE,
        -1 => IO_ERROR_MESSAGE,
        NFC_EINVARG => INVALID_ARGUMENT_MESSAGE,
        NFC_EDEVNOTSUPP => DEVICE_NOT_SUPPORTED_MESSAGE,
        NFC_ENOTSUCHDEV => NO_SUCH_DEVICE_MESSAGE,
        NFC_EOVFLOW => BUFFER_OVERFLOW_MESSAGE,
        NFC_ETIMEOUT => TIMEOUT_MESSAGE,
        NFC_EOPABORTED => OPERATION_ABORTED_MESSAGE,
        NFC_ENOTIMPL => NOT_IMPLEMENTED_MESSAGE,
        NFC_ETGRELEASED => TARGET_RELEASED_MESSAGE,
        NFC_EMFCAUTHFAIL => MIFARE_AUTH_FAILED_MESSAGE,
        NFC_ERFTRANS => RF_TRANSMISSION_ERROR_MESSAGE,
        NFC_ECHIP => CHIP_ERROR_MESSAGE,
        _ => UNKNOWN_ERROR_MESSAGE,
    }
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

struct InfiniteSelectGuard {
    device: *mut nfc_device,
    previous: Option<bool>,
    temporary_value: bool,
    active: bool,
}

impl InfiniteSelectGuard {
    unsafe fn set(device: *mut nfc_device, temporary_value: bool) -> Result<Self, c_int> {
        let previous = unsafe { as_ref(device) }.map(|device| device.bInfiniteSelect);
        let result = unsafe {
            nfc_device_set_property_bool(device, nfc_property::NP_INFINITE_SELECT, temporary_value)
        };
        if result < 0 {
            return Err(result);
        }

        Ok(Self {
            device,
            previous,
            temporary_value,
            active: true,
        })
    }

    fn restore(&mut self) -> c_int {
        if !self.active {
            return NFC_SUCCESS;
        }
        self.active = false;

        let Some(previous) = self.previous else {
            return NFC_SUCCESS;
        };
        if previous == self.temporary_value {
            return NFC_SUCCESS;
        }

        unsafe {
            nfc_device_set_property_bool(self.device, nfc_property::NP_INFINITE_SELECT, previous)
        }
    }

    fn finish(mut self, result: c_int) -> c_int {
        let restore_result = self.restore();
        if restore_result < 0 {
            restore_result
        } else {
            result
        }
    }
}

impl Drop for InfiniteSelectGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

fn property_name(property: nfc_property) -> &'static str {
    let index = property as usize;
    PROPERTY_NAMES
        .get(index)
        .copied()
        .unwrap_or("UNKNOWN_PROPERTY")
}

#[cfg(all(not(test), libnfc_external_bridges))]
unsafe fn bridge_iso14443_cascade_uid(
    abt_uid: *const u8,
    sz_uid: size_t,
    pbt_cascaded_uid: *mut u8,
    psz_cascaded_uid: *mut size_t,
) {
    unsafe {
        iso14443_cascade_uid(abt_uid, sz_uid, pbt_cascaded_uid, psz_cascaded_uid);
    }
}

#[cfg(any(test, not(libnfc_external_bridges)))]
unsafe fn bridge_iso14443_cascade_uid(
    abt_uid: *const u8,
    sz_uid: size_t,
    pbt_cascaded_uid: *mut u8,
    psz_cascaded_uid: *mut size_t,
) {
    if psz_cascaded_uid.is_null() || pbt_cascaded_uid.is_null() {
        return;
    }

    unsafe {
        match sz_uid as usize {
            4 => {
                *psz_cascaded_uid = sz_uid;
                ptr::copy_nonoverlapping(abt_uid, pbt_cascaded_uid, 4);
            }
            7 => {
                *psz_cascaded_uid = 8;
                *pbt_cascaded_uid.add(0) = 0x88;
                ptr::copy_nonoverlapping(abt_uid, pbt_cascaded_uid.add(1), 3);
                ptr::copy_nonoverlapping(abt_uid.add(3), pbt_cascaded_uid.add(4), 4);
            }
            10 => {
                *psz_cascaded_uid = 12;
                *pbt_cascaded_uid.add(0) = 0x88;
                ptr::copy_nonoverlapping(abt_uid, pbt_cascaded_uid.add(1), 3);
                *pbt_cascaded_uid.add(4) = 0x88;
                ptr::copy_nonoverlapping(abt_uid.add(3), pbt_cascaded_uid.add(5), 3);
                ptr::copy_nonoverlapping(abt_uid.add(6), pbt_cascaded_uid.add(8), 4);
            }
            _ => {
                *psz_cascaded_uid = 0;
            }
        }
    }
}

unsafe fn call_device_set_property_bool_impl(
    device: *mut nfc_device,
    property: nfc_property,
    enable: bool,
) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .device_set_property_bool
                .map(|callback| callback(device, property, enable))
        })
    }
}

unsafe fn call_device_set_property_int_impl(
    device: *mut nfc_device,
    property: nfc_property,
    value: c_int,
) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .device_set_property_int
                .map(|callback| callback(device, property, value))
        })
    }
}

unsafe fn call_initiator_init_impl(device: *mut nfc_device) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver.initiator_init.map(|callback| callback(device))
        })
    }
}

unsafe fn call_initiator_init_secure_element_impl(device: *mut nfc_device) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .initiator_init_secure_element
                .map(|callback| callback(device))
        })
    }
}

unsafe fn call_initiator_select_passive_target_impl(
    device: *mut nfc_device,
    nm: nfc_modulation,
    init_data: *const u8,
    init_data_len: usize,
    target: *mut nfc_target,
) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .initiator_select_passive_target
                .map(|callback| callback(device, nm, init_data, init_data_len, target))
        })
    }
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

unsafe fn call_initiator_select_dep_target_impl(
    device: *mut nfc_device,
    ndm: nfc_dep_mode,
    nbr: nfc_baud_rate,
    initiator: *const nfc_dep_info,
    target: *mut nfc_target,
    timeout: c_int,
) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .initiator_select_dep_target
                .map(|callback| callback(device, ndm, nbr, initiator, target, timeout))
        })
    }
}

unsafe fn call_initiator_deselect_target_impl(device: *mut nfc_device) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .initiator_deselect_target
                .map(|callback| callback(device))
        })
    }
}

unsafe fn call_initiator_transceive_bytes_impl(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: usize,
    rx: *mut u8,
    rx_len: usize,
    timeout: c_int,
) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .initiator_transceive_bytes
                .map(|callback| callback(device, tx, tx_len, rx, rx_len, timeout))
        })
    }
}

unsafe fn call_initiator_transceive_bits_impl(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: usize,
    tx_parity: *const u8,
    rx: *mut u8,
    rx_parity: *mut u8,
) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .initiator_transceive_bits
                .map(|callback| callback(device, tx, tx_bits_len, tx_parity, rx, rx_parity))
        })
    }
}

unsafe fn call_initiator_transceive_bytes_timed_impl(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: usize,
    rx: *mut u8,
    rx_len: usize,
    cycles: *mut u32,
) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .initiator_transceive_bytes_timed
                .map(|callback| callback(device, tx, tx_len, rx, rx_len, cycles))
        })
    }
}

unsafe fn call_initiator_transceive_bits_timed_impl(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: usize,
    tx_parity: *const u8,
    rx: *mut u8,
    rx_parity: *mut u8,
    cycles: *mut u32,
) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .initiator_transceive_bits_timed
                .map(|callback| callback(device, tx, tx_bits_len, tx_parity, rx, rx_parity, cycles))
        })
    }
}

unsafe fn call_initiator_target_is_present_impl(
    device: *mut nfc_device,
    target: *const nfc_target,
) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .initiator_target_is_present
                .map(|callback| callback(device, target))
        })
    }
}

unsafe fn call_target_init_impl(
    device: *mut nfc_device,
    target: *mut nfc_target,
    rx: *mut u8,
    rx_len: usize,
    timeout: c_int,
) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .target_init
                .map(|callback| callback(device, target, rx, rx_len, timeout))
        })
    }
}

unsafe fn call_target_send_bytes_impl(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: usize,
    timeout: c_int,
) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .target_send_bytes
                .map(|callback| callback(device, tx, tx_len, timeout))
        })
    }
}

unsafe fn call_target_receive_bytes_impl(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: usize,
    timeout: c_int,
) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .target_receive_bytes
                .map(|callback| callback(device, rx, rx_len, timeout))
        })
    }
}

unsafe fn call_target_send_bits_impl(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: usize,
    tx_parity: *const u8,
) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .target_send_bits
                .map(|callback| callback(device, tx, tx_bits_len, tx_parity))
        })
    }
}

unsafe fn call_target_receive_bits_impl(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: usize,
    rx_parity: *mut u8,
) -> c_int {
    unsafe {
        dispatch_driver_call(device, |driver| {
            driver
                .target_receive_bits
                .map(|callback| callback(device, rx, rx_len, rx_parity))
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

fn apply_property_sequence(device: *mut nfc_device, settings: &[PropertyBoolSetting]) -> c_int {
    for setting in settings {
        let res = unsafe { nfc_device_set_property_bool(device, setting.property, setting.value) };
        if res < 0 {
            return res;
        }
    }

    NFC_SUCCESS
}

unsafe fn read_modulation_type(nm: *const nfc_modulation) -> nfc_modulation_type {
    unsafe { ptr::addr_of!((*nm).nmt).read_unaligned() }
}

unsafe fn read_baud_rate(nm: *const nfc_modulation) -> nfc_baud_rate {
    unsafe { ptr::addr_of!((*nm).nbr).read_unaligned() }
}

fn modulation_supported(supported: *const nfc_modulation_type, value: nfc_modulation_type) -> bool {
    if supported.is_null() {
        return false;
    }

    let mut index = 0usize;
    loop {
        let candidate = unsafe { supported.add(index).read() };
        if candidate as c_int == 0 {
            return false;
        }
        if candidate == value {
            return true;
        }
        index += 1;
    }
}

fn baud_rate_supported(supported: *const nfc_baud_rate, value: nfc_baud_rate) -> bool {
    if supported.is_null() {
        return false;
    }

    let mut index = 0usize;
    loop {
        let candidate = unsafe { supported.add(index).read() };
        if candidate as c_int == 0 {
            return false;
        }
        if candidate == value {
            return true;
        }
        index += 1;
    }
}

unsafe fn get_baud_rates_for_mode(
    device: *mut nfc_device,
    mode: nfc_mode,
    modulation_type: nfc_modulation_type,
    status: &mut c_int,
) -> *const nfc_baud_rate {
    let mut supported = ptr::null();
    *status =
        unsafe { get_supported_baud_rate_impl(device, mode, modulation_type, &mut supported) };
    if *status < 0 {
        return ptr::null();
    }
    supported
}

unsafe fn validate_modulation(
    device: *mut nfc_device,
    mode: nfc_mode,
    nm: *const nfc_modulation,
) -> c_int {
    let mut supported_types = ptr::null();
    let mut res = unsafe { get_supported_modulation_impl(device, mode, &mut supported_types) };
    if res < 0 {
        return res;
    }

    let modulation_type = unsafe { read_modulation_type(nm) };
    if !modulation_supported(supported_types, modulation_type) {
        log_general_debug("Modulation type not supported");
        return NFC_EINVARG;
    }

    let supported_rates =
        unsafe { get_baud_rates_for_mode(device, mode, modulation_type, &mut res) };
    if res < 0 {
        return res;
    }

    let baud_rate = unsafe { read_baud_rate(nm) };
    if baud_rate_supported(supported_rates, baud_rate) {
        return NFC_SUCCESS;
    }

    log_general_debug("Baud rate not supported");
    NFC_EINVARG
}

fn default_initiator_data(nm: nfc_modulation) -> (*const u8, usize) {
    match unsafe { ptr::addr_of!(nm.nmt).read_unaligned() } {
        nfc_modulation_type::NMT_ISO14443B => {
            static ISO14443B: [u8; 1] = [0x00];
            (ISO14443B.as_ptr(), ISO14443B.len())
        }
        nfc_modulation_type::NMT_ISO14443BI => {
            static ISO14443BI: [u8; 4] = [0x01, 0x0b, 0x3f, 0x80];
            (ISO14443BI.as_ptr(), ISO14443BI.len())
        }
        nfc_modulation_type::NMT_FELICA => {
            static FELICA: [u8; 5] = [0x00, 0xff, 0xff, 0x01, 0x00];
            (FELICA.as_ptr(), FELICA.len())
        }
        _ => (ptr::null(), 0),
    }
}

unsafe fn copy_target_bytes(dst: *mut nfc_target, src: *const nfc_target) {
    unsafe {
        ptr::copy_nonoverlapping(src as *const u8, dst as *mut u8, size_of::<nfc_target>());
    }
}

fn targets_equal(left: *const nfc_target, right: *const nfc_target) -> bool {
    let left_bytes = unsafe { slice::from_raw_parts(left as *const u8, size_of::<nfc_target>()) };
    let right_bytes = unsafe { slice::from_raw_parts(right as *const u8, size_of::<nfc_target>()) };
    left_bytes == right_bytes
}

fn target_already_seen(
    targets: *const nfc_target,
    count: usize,
    candidate: *const nfc_target,
) -> bool {
    for index in 0..count {
        if targets_equal(unsafe { targets.add(index) }, candidate) {
            return true;
        }
    }

    false
}

fn modulation_requires_single_attempt(nm: nfc_modulation) -> bool {
    matches!(
        unsafe { ptr::addr_of!(nm.nmt).read_unaligned() },
        nfc_modulation_type::NMT_FELICA
            | nfc_modulation_type::NMT_JEWEL
            | nfc_modulation_type::NMT_BARCODE
            | nfc_modulation_type::NMT_ISO14443BI
            | nfc_modulation_type::NMT_ISO14443B2SR
            | nfc_modulation_type::NMT_ISO14443B2CT
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_set_property_int(
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
        unsafe { call_device_set_property_int_impl(device, property, value) }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_set_property_bool(
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
        unsafe { call_device_set_property_bool_impl(device, property, enable) }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_init(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_init", NFC_ESOFT, || {
        const INITIATOR_SETTINGS: [PropertyBoolSetting; 8] = [
            PropertyBoolSetting {
                property: nfc_property::NP_ACTIVATE_FIELD,
                value: false,
            },
            PropertyBoolSetting {
                property: nfc_property::NP_ACTIVATE_FIELD,
                value: true,
            },
            PropertyBoolSetting {
                property: nfc_property::NP_INFINITE_SELECT,
                value: true,
            },
            PropertyBoolSetting {
                property: nfc_property::NP_AUTO_ISO14443_4,
                value: true,
            },
            PropertyBoolSetting {
                property: nfc_property::NP_FORCE_ISO14443_A,
                value: true,
            },
            PropertyBoolSetting {
                property: nfc_property::NP_FORCE_SPEED_106,
                value: true,
            },
            PropertyBoolSetting {
                property: nfc_property::NP_ACCEPT_INVALID_FRAMES,
                value: false,
            },
            PropertyBoolSetting {
                property: nfc_property::NP_ACCEPT_MULTIPLE_FRAMES,
                value: false,
            },
        ];

        let res = apply_property_sequence(device, &INITIATOR_SETTINGS);
        if res < 0 {
            return res;
        }

        unsafe { call_initiator_init_impl(device) }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_init_secure_element(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_init_secure_element", NFC_ESOFT, || unsafe {
        call_initiator_init_secure_element_impl(device)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_select_passive_target(
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
            let validation = validate_modulation(device, nfc_mode::N_INITIATOR, ptr::addr_of!(nm));
            if validation != NFC_SUCCESS {
                return validation;
            }

            if init_data_len == 0 {
                let (default_data, default_len) = default_initiator_data(nm);
                return call_initiator_select_passive_target_impl(
                    device,
                    nm,
                    default_data,
                    default_len,
                    target,
                );
            }

            if init_data.is_null() {
                log_general_error("Failed to copy init data");
                return NFC_EINVARG;
            }

            let max_abt = (init_data_len as usize).max(12);
            let mut abt_init = vec![0u8; max_abt];
            let mut cascaded_len = 0usize;

            if read_modulation_type(ptr::addr_of!(nm)) == nfc_modulation_type::NMT_ISO14443A {
                bridge_iso14443_cascade_uid(
                    init_data,
                    init_data_len,
                    abt_init.as_mut_ptr(),
                    &mut cascaded_len,
                );
            } else {
                ptr::copy_nonoverlapping(init_data, abt_init.as_mut_ptr(), init_data_len as usize);
                cascaded_len = init_data_len as usize;
            }

            call_initiator_select_passive_target_impl(
                device,
                nm,
                abt_init.as_ptr(),
                cascaded_len,
                target,
            )
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_list_passive_targets(
    device: *mut nfc_device,
    nm: nfc_modulation,
    targets: *mut nfc_target,
    targets_len: size_t,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_list_passive_targets", NFC_ESOFT, || unsafe {
        if targets_len == 0 {
            return 0;
        }

        reset_device_last_error(device);
        let guard = match InfiniteSelectGuard::set(device, false) {
            Ok(guard) => guard,
            Err(error) => return error,
        };

        let (init_data, init_data_len) = default_initiator_data(nm);
        let mut target_count = 0usize;
        let mut candidate = std::mem::zeroed::<nfc_target>();

        while nfc_initiator_select_passive_target(
            device,
            nm,
            init_data,
            init_data_len,
            ptr::addr_of_mut!(candidate),
        ) > 0
        {
            if target_already_seen(targets, target_count, ptr::addr_of!(candidate)) {
                break;
            }

            copy_target_bytes(targets.add(target_count), ptr::addr_of!(candidate));
            target_count += 1;

            if target_count >= targets_len as usize || modulation_requires_single_attempt(nm) {
                break;
            }

            nfc_initiator_deselect_target(device);
        }

        guard.finish(target_count as c_int)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_poll_target(
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_select_dep_target(
    device: *mut nfc_device,
    ndm: nfc_dep_mode,
    nbr: nfc_baud_rate,
    initiator: *const nfc_dep_info,
    target: *mut nfc_target,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_select_dep_target", NFC_ESOFT, || unsafe {
        call_initiator_select_dep_target_impl(device, ndm, nbr, initiator, target, timeout)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_poll_dep_target(
    device: *mut nfc_device,
    ndm: nfc_dep_mode,
    nbr: nfc_baud_rate,
    initiator: *const nfc_dep_info,
    target: *mut nfc_target,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_poll_dep_target", NFC_ESOFT, || unsafe {
        const PERIOD: c_int = 300;
        let mut remaining_time = timeout;
        let mut result = 0;
        let guard = match InfiniteSelectGuard::set(device, true) {
            Ok(guard) => guard,
            Err(error) => return error,
        };

        while remaining_time > 0 {
            let select_res =
                nfc_initiator_select_dep_target(device, ndm, nbr, initiator, target, PERIOD);

            if select_res < 0 && select_res != NFC_ETIMEOUT {
                result = select_res;
                break;
            }

            if select_res == 1 {
                result = select_res;
                break;
            }

            remaining_time -= PERIOD;
        }

        guard.finish(result)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_deselect_target(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_deselect_target", NFC_ESOFT, || unsafe {
        call_initiator_deselect_target_impl(device)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_target_is_present(
    device: *mut nfc_device,
    target: *const nfc_target,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_target_is_present", NFC_ESOFT, || unsafe {
        call_initiator_target_is_present_impl(device, target)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_target_init(
    device: *mut nfc_device,
    target: *mut nfc_target,
    rx: *mut u8,
    rx_len: size_t,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_target_init", NFC_ESOFT, || unsafe {
        const TARGET_SETTINGS: [PropertyBoolSetting; 8] = [
            PropertyBoolSetting {
                property: nfc_property::NP_ACCEPT_INVALID_FRAMES,
                value: false,
            },
            PropertyBoolSetting {
                property: nfc_property::NP_ACCEPT_MULTIPLE_FRAMES,
                value: false,
            },
            PropertyBoolSetting {
                property: nfc_property::NP_HANDLE_CRC,
                value: true,
            },
            PropertyBoolSetting {
                property: nfc_property::NP_HANDLE_PARITY,
                value: true,
            },
            PropertyBoolSetting {
                property: nfc_property::NP_AUTO_ISO14443_4,
                value: true,
            },
            PropertyBoolSetting {
                property: nfc_property::NP_EASY_FRAMING,
                value: true,
            },
            PropertyBoolSetting {
                property: nfc_property::NP_ACTIVATE_CRYPTO1,
                value: false,
            },
            PropertyBoolSetting {
                property: nfc_property::NP_ACTIVATE_FIELD,
                value: false,
            },
        ];

        let res = apply_property_sequence(device, &TARGET_SETTINGS);
        if res < 0 {
            return res;
        }

        call_target_init_impl(device, target, rx, rx_len, timeout)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_transceive_bytes(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: size_t,
    rx: *mut u8,
    rx_len: size_t,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_transceive_bytes", NFC_ESOFT, || unsafe {
        call_initiator_transceive_bytes_impl(device, tx, tx_len, rx, rx_len, timeout)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_transceive_bits(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: size_t,
    tx_parity: *const u8,
    rx: *mut u8,
    rx_len: size_t,
    rx_parity: *mut u8,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_transceive_bits", NFC_ESOFT, || unsafe {
        let _ = rx_len;
        call_initiator_transceive_bits_impl(device, tx, tx_bits_len, tx_parity, rx, rx_parity)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_transceive_bytes_timed(
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
            call_initiator_transceive_bytes_timed_impl(device, tx, tx_len, rx, rx_len, cycles)
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_initiator_transceive_bits_timed(
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
            let _ = rx_len;
            call_initiator_transceive_bits_timed_impl(
                device,
                tx,
                tx_bits_len,
                tx_parity,
                rx,
                rx_parity,
                cycles,
            )
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_target_send_bytes(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: size_t,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_target_send_bytes", NFC_ESOFT, || unsafe {
        call_target_send_bytes_impl(device, tx, tx_len, timeout)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_target_receive_bytes(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: size_t,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_target_receive_bytes", NFC_ESOFT, || unsafe {
        call_target_receive_bytes_impl(device, rx, rx_len, timeout)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_target_send_bits(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: size_t,
    tx_parity: *const u8,
) -> c_int {
    ffi_catch_unwind_int("nfc_target_send_bits", NFC_ESOFT, || unsafe {
        call_target_send_bits_impl(device, tx, tx_bits_len, tx_parity)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_target_receive_bits(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: size_t,
    rx_parity: *mut u8,
) -> c_int {
    ffi_catch_unwind_int("nfc_target_receive_bits", NFC_ESOFT, || unsafe {
        call_target_receive_bits_impl(device, rx, rx_len, rx_parity)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_abort_command(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_abort_command", NFC_ESOFT, || unsafe {
        call_abort_command_impl(device)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_idle(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_idle", NFC_ESOFT, || unsafe { call_idle_impl(device) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_name(device: *mut nfc_device) -> *const c_char {
    ffi_catch_unwind_ptr("nfc_device_get_name", || unsafe {
        as_ref(device)
            .map(|device| device.name.as_ptr().cast_mut())
            .unwrap_or(ptr::null_mut())
    }) as *const c_char
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_connstring(device: *mut nfc_device) -> *const c_char {
    ffi_catch_unwind_ptr("nfc_device_get_connstring", || unsafe {
        as_ref(device)
            .map(|device| device.connstring.as_ptr().cast_mut())
            .unwrap_or(ptr::null_mut())
    }) as *const c_char
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_supported_modulation(
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_supported_baud_rate(
    device: *mut nfc_device,
    modulation_type: nfc_modulation_type,
    supported: *mut *const nfc_baud_rate,
) -> c_int {
    ffi_catch_unwind_int("nfc_device_get_supported_baud_rate", NFC_ESOFT, || unsafe {
        get_supported_baud_rate_impl(device, nfc_mode::N_INITIATOR, modulation_type, supported)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_supported_baud_rate_target_mode(
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_information_about(
    device: *mut nfc_device,
    buf: *mut *mut c_char,
) -> c_int {
    ffi_catch_unwind_int("nfc_device_get_information_about", NFC_ESOFT, || unsafe {
        get_information_about_impl(device, buf)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_get_last_error(device: *const nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_device_get_last_error", NFC_ESOFT, || unsafe {
        device_last_error(device)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_strerror(device: *const nfc_device) -> *const c_char {
    ffi_catch_unwind_ptr("nfc_strerror", || unsafe {
        error_message_ptr(device_last_error(device)).cast_mut()
    }) as *const c_char
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_strerror_r(
    device: *const nfc_device,
    buf: *mut c_char,
    buflen: size_t,
) -> c_int {
    ffi_catch_unwind_int("nfc_strerror_r", NFC_ESOFT, || unsafe {
        if buf.is_null() && buflen > 0 {
            return -1;
        }

        let written = libc::snprintf(buf, buflen, PRINTF_STRING_FORMAT, nfc_strerror(device));
        if written < 0 { -1 } else { 0 }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_perror(device: *const nfc_device, message: *const c_char) {
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
                    64,
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
                    99,
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
