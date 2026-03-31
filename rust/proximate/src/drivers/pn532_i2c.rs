// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Rust-owned PN532 I2C driver.

#![allow(non_camel_case_types)]

use crate::buses::i2c::{i2c_close, i2c_device, i2c_list_ports, i2c_open, i2c_read, i2c_write};
use crate::buses::{invalid_i2c_address, invalid_i2c_bus};
use crate::drivers::pn53x_native::{
    NFC_EIO, NFC_EOPABORTED, NFC_ETIMEOUT, NFC_SUCCESS, PN53X_EXTENDED_FRAME_DATA_MAX_LEN,
    PN53X_PREAMBLE_AND_START, PN532_BUFFER_LEN, PN532_I2C_ADDR, PN532_TIMER_CORRECTION, chip_data,
    free_decode_param, pn53x_ack_frame_bytes, pn53x_build_frame_bridge,
    pn53x_check_ack_frame_bridge, pn53x_check_communication_bridge, pn53x_data_free_bridge,
    pn53x_data_new_bridge, pn53x_get_information_about_callback,
    pn53x_get_supported_baud_rate_callback, pn53x_get_supported_modulation_callback,
    pn53x_idle_callback, pn53x_init_bridge, pn53x_initiator_deselect_target_callback,
    pn53x_initiator_init_callback, pn53x_initiator_poll_target_callback,
    pn53x_initiator_select_dep_target_callback, pn53x_initiator_select_passive_target_callback,
    pn53x_initiator_target_is_present_callback, pn53x_initiator_transceive_bits_callback,
    pn53x_initiator_transceive_bits_timed_callback, pn53x_initiator_transceive_bytes_callback,
    pn53x_initiator_transceive_bytes_timed_callback, pn53x_io, pn53x_power_mode,
    pn53x_powerdown_callback, pn53x_set_property_bool_callback, pn53x_set_property_int_callback,
    pn53x_strerror_callback, pn53x_target_init_callback, pn53x_target_receive_bits_callback,
    pn53x_target_receive_bytes_callback, pn53x_target_send_bits_callback,
    pn53x_target_send_bytes_callback, pn53x_type, pn532_initiator_init_secure_element_callback,
    pn532_sam_configuration_bridge, pn532_sam_mode,
};
#[cfg(test)]
use crate::drivers::pn53x_native::{
    test_queue_check_communication_result as test_queue_check_communication_result_native,
    test_reset as test_reset_native, test_snapshot as test_snapshot_native,
    test_lock as test_lock_native,
};
use crate::ffi_support::{as_mut, copy_bytes_to_c_buffer};
use crate::lifecycle::{
    DEVICE_NAME_LENGTH, nfc_connstring, nfc_context, nfc_device, nfc_device_free, nfc_device_new,
    nfc_driver, scan_type_enum,
};
use crate::{NFC_BUFSIZE_CONNSTRING, connstring_decode};
use libc::{c_char, c_int, c_void, ssize_t};
use std::ffi::CString;
use std::ptr;
use std::slice;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const PN532_I2C_DRIVER_NAME: &[u8] = b"pn532_i2c";
const PN532_I2C_DRIVER_NAME_CSTR: *const c_char = b"pn532_i2c\0" as *const u8 as *const c_char;
const PN532_SEND_RETRIES: u8 = 3;
const PN532_BUS_FREE_TIME_MS: u64 = 5;

#[repr(C)]
struct Pn532I2cData {
    dev: i2c_device,
    abort_flag: bool,
}

fn driver_ptr() -> *const nfc_driver {
    ptr::addr_of!(PN532_I2C_DRIVER)
}

#[cfg(libnfc_driver_pn532_i2c)]
pub(crate) fn builtin_driver_ptr() -> *const nfc_driver {
    driver_ptr()
}

unsafe fn driver_data<'a>(device: *mut nfc_device) -> Option<&'a mut Pn532I2cData> {
    let device = unsafe { as_mut(device) }?;
    unsafe { as_mut(device.driver_data.cast::<Pn532I2cData>()) }
}

