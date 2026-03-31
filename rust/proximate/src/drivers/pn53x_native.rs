// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Shared PN53x bridge definitions for native PN532 UART/SPI/I2C transports.

#![allow(non_camel_case_types)]

use crate::ffi_support::{as_mut, as_ref, copy_bytes_to_c_buffer};
use crate::ffi_types::{
    nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_mode, nfc_modulation, nfc_modulation_type,
    nfc_property, nfc_target,
};
use crate::lifecycle::nfc_device;
use libc::{c_char, c_int, c_void};
use std::ptr;
use std::slice;

pub(crate) const NFC_SUCCESS: c_int = 0;
pub(crate) const NFC_EIO: c_int = -1;
pub(crate) const NFC_EINVARG: c_int = -2;
pub(crate) const NFC_ETIMEOUT: c_int = -6;
pub(crate) const NFC_EOPABORTED: c_int = -7;
pub(crate) const NFC_ESOFT: c_int = -80;

pub(crate) const PN53X_EXTENDED_FRAME_DATA_MAX_LEN: usize = 264;
pub(crate) const PN53X_EXTENDED_FRAME_OVERHEAD: usize = 11;
pub(crate) const PN53X_ACK_FRAME_LEN: usize = 6;
pub(crate) const PN532_BUFFER_LEN: usize =
    PN53X_EXTENDED_FRAME_DATA_MAX_LEN + PN53X_EXTENDED_FRAME_OVERHEAD;
pub(crate) const PN532_TIMER_CORRECTION: i16 = 48;
pub(crate) const PN532_I2C_ADDR: u32 = 0x24;

pub(crate) const PN53X_PREAMBLE_AND_START: [u8; 3] = [0x00, 0x00, 0xff];

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) enum pn53x_type {
    PN53X = 0x00,
    PN531 = 0x01,
    PN532 = 0x02,
    PN533 = 0x04,
    RCS360 = 0x08,
}

#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum pn53x_power_mode {
    NORMAL = 0,
    POWERDOWN = 1,
    LOWVBAT = 2,
}

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub(crate) enum pn53x_operating_mode {
    IDLE = 0,
    INITIATOR = 1,
    TARGET = 2,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) enum pn532_sam_mode {
    PSM_NORMAL = 0x01,
    #[allow(dead_code)]
    PSM_VIRTUAL_CARD = 0x02,
    #[allow(dead_code)]
    PSM_WIRED_CARD = 0x03,
    #[allow(dead_code)]
    PSM_DUAL_CARD = 0x04,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct pn53x_io {
    pub send: Option<unsafe extern "C" fn(*mut nfc_device, *const u8, usize, c_int) -> c_int>,
    pub receive: Option<unsafe extern "C" fn(*mut nfc_device, *mut u8, usize, c_int) -> c_int>,
}

#[repr(C)]
#[allow(non_snake_case)]
pub(crate) struct pn53x_data {
    pub type_: pn53x_type,
    pub firmware_text: [c_char; 22],
    pub power_mode: pn53x_power_mode,
    pub operating_mode: pn53x_operating_mode,
    pub current_target: *mut nfc_target,
    pub sam_mode: pn532_sam_mode,
    pub io: *const pn53x_io,
    pub last_status_byte: u8,
    pub ui8TxBits: u8,
    pub ui8Parameters: u8,
    pub last_command: u8,
    pub timer_correction: i16,
    pub timer_prescaler: u16,
    pub wb_data: [u8; 0x18],
    pub wb_mask: [u8; 0x18],
    pub wb_trigged: bool,
    pub timeout_command: c_int,
    pub timeout_atr: c_int,
    pub timeout_communication: c_int,
    pub supported_modulation_as_initiator: *mut nfc_modulation_type,
    pub supported_modulation_as_target: *mut nfc_modulation_type,
    pub progressive_field: bool,
}

pub(crate) unsafe fn chip_data<'a>(device: *mut nfc_device) -> Option<&'a mut pn53x_data> {
    let device = unsafe { as_mut(device) }?;
    unsafe { as_mut(device.chip_data.cast::<pn53x_data>()) }
}

pub(crate) unsafe fn free_decode_param(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe { crate::release_allocated_ptr(ptr.cast::<c_void>()) };
    }
}