unsafe fn alloc_driver_data(device: *mut nfc_device) -> bool {
    let Some(device) = (unsafe { as_mut(device) }) else {
        return false;
    };
    if !device.driver_data.is_null() {
        return true;
    }
    let allocation = unsafe { libc::calloc(1, std::mem::size_of::<Pn532I2cData>()) };
    if allocation.is_null() {
        unsafe { libc::perror(crate::MALLOC_LABEL) };
        return false;
    }
    device.driver_data = allocation;
    true
}

fn decode_descriptor(connstring: *const c_char) -> Option<String> {
    let mut bus = ptr::null_mut();
    let level = unsafe {
        connstring_decode(
            connstring,
            PN532_I2C_DRIVER_NAME_CSTR,
            ptr::null(),
            ptr::addr_of_mut!(bus),
            ptr::null_mut(),
        )
    };
    if level < 2 || bus.is_null() {
        unsafe { free_decode_param(bus) };
        return None;
    }
    let bus_name = unsafe { std::ffi::CStr::from_ptr(bus) }
        .to_string_lossy()
        .into_owned();
    unsafe { free_decode_param(bus) };
    Some(bus_name)
}

fn copy_scan_connstring(connstrings: *mut nfc_connstring, index: usize, bytes: &[u8]) -> bool {
    unsafe {
        copy_bytes_to_c_buffer(
            connstrings.add(index).cast::<c_char>(),
            NFC_BUFSIZE_CONNSTRING,
            bytes,
        )
    }
}

unsafe fn free_port_list(list: *mut *mut c_char) {
    if list.is_null() {
        return;
    }
    let mut index = 0usize;
    loop {
        let entry = unsafe { *list.add(index) };
        if entry.is_null() {
            break;
        }
        unsafe { crate::release_allocated_ptr(entry.cast::<c_void>()) };
        index += 1;
    }
    unsafe { crate::release_allocated_ptr(list.cast::<c_void>()) };
}

unsafe fn configure_device_common(device: *mut nfc_device, io: *const pn53x_io) -> bool {
    if unsafe { pn53x_data_new_bridge(device, io) }.is_null() {
        return false;
    }
    if let Some(chip) = unsafe { chip_data(device) } {
        chip.type_ = pn53x_type::PN532;
        chip.power_mode = pn53x_power_mode::LOWVBAT;
        chip.timer_correction = PN532_TIMER_CORRECTION;
    }
    true
}

fn last_transaction_stop() -> &'static Mutex<Option<Instant>> {
    static STATE: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(None))
}

fn respect_bus_free_time() {
    let mut guard = last_transaction_stop().lock().unwrap();
    if let Some(last) = *guard {
        let elapsed = last.elapsed();
        let required = Duration::from_millis(PN532_BUS_FREE_TIME_MS);
        if elapsed < required {
            std::thread::sleep(required - elapsed);
        }
    }
    *guard = Some(Instant::now());
}

unsafe fn pn532_i2c_read_bus(device: i2c_device, buffer: *mut u8, buffer_len: usize) -> ssize_t {
    respect_bus_free_time();
    let rc = unsafe { i2c_read(device, buffer, buffer_len) };
    *last_transaction_stop().lock().unwrap() = Some(Instant::now());
    rc
}

unsafe fn pn532_i2c_write_bus(device: i2c_device, buffer: *const u8, buffer_len: usize) -> c_int {
    respect_bus_free_time();
    let rc = unsafe { i2c_write(device, buffer, buffer_len) };
    *last_transaction_stop().lock().unwrap() = Some(Instant::now());
    rc
}