#[cfg(not(test))]
unsafe extern "C" {
    static pn53x_ack_frame: [u8; PN53X_ACK_FRAME_LEN];
    fn pn53x_build_frame(
        frame: *mut u8,
        frame_len: *mut usize,
        data: *const u8,
        data_len: usize,
    ) -> c_int;
    fn pn53x_check_ack_frame(device: *mut nfc_device, frame: *const u8, frame_len: usize) -> c_int;
    fn pn53x_check_communication(device: *mut nfc_device) -> c_int;
    fn pn53x_data_new(device: *mut nfc_device, io: *const pn53x_io) -> *mut c_void;
    fn pn53x_data_free(device: *mut nfc_device);
    fn pn53x_init(device: *mut nfc_device) -> c_int;
    fn pn53x_strerror(device: *const nfc_device) -> *const c_char;
    fn pn53x_initiator_init(device: *mut nfc_device) -> c_int;
    fn pn532_initiator_init_secure_element(device: *mut nfc_device) -> c_int;
    fn pn53x_initiator_select_passive_target(
        device: *mut nfc_device,
        modulation: nfc_modulation,
        init_data: *const u8,
        init_data_len: usize,
        target: *mut nfc_target,
    ) -> c_int;
    fn pn53x_initiator_poll_target(
        device: *mut nfc_device,
        modulations: *const nfc_modulation,
        modulation_count: usize,
        poll_nr: u8,
        period: u8,
        target: *mut nfc_target,
    ) -> c_int;
    fn pn53x_initiator_select_dep_target(
        device: *mut nfc_device,
        dep_mode: nfc_dep_mode,
        baud_rate: nfc_baud_rate,
        initiator: *const nfc_dep_info,
        target: *mut nfc_target,
        timeout: c_int,
    ) -> c_int;
    fn pn53x_initiator_deselect_target(device: *mut nfc_device) -> c_int;
    fn pn53x_initiator_transceive_bytes(
        device: *mut nfc_device,
        tx: *const u8,
        tx_len: usize,
        rx: *mut u8,
        rx_len: usize,
        timeout: c_int,
    ) -> c_int;
    fn pn53x_initiator_transceive_bits(
        device: *mut nfc_device,
        tx: *const u8,
        tx_bits_len: usize,
        tx_parity: *const u8,
        rx: *mut u8,
        rx_parity: *mut u8,
    ) -> c_int;
    fn pn53x_initiator_transceive_bytes_timed(
        device: *mut nfc_device,
        tx: *const u8,
        tx_len: usize,
        rx: *mut u8,
        rx_len: usize,
        cycles: *mut u32,
    ) -> c_int;
    fn pn53x_initiator_transceive_bits_timed(
        device: *mut nfc_device,
        tx: *const u8,
        tx_bits_len: usize,
        tx_parity: *const u8,
        rx: *mut u8,
        rx_parity: *mut u8,
        cycles: *mut u32,
    ) -> c_int;
    fn pn53x_initiator_target_is_present(
        device: *mut nfc_device,
        target: *const nfc_target,
    ) -> c_int;
    fn pn53x_target_init(
        device: *mut nfc_device,
        target: *mut nfc_target,
        rx: *mut u8,
        rx_len: usize,
        timeout: c_int,
    ) -> c_int;
    fn pn53x_target_send_bytes(
        device: *mut nfc_device,
        tx: *const u8,
        tx_len: usize,
        timeout: c_int,
    ) -> c_int;
    fn pn53x_target_receive_bytes(
        device: *mut nfc_device,
        rx: *mut u8,
        rx_len: usize,
        timeout: c_int,
    ) -> c_int;
    fn pn53x_target_send_bits(
        device: *mut nfc_device,
        tx: *const u8,
        tx_bits_len: usize,
        tx_parity: *const u8,
    ) -> c_int;
    fn pn53x_target_receive_bits(
        device: *mut nfc_device,
        rx: *mut u8,
        rx_len: usize,
        rx_parity: *mut u8,
    ) -> c_int;
    fn pn53x_set_property_bool(
        device: *mut nfc_device,
        property: nfc_property,
        enable: bool,
    ) -> c_int;
    fn pn53x_set_property_int(
        device: *mut nfc_device,
        property: nfc_property,
        value: c_int,
    ) -> c_int;
    fn pn53x_get_supported_modulation(
        device: *mut nfc_device,
        mode: nfc_mode,
        supported: *mut *const nfc_modulation_type,
    ) -> c_int;
    fn pn53x_get_supported_baud_rate(
        device: *mut nfc_device,
        mode: nfc_mode,
        modulation_type: nfc_modulation_type,
        supported: *mut *const nfc_baud_rate,
    ) -> c_int;
    fn pn53x_get_information_about(device: *mut nfc_device, buffer: *mut *mut c_char) -> c_int;
    fn pn53x_idle(device: *mut nfc_device) -> c_int;
    fn pn53x_PowerDown(device: *mut nfc_device) -> c_int;
    fn pn532_SAMConfiguration(
        device: *mut nfc_device,
        mode: pn532_sam_mode,
        timeout: c_int,
    ) -> c_int;
}

#[cfg(not(test))]
pub(crate) fn pn53x_ack_frame_bytes() -> &'static [u8] {
    unsafe { &pn53x_ack_frame }
}

#[cfg(not(test))]
pub(crate) unsafe fn pn53x_build_frame_bridge(
    frame: *mut u8,
    frame_len: *mut usize,
    data: *const u8,
    data_len: usize,
) -> c_int {
    unsafe { pn53x_build_frame(frame, frame_len, data, data_len) }
}

#[cfg(not(test))]
pub(crate) unsafe fn pn53x_check_ack_frame_bridge(
    device: *mut nfc_device,
    frame: *const u8,
    frame_len: usize,
) -> c_int {
    unsafe { pn53x_check_ack_frame(device, frame, frame_len) }
}

#[cfg(not(test))]
pub(crate) unsafe fn pn53x_check_communication_bridge(device: *mut nfc_device) -> c_int {
    unsafe { pn53x_check_communication(device) }
}

#[cfg(not(test))]
pub(crate) unsafe fn pn53x_data_new_bridge(
    device: *mut nfc_device,
    io: *const pn53x_io,
) -> *mut c_void {
    unsafe { pn53x_data_new(device, io) }
}

#[cfg(not(test))]
pub(crate) unsafe fn pn53x_data_free_bridge(device: *mut nfc_device) {
    unsafe { pn53x_data_free(device) };
}

#[cfg(not(test))]
pub(crate) unsafe fn pn53x_init_bridge(device: *mut nfc_device) -> c_int {
    unsafe { pn53x_init(device) }
}