unsafe extern "C" fn pn532_i2c_scan(
    context: *const nfc_context,
    connstrings: *mut nfc_connstring,
    connstrings_len: usize,
) -> usize {
    if connstrings.is_null() || connstrings_len == 0 {
        return 0;
    }
    let list = unsafe { i2c_list_ports() };
    if list.is_null() {
        return 0;
    }

    let mut found = 0usize;
    let mut index = 0usize;
    loop {
        let entry = unsafe { *list.add(index) };
        if entry.is_null() {
            break;
        }

        let handle = unsafe { i2c_open(entry, PN532_I2C_ADDR) };
        if handle == invalid_i2c_bus() || handle == invalid_i2c_address() {
            index += 1;
            continue;
        }

        let bus_name = unsafe { std::ffi::CStr::from_ptr(entry) }
            .to_string_lossy()
            .into_owned();
        let connstring_bytes = format!(
            "{}:{}",
            String::from_utf8_lossy(PN532_I2C_DRIVER_NAME),
            bus_name
        )
        .into_bytes();
        let Ok(full_connstring) = CString::new(connstring_bytes.clone()) else {
            unsafe { i2c_close(handle) };
            index += 1;
            continue;
        };
        let device = unsafe { nfc_device_new(context, full_connstring.as_ptr()) };
        if device.is_null() || !unsafe { alloc_driver_data(device) } {
            unsafe {
                i2c_close(handle);
                if !device.is_null() {
                    nfc_device_free(device);
                }
            }
            index += 1;
            continue;
        }

        unsafe {
            (*device).driver = driver_ptr();
            (*device)
                .driver_data
                .cast::<Pn532I2cData>()
                .write(Pn532I2cData {
                    dev: handle,
                    abort_flag: false,
                });
        }
        let ok = if unsafe { configure_device_common(device, ptr::addr_of!(PN532_I2C_IO)) } {
            (unsafe { pn53x_check_communication_bridge(device) }) >= 0
        } else {
            false
        };
        unsafe {
            if let Some(data) = driver_data(device) {
                i2c_close(data.dev);
            }
            pn53x_data_free_bridge(device);
            nfc_device_free(device);
        }
        if ok && copy_scan_connstring(connstrings, found, &connstring_bytes) {
            found += 1;
            if found >= connstrings_len {
                break;
            }
        }
        index += 1;
    }

    unsafe { free_port_list(list) };
    found
}

unsafe extern "C" fn pn532_i2c_close(device: *mut nfc_device) {
    if device.is_null() {
        return;
    }
    unsafe {
        pn53x_idle_callback(device);
        if let Some(data) = driver_data(device) {
            i2c_close(data.dev);
        }
        pn53x_data_free_bridge(device);
        nfc_device_free(device);
    }
}

unsafe extern "C" fn pn532_i2c_open(
    context: *const nfc_context,
    connstring: *const c_char,
) -> *mut nfc_device {
    let Some(bus_name) = decode_descriptor(connstring) else {
        return ptr::null_mut();
    };
    let Ok(bus_c) = CString::new(bus_name.clone()) else {
        return ptr::null_mut();
    };
    let handle = unsafe { i2c_open(bus_c.as_ptr(), PN532_I2C_ADDR) };
    if handle == invalid_i2c_bus() || handle == invalid_i2c_address() {
        return ptr::null_mut();
    }

    let device = unsafe { nfc_device_new(context, connstring) };
    if device.is_null() || !unsafe { alloc_driver_data(device) } {
        unsafe {
            i2c_close(handle);
            if !device.is_null() {
                nfc_device_free(device);
            }
        }
        return ptr::null_mut();
    }

    let name = format!(
        "{}:{}",
        String::from_utf8_lossy(PN532_I2C_DRIVER_NAME),
        bus_name
    );
    unsafe {
        copy_bytes_to_c_buffer(
            (*device).name.as_mut_ptr(),
            DEVICE_NAME_LENGTH,
            name.as_bytes(),
        );
        (*device).driver = driver_ptr();
        (*device)
            .driver_data
            .cast::<Pn532I2cData>()
            .write(Pn532I2cData {
                dev: handle,
                abort_flag: false,
            });
    }

    if !unsafe { configure_device_common(device, ptr::addr_of!(PN532_I2C_IO)) }
        || unsafe { pn53x_check_communication_bridge(device) } < 0
    {
        unsafe { pn532_i2c_close(device) };
        return ptr::null_mut();
    }

    unsafe { pn53x_init_bridge(device) };
    device
}

unsafe extern "C" fn pn532_i2c_wakeup(device: *mut nfc_device) -> c_int {
    if let Some(chip) = unsafe { chip_data(device) } {
        chip.power_mode = pn53x_power_mode::NORMAL;
    }
    NFC_SUCCESS
}