#[cfg(not(test))]
pub(crate) unsafe fn pn532_sam_configuration_bridge(
    device: *mut nfc_device,
    mode: pn532_sam_mode,
    timeout: c_int,
) -> c_int {
    unsafe { pn532_SAMConfiguration(device, mode, timeout) }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_strerror_callback(
    device: *const nfc_device,
) -> *const c_char {
    unsafe { pn53x_strerror(device) }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_initiator_init_callback(device: *mut nfc_device) -> c_int {
    unsafe { pn53x_initiator_init(device) }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn532_initiator_init_secure_element_callback(
    device: *mut nfc_device,
) -> c_int {
    unsafe { pn532_initiator_init_secure_element(device) }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_initiator_select_passive_target_callback(
    device: *mut nfc_device,
    modulation: nfc_modulation,
    init_data: *const u8,
    init_data_len: usize,
    target: *mut nfc_target,
) -> c_int {
    unsafe {
        pn53x_initiator_select_passive_target(device, modulation, init_data, init_data_len, target)
    }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_initiator_poll_target_callback(
    device: *mut nfc_device,
    modulations: *const nfc_modulation,
    modulation_count: usize,
    poll_nr: u8,
    period: u8,
    target: *mut nfc_target,
) -> c_int {
    unsafe {
        pn53x_initiator_poll_target(
            device,
            modulations,
            modulation_count,
            poll_nr,
            period,
            target,
        )
    }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_initiator_select_dep_target_callback(
    device: *mut nfc_device,
    dep_mode: nfc_dep_mode,
    baud_rate: nfc_baud_rate,
    initiator: *const nfc_dep_info,
    target: *mut nfc_target,
    timeout: c_int,
) -> c_int {
    unsafe {
        pn53x_initiator_select_dep_target(device, dep_mode, baud_rate, initiator, target, timeout)
    }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_initiator_deselect_target_callback(
    device: *mut nfc_device,
) -> c_int {
    unsafe { pn53x_initiator_deselect_target(device) }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_initiator_transceive_bytes_callback(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: usize,
    rx: *mut u8,
    rx_len: usize,
    timeout: c_int,
) -> c_int {
    unsafe { pn53x_initiator_transceive_bytes(device, tx, tx_len, rx, rx_len, timeout) }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_initiator_transceive_bits_callback(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: usize,
    tx_parity: *const u8,
    rx: *mut u8,
    rx_parity: *mut u8,
) -> c_int {
    unsafe { pn53x_initiator_transceive_bits(device, tx, tx_bits_len, tx_parity, rx, rx_parity) }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_initiator_transceive_bytes_timed_callback(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: usize,
    rx: *mut u8,
    rx_len: usize,
    cycles: *mut u32,
) -> c_int {
    unsafe { pn53x_initiator_transceive_bytes_timed(device, tx, tx_len, rx, rx_len, cycles) }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_initiator_transceive_bits_timed_callback(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: usize,
    tx_parity: *const u8,
    rx: *mut u8,
    rx_parity: *mut u8,
    cycles: *mut u32,
) -> c_int {
    unsafe {
        pn53x_initiator_transceive_bits_timed(
            device,
            tx,
            tx_bits_len,
            tx_parity,
            rx,
            rx_parity,
            cycles,
        )
    }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_initiator_target_is_present_callback(
    device: *mut nfc_device,
    target: *const nfc_target,
) -> c_int {
    unsafe { pn53x_initiator_target_is_present(device, target) }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_target_init_callback(
    device: *mut nfc_device,
    target: *mut nfc_target,
    rx: *mut u8,
    rx_len: usize,
    timeout: c_int,
) -> c_int {
    unsafe { pn53x_target_init(device, target, rx, rx_len, timeout) }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_target_send_bytes_callback(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: usize,
    timeout: c_int,
) -> c_int {
    unsafe { pn53x_target_send_bytes(device, tx, tx_len, timeout) }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_target_receive_bytes_callback(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: usize,
    timeout: c_int,
) -> c_int {
    unsafe { pn53x_target_receive_bytes(device, rx, rx_len, timeout) }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_target_send_bits_callback(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: usize,
    tx_parity: *const u8,
) -> c_int {
    unsafe { pn53x_target_send_bits(device, tx, tx_bits_len, tx_parity) }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_target_receive_bits_callback(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: usize,
    rx_parity: *mut u8,
) -> c_int {
    unsafe { pn53x_target_receive_bits(device, rx, rx_len, rx_parity) }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_set_property_bool_callback(
    device: *mut nfc_device,
    property: nfc_property,
    enable: bool,
) -> c_int {
    unsafe { pn53x_set_property_bool(device, property, enable) }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_set_property_int_callback(
    device: *mut nfc_device,
    property: nfc_property,
    value: c_int,
) -> c_int {
    unsafe { pn53x_set_property_int(device, property, value) }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_get_supported_modulation_callback(
    device: *mut nfc_device,
    mode: nfc_mode,
    supported: *mut *const nfc_modulation_type,
) -> c_int {
    unsafe { pn53x_get_supported_modulation(device, mode, supported) }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_get_supported_baud_rate_callback(
    device: *mut nfc_device,
    mode: nfc_mode,
    modulation_type: nfc_modulation_type,
    supported: *mut *const nfc_baud_rate,
) -> c_int {
    unsafe { pn53x_get_supported_baud_rate(device, mode, modulation_type, supported) }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_get_information_about_callback(
    device: *mut nfc_device,
    buffer: *mut *mut c_char,
) -> c_int {
    unsafe { pn53x_get_information_about(device, buffer) }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_idle_callback(device: *mut nfc_device) -> c_int {
    unsafe { pn53x_idle(device) }
}

#[cfg(not(test))]
pub(crate) unsafe extern "C" fn pn53x_powerdown_callback(device: *mut nfc_device) -> c_int {
    unsafe { pn53x_PowerDown(device) }
}

#[cfg(test)]
const TEST_ACK_FRAME: [u8; PN53X_ACK_FRAME_LEN] = [0x00, 0x00, 0xff, 0x00, 0xff, 0x00];
#[cfg(test)]
static TEST_MODULATION_TYPES: [nfc_modulation_type; 2] = [
    nfc_modulation_type::NMT_ISO14443A,
    nfc_modulation_type::NMT_UNDEFINED,
];
#[cfg(test)]
static TEST_BAUD_RATES: [nfc_baud_rate; 2] = [nfc_baud_rate::NBR_106, nfc_baud_rate::NBR_UNDEFINED];

#[cfg(test)]
#[derive(Default, Clone, Debug)]
pub(crate) struct TestStateSnapshot {
    pub check_communication_calls: usize,
    pub check_communication_results_remaining: usize,
    pub data_new_calls: usize,
    pub data_free_calls: usize,
    pub init_calls: usize,
    pub sam_configuration_calls: usize,
    pub supported_modulation_calls: Vec<nfc_mode>,
    pub supported_baud_rate_calls: Vec<(nfc_mode, nfc_modulation_type)>,
    pub info_requests: usize,
}

#[cfg(test)]
#[derive(Default)]
struct TestState {
    check_communication_results: std::collections::VecDeque<c_int>,
    check_communication_calls: usize,
    data_new_calls: usize,
    data_free_calls: usize,
    init_calls: usize,
    sam_configuration_calls: usize,
    supported_modulation_calls: Vec<nfc_mode>,
    supported_baud_rate_calls: Vec<(nfc_mode, nfc_modulation_type)>,
    info_requests: usize,
}

#[cfg(test)]
fn test_state() -> &'static std::sync::Mutex<TestState> {
    static STATE: std::sync::OnceLock<std::sync::Mutex<TestState>> = std::sync::OnceLock::new();
    STATE.get_or_init(|| std::sync::Mutex::new(TestState::default()))
}

#[cfg(test)]
pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

#[cfg(test)]
pub(crate) fn test_reset() {
    *test_state().lock().unwrap() = TestState::default();
}

#[cfg(test)]
pub(crate) fn test_queue_check_communication_result(code: c_int) {
    test_state()
        .lock()
        .unwrap()
        .check_communication_results
        .push_back(code);
}

#[cfg(test)]
pub(crate) fn test_snapshot() -> TestStateSnapshot {
    let state = test_state().lock().unwrap();
    TestStateSnapshot {
        check_communication_calls: state.check_communication_calls,
        check_communication_results_remaining: state.check_communication_results.len(),
        data_new_calls: state.data_new_calls,
        data_free_calls: state.data_free_calls,
        init_calls: state.init_calls,
        sam_configuration_calls: state.sam_configuration_calls,
        supported_modulation_calls: state.supported_modulation_calls.clone(),
        supported_baud_rate_calls: state.supported_baud_rate_calls.clone(),
        info_requests: state.info_requests,
    }
}

#[cfg(test)]
pub(crate) fn pn53x_ack_frame_bytes() -> &'static [u8] {
    &TEST_ACK_FRAME
}

#[cfg(test)]
pub(crate) unsafe fn pn53x_build_frame_bridge(
    frame: *mut u8,
    frame_len: *mut usize,
    data: *const u8,
    data_len: usize,
) -> c_int {
    if frame.is_null() || frame_len.is_null() || data.is_null() || data_len == 0 {
        return NFC_EINVARG;
    }

    let source = unsafe { slice::from_raw_parts(data, data_len) };
    let destination = unsafe { slice::from_raw_parts_mut(frame, PN532_BUFFER_LEN) };
    if data_len <= 254 {
        let len = data_len as u8 + 1;
        destination[..6].copy_from_slice(&[0x00, 0x00, 0xff, len, (!len).wrapping_add(1), 0xD4]);
        destination[6..6 + data_len].copy_from_slice(source);
        let mut dcs = 0u8.wrapping_sub(0xD4);
        for byte in source {
            dcs = dcs.wrapping_sub(*byte);
        }
        destination[6 + data_len] = dcs;
        destination[7 + data_len] = 0x00;
        unsafe { *frame_len = data_len + 8 };
    } else if data_len <= PN53X_EXTENDED_FRAME_DATA_MAX_LEN {
        let high = ((data_len + 1) >> 8) as u8;
        let low = ((data_len + 1) & 0xff) as u8;
        destination[..9].copy_from_slice(&[
            0x00,
            0x00,
            0xff,
            0xff,
            0xff,
            high,
            low,
            (0u8).wrapping_sub(high.wrapping_add(low)),
            0xD4,
        ]);
        destination[9..9 + data_len].copy_from_slice(source);
        let mut dcs = 0u8.wrapping_sub(0xD4);
        for byte in source {
            dcs = dcs.wrapping_sub(*byte);
        }
        destination[9 + data_len] = dcs;
        destination[10 + data_len] = 0x00;
        unsafe { *frame_len = data_len + 11 };
    } else {
        return NFC_EIO;
    }

    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe fn pn53x_check_ack_frame_bridge(
    _device: *mut nfc_device,
    frame: *const u8,
    frame_len: usize,
) -> c_int {
    if frame.is_null() || frame_len < TEST_ACK_FRAME.len() {
        return NFC_EIO;
    }
    if unsafe { slice::from_raw_parts(frame, TEST_ACK_FRAME.len()) } == TEST_ACK_FRAME {
        0
    } else {
        NFC_EIO
    }
}

#[cfg(test)]
pub(crate) unsafe fn pn53x_check_communication_bridge(_device: *mut nfc_device) -> c_int {
    let mut state = test_state().lock().unwrap();
    state.check_communication_calls += 1;
    state
        .check_communication_results
        .pop_front()
        .unwrap_or(NFC_SUCCESS)
}

#[cfg(test)]
pub(crate) unsafe fn pn53x_data_new_bridge(
    device: *mut nfc_device,
    io: *const pn53x_io,
) -> *mut c_void {
    let Some(device) = (unsafe { as_mut(device) }) else {
        return ptr::null_mut();
    };
    if io.is_null() {
        return ptr::null_mut();
    }
    let mut chip = Box::new(pn53x_data {
        type_: pn53x_type::PN53X,
        firmware_text: [0; 22],
        power_mode: pn53x_power_mode::NORMAL,
        operating_mode: pn53x_operating_mode::IDLE,
        current_target: ptr::null_mut(),
        sam_mode: pn532_sam_mode::PSM_NORMAL,
        io,
        last_status_byte: 0,
        ui8TxBits: 0,
        ui8Parameters: 0,
        last_command: 0,
        timer_correction: 0,
        timer_prescaler: 0,
        wb_data: [0; 0x18],
        wb_mask: [0; 0x18],
        wb_trigged: false,
        timeout_command: 0,
        timeout_atr: 0,
        timeout_communication: 0,
        supported_modulation_as_initiator: ptr::null_mut(),
        supported_modulation_as_target: ptr::null_mut(),
        progressive_field: false,
    });
    let chip_ptr = chip.as_mut() as *mut pn53x_data;
    device.chip_data = Box::into_raw(chip).cast::<c_void>();
    test_state().lock().unwrap().data_new_calls += 1;
    chip_ptr.cast::<c_void>()
}

#[cfg(test)]
pub(crate) unsafe fn pn53x_data_free_bridge(device: *mut nfc_device) {
    let Some(device) = (unsafe { as_mut(device) }) else {
        return;
    };
    if !device.chip_data.is_null() {
        unsafe {
            drop(Box::from_raw(device.chip_data.cast::<pn53x_data>()));
        }
        device.chip_data = ptr::null_mut();
    }
    test_state().lock().unwrap().data_free_calls += 1;
}

#[cfg(test)]
pub(crate) unsafe fn pn53x_init_bridge(_device: *mut nfc_device) -> c_int {
    test_state().lock().unwrap().init_calls += 1;
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe fn pn532_sam_configuration_bridge(
    device: *mut nfc_device,
    _mode: pn532_sam_mode,
    _timeout: c_int,
) -> c_int {
    test_state().lock().unwrap().sam_configuration_calls += 1;
    if let Some(chip) = unsafe { chip_data(device) } {
        chip.power_mode = pn53x_power_mode::NORMAL;
    }
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_strerror_callback(
    _device: *const nfc_device,
) -> *const c_char {
    c"test pn53x error".as_ptr()
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_initiator_init_callback(_device: *mut nfc_device) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn532_initiator_init_secure_element_callback(
    _device: *mut nfc_device,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_initiator_select_passive_target_callback(
    _device: *mut nfc_device,
    _modulation: nfc_modulation,
    _init_data: *const u8,
    _init_data_len: usize,
    _target: *mut nfc_target,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_initiator_poll_target_callback(
    _device: *mut nfc_device,
    _modulations: *const nfc_modulation,
    _modulation_count: usize,
    _poll_nr: u8,
    _period: u8,
    _target: *mut nfc_target,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_initiator_select_dep_target_callback(
    _device: *mut nfc_device,
    _dep_mode: nfc_dep_mode,
    _baud_rate: nfc_baud_rate,
    _initiator: *const nfc_dep_info,
    _target: *mut nfc_target,
    _timeout: c_int,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_initiator_deselect_target_callback(
    _device: *mut nfc_device,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_initiator_transceive_bytes_callback(
    _device: *mut nfc_device,
    _tx: *const u8,
    _tx_len: usize,
    _rx: *mut u8,
    _rx_len: usize,
    _timeout: c_int,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_initiator_transceive_bits_callback(
    _device: *mut nfc_device,
    _tx: *const u8,
    _tx_bits_len: usize,
    _tx_parity: *const u8,
    _rx: *mut u8,
    _rx_parity: *mut u8,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_initiator_transceive_bytes_timed_callback(
    _device: *mut nfc_device,
    _tx: *const u8,
    _tx_len: usize,
    _rx: *mut u8,
    _rx_len: usize,
    _cycles: *mut u32,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_initiator_transceive_bits_timed_callback(
    _device: *mut nfc_device,
    _tx: *const u8,
    _tx_bits_len: usize,
    _tx_parity: *const u8,
    _rx: *mut u8,
    _rx_parity: *mut u8,
    _cycles: *mut u32,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_initiator_target_is_present_callback(
    _device: *mut nfc_device,
    _target: *const nfc_target,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_target_init_callback(
    _device: *mut nfc_device,
    _target: *mut nfc_target,
    _rx: *mut u8,
    _rx_len: usize,
    _timeout: c_int,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_target_send_bytes_callback(
    _device: *mut nfc_device,
    _tx: *const u8,
    _tx_len: usize,
    _timeout: c_int,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_target_receive_bytes_callback(
    _device: *mut nfc_device,
    _rx: *mut u8,
    _rx_len: usize,
    _timeout: c_int,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_target_send_bits_callback(
    _device: *mut nfc_device,
    _tx: *const u8,
    _tx_bits_len: usize,
    _tx_parity: *const u8,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_target_receive_bits_callback(
    _device: *mut nfc_device,
    _rx: *mut u8,
    _rx_len: usize,
    _rx_parity: *mut u8,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_set_property_bool_callback(
    _device: *mut nfc_device,
    _property: nfc_property,
    _enable: bool,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_set_property_int_callback(
    _device: *mut nfc_device,
    _property: nfc_property,
    _value: c_int,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_get_supported_modulation_callback(
    _device: *mut nfc_device,
    mode: nfc_mode,
    supported: *mut *const nfc_modulation_type,
) -> c_int {
    test_state()
        .lock()
        .unwrap()
        .supported_modulation_calls
        .push(mode);
    let Some(supported) = (unsafe { as_mut(supported) }) else {
        return NFC_EINVARG;
    };
    *supported = TEST_MODULATION_TYPES.as_ptr();
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_get_supported_baud_rate_callback(
    _device: *mut nfc_device,
    mode: nfc_mode,
    modulation_type: nfc_modulation_type,
    supported: *mut *const nfc_baud_rate,
) -> c_int {
    test_state()
        .lock()
        .unwrap()
        .supported_baud_rate_calls
        .push((mode, modulation_type));
    let Some(supported) = (unsafe { as_mut(supported) }) else {
        return NFC_EINVARG;
    };
    *supported = TEST_BAUD_RATES.as_ptr();
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_get_information_about_callback(
    _device: *mut nfc_device,
    buffer: *mut *mut c_char,
) -> c_int {
    let Some(buffer) = (unsafe { as_mut(buffer) }) else {
        return NFC_EINVARG;
    };
    let payload = b"test pn53x info";
    let allocation = unsafe { libc::malloc(payload.len() + 1) as *mut c_char };
    if allocation.is_null() {
        return NFC_EIO;
    }
    if !unsafe { copy_bytes_to_c_buffer(allocation, payload.len() + 1, payload) } {
        unsafe { crate::release_allocated_ptr(allocation.cast::<c_void>()) };
        return NFC_EIO;
    }
    *buffer = allocation;
    test_state().lock().unwrap().info_requests += 1;
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_idle_callback(device: *mut nfc_device) -> c_int {
    if let Some(chip) = unsafe { chip_data(device) } {
        chip.power_mode = pn53x_power_mode::NORMAL;
    }
    NFC_SUCCESS
}

#[cfg(test)]
pub(crate) unsafe extern "C" fn pn53x_powerdown_callback(device: *mut nfc_device) -> c_int {
    if let Some(chip) = unsafe { chip_data(device) } {
        chip.power_mode = pn53x_power_mode::POWERDOWN;
    }
    NFC_SUCCESS
}