unsafe extern "C" fn pn532_i2c_send(
    device: *mut nfc_device,
    data: *const u8,
    data_len: usize,
    timeout: c_int,
) -> c_int {
    match unsafe { chip_data(device).map(|chip| chip.power_mode) } {
        Some(pn53x_power_mode::LOWVBAT) => {
            let rc = unsafe { pn532_i2c_wakeup(device) };
            if rc < 0 {
                unsafe { (*device).last_error = rc };
                return rc;
            }
            let rc =
                unsafe { pn532_sam_configuration_bridge(device, pn532_sam_mode::PSM_NORMAL, 1000) };
            if rc < 0 {
                unsafe { (*device).last_error = rc };
                return rc;
            }
        }
        Some(pn53x_power_mode::POWERDOWN) => {
            let rc = unsafe { pn532_i2c_wakeup(device) };
            if rc < 0 {
                unsafe { (*device).last_error = rc };
                return rc;
            }
        }
        _ => {}
    }

    let Some(driver_data) = (unsafe { driver_data(device) }) else {
        return NFC_EIO;
    };
    let mut frame = [0u8; PN532_BUFFER_LEN];
    frame[..PN53X_PREAMBLE_AND_START.len()].copy_from_slice(&PN53X_PREAMBLE_AND_START);
    let mut frame_len = 0usize;
    let build_rc = unsafe {
        pn53x_build_frame_bridge(
            frame.as_mut_ptr(),
            ptr::addr_of_mut!(frame_len),
            data,
            data_len,
        )
    };
    if build_rc < 0 {
        unsafe { (*device).last_error = build_rc };
        return build_rc;
    }

    let mut write_rc = NFC_EIO;
    for _ in 0..PN532_SEND_RETRIES {
        write_rc = unsafe { pn532_i2c_write_bus(driver_data.dev, frame.as_ptr(), frame_len) };
        if write_rc >= 0 {
            break;
        }
    }
    if write_rc < 0 {
        unsafe { (*device).last_error = write_rc };
        return write_rc;
    }

    let mut ack = [0u8; 6];
    let ready_rc = unsafe { pn532_i2c_wait_rdyframe(device, ack.as_mut_ptr(), ack.len(), timeout) };
    if ready_rc < 0 {
        if ready_rc == NFC_EOPABORTED {
            unsafe { pn532_i2c_ack(device) };
        }
        unsafe { (*device).last_error = ready_rc };
        return ready_rc;
    }

    let ack_rc = unsafe { pn53x_check_ack_frame_bridge(device, ack.as_ptr(), ready_rc as usize) };
    unsafe { (*device).last_error = ack_rc };
    ack_rc
}

unsafe fn pn532_i2c_wait_rdyframe(
    device: *mut nfc_device,
    data: *mut u8,
    data_len: usize,
    timeout: c_int,
) -> c_int {
    let Some(driver_data) = (unsafe { driver_data(device) }) else {
        return NFC_EIO;
    };
    let start = Instant::now();
    let mut i2c_rx = vec![0u8; data_len + 1];

    loop {
        let rec_count =
            unsafe { pn532_i2c_read_bus(driver_data.dev, i2c_rx.as_mut_ptr(), i2c_rx.len()) };
        if driver_data.abort_flag {
            driver_data.abort_flag = false;
            return NFC_EOPABORTED;
        }
        if rec_count <= 0 {
            return NFC_EIO;
        }

        if (i2c_rx[0] & 1) != 0 {
            let payload_len = (rec_count - 1) as usize;
            let copy_len = payload_len.min(data_len);
            unsafe {
                ptr::copy_nonoverlapping(i2c_rx[1..].as_ptr(), data, copy_len);
            }
            return payload_len as c_int;
        }

        if timeout > 0 && start.elapsed() > Duration::from_millis(timeout as u64) {
            return NFC_ETIMEOUT;
        }
    }
}

unsafe extern "C" fn pn532_i2c_receive(
    device: *mut nfc_device,
    data: *mut u8,
    data_len: usize,
    timeout: c_int,
) -> c_int {
    let mut frame = [0u8; PN53X_EXTENDED_FRAME_DATA_MAX_LEN];
    let frame_len =
        unsafe { pn532_i2c_wait_rdyframe(device, frame.as_mut_ptr(), frame.len(), timeout) };
    if frame_len == NFC_EOPABORTED {
        return unsafe { pn532_i2c_ack(device) };
    }
    if frame_len < 0 {
        unsafe { (*device).last_error = frame_len };
        return frame_len;
    }

    if frame[..3] != PN53X_PREAMBLE_AND_START {
        unsafe { (*device).last_error = NFC_EIO };
        return NFC_EIO;
    }

    let tfi_index;
    let len;
    if frame[3] == 0x01 && frame[4] == 0xff {
        unsafe { (*device).last_error = NFC_EIO };
        return NFC_EIO;
    } else if frame[3] == 0xff && frame[4] == 0xff {
        len = ((frame[5] as usize) << 8) + frame[6] as usize;
        if frame[5].wrapping_add(frame[6]).wrapping_add(frame[7]) != 0 {
            unsafe { (*device).last_error = NFC_EIO };
            return NFC_EIO;
        }
        tfi_index = 8usize;
    } else {
        len = frame[3] as usize;
        if frame[3].wrapping_add(frame[4]) != 0 {
            unsafe { (*device).last_error = NFC_EIO };
            return NFC_EIO;
        }
        tfi_index = 5usize;
    }

    if len < 2 || len - 2 > data_len {
        unsafe { (*device).last_error = NFC_EIO };
        return NFC_EIO;
    }
    if frame[tfi_index] != 0xD5 {
        unsafe { (*device).last_error = NFC_EIO };
        return NFC_EIO;
    }

    let command = unsafe { chip_data(device).map(|chip| chip.last_command).unwrap_or(0) };
    if frame[tfi_index + 1] != command.wrapping_add(1) {
        unsafe { (*device).last_error = NFC_EIO };
        return NFC_EIO;
    }

    let dcs = frame[tfi_index + len];
    let mut calc = dcs;
    for byte in &frame[tfi_index..tfi_index + len] {
        calc = calc.wrapping_add(*byte);
    }
    if calc != 0 || frame[tfi_index + len + 1] != 0x00 {
        unsafe { (*device).last_error = NFC_EIO };
        return NFC_EIO;
    }

    unsafe {
        ptr::copy_nonoverlapping(
            frame[tfi_index + 2..tfi_index + len].as_ptr(),
            data,
            len - 2,
        );
        (*device).last_error = (len - 2) as c_int;
    }
    (len - 2) as c_int
}

unsafe extern "C" fn pn532_i2c_ack(device: *mut nfc_device) -> c_int {
    let Some(driver_data) = (unsafe { driver_data(device) }) else {
        return NFC_EIO;
    };
    let ack = pn53x_ack_frame_bytes();
    unsafe { pn532_i2c_write_bus(driver_data.dev, ack.as_ptr(), ack.len()) }
}

unsafe extern "C" fn pn532_i2c_abort_command(device: *mut nfc_device) -> c_int {
    if let Some(driver_data) = unsafe { driver_data(device) } {
        driver_data.abort_flag = true;
    }
    NFC_SUCCESS
}

static PN532_I2C_IO: pn53x_io = pn53x_io {
    send: Some(pn532_i2c_send),
    receive: Some(pn532_i2c_receive),
};

static PN532_I2C_DRIVER: nfc_driver = nfc_driver {
    name: PN532_I2C_DRIVER_NAME_CSTR,
    scan_type: scan_type_enum::INTRUSIVE,
    scan: Some(pn532_i2c_scan),
    open: Some(pn532_i2c_open),
    close: Some(pn532_i2c_close),
    strerror: Some(pn53x_strerror_callback),
    initiator_init: Some(pn53x_initiator_init_callback),
    initiator_init_secure_element: Some(pn532_initiator_init_secure_element_callback),
    initiator_select_passive_target: Some(pn53x_initiator_select_passive_target_callback),
    initiator_poll_target: Some(pn53x_initiator_poll_target_callback),
    initiator_select_dep_target: Some(pn53x_initiator_select_dep_target_callback),
    initiator_deselect_target: Some(pn53x_initiator_deselect_target_callback),
    initiator_transceive_bytes: Some(pn53x_initiator_transceive_bytes_callback),
    initiator_transceive_bits: Some(pn53x_initiator_transceive_bits_callback),
    initiator_transceive_bytes_timed: Some(pn53x_initiator_transceive_bytes_timed_callback),
    initiator_transceive_bits_timed: Some(pn53x_initiator_transceive_bits_timed_callback),
    initiator_target_is_present: Some(pn53x_initiator_target_is_present_callback),
    target_init: Some(pn53x_target_init_callback),
    target_send_bytes: Some(pn53x_target_send_bytes_callback),
    target_receive_bytes: Some(pn53x_target_receive_bytes_callback),
    target_send_bits: Some(pn53x_target_send_bits_callback),
    target_receive_bits: Some(pn53x_target_receive_bits_callback),
    device_set_property_bool: Some(pn53x_set_property_bool_callback),
    device_set_property_int: Some(pn53x_set_property_int_callback),
    get_supported_modulation: Some(pn53x_get_supported_modulation_callback),
    get_supported_baud_rate: Some(pn53x_get_supported_baud_rate_callback),
    device_get_information_about: Some(pn53x_get_information_about_callback),
    abort_command: Some(pn532_i2c_abort_command),
    idle: Some(pn53x_idle_callback),
    powerdown: Some(pn53x_powerdown_callback),
};

#[cfg(test)]
pub(crate) use crate::buses::i2c::{
    test_add_bus, test_queue_rx as test_queue_rx_i2c, test_reset as test_reset_i2c, test_take_tx,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::nfc_context;

    fn context() -> *mut nfc_context {
        unsafe { crate::nfc_context_new() }
    }

    #[test]
    fn open_send_receive_and_abort() {
        let _guard = test_lock_native();
        test_reset_i2c();
        test_reset_native();
        test_add_bus("/dev/i2c-1", true, false);
        test_queue_check_communication_result_native(NFC_SUCCESS);

        let ctx = context();
        let connstring = CString::new("pn532_i2c:/dev/i2c-1").unwrap();
        let device = unsafe { pn532_i2c_open(ctx, connstring.as_ptr()) };
        assert!(!device.is_null());

        let mut ack_frame = Vec::with_capacity(7);
        ack_frame.push(0x01);
        ack_frame.extend_from_slice(pn53x_ack_frame_bytes());
        test_queue_rx_i2c("/dev/i2c-1", &ack_frame);
        let tx = [0x02, 0x00];
        assert_eq!(
            unsafe { pn532_i2c_send(device, tx.as_ptr(), tx.len(), 50) },
            NFC_SUCCESS
        );
        assert!(!test_take_tx("/dev/i2c-1").is_empty());

        test_queue_rx_i2c(
            "/dev/i2c-1",
            &[
                0x01, 0x00, 0x00, 0xff, 0x03, 0xfd, 0xD5, 0x03, 0x28, 0x00, 0x00,
            ],
        );
        if let Some(chip) = unsafe { chip_data(device) } {
            chip.last_command = 0x02;
        }
        let mut rx = [0u8; 1];
        assert_eq!(
            unsafe { pn532_i2c_receive(device, rx.as_mut_ptr(), rx.len(), 50) },
            1
        );
        assert_eq!(rx, [0x28]);

        assert_eq!(unsafe { pn532_i2c_abort_command(device) }, NFC_SUCCESS);
        assert_eq!(
            unsafe { pn532_i2c_wait_rdyframe(device, rx.as_mut_ptr(), rx.len(), 50) },
            NFC_EOPABORTED
        );

        unsafe { pn532_i2c_close(device) };
        let native = test_snapshot_native();
        assert_eq!(native.data_new_calls, 1);
        assert_eq!(native.data_free_calls, 1);
        unsafe { crate::nfc_context_free(ctx) };
    }
}
