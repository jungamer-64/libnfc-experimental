// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Rust-owned PN532 SPI driver.

#![allow(non_camel_case_types)]

use crate::buses::spi::{
    spi_close, spi_get_speed, spi_list_ports, spi_open, spi_port, spi_receive, spi_send,
    spi_send_receive, spi_set_mode, spi_set_speed,
};
use crate::buses::{claimed_spi_port, invalid_spi_port};
use crate::c_api_impl::{NFC_BUFSIZE_CONNSTRING, connstring_decode};
use crate::drivers::pn53x_native::{
    NFC_EIO, NFC_EOPABORTED, NFC_ETIMEOUT, NFC_SUCCESS, PN532_BUFFER_LEN, PN532_TIMER_CORRECTION,
    chip_data, free_decode_param, pn53x_ack_frame_bytes, pn53x_build_frame_bridge,
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
    test_lock as test_lock_native,
    test_queue_check_communication_result as test_queue_check_communication_result_native,
    test_reset as test_reset_native, test_snapshot as test_snapshot_native,
};
use crate::ffi_support::{as_mut, copy_bytes_to_c_buffer};
use crate::lifecycle::{
    DEVICE_NAME_LENGTH, nfc_connstring, nfc_context, nfc_device, nfc_device_free, nfc_device_new,
    nfc_driver, scan_type_enum,
};
use libc::{c_char, c_int, c_void};
use std::ffi::CString;
use std::ptr;
use std::slice;
use std::time::Duration;

const PN532_SPI_DEFAULT_SPEED: u32 = 1_000_000;
const PN532_SPI_MODE: u32 = 0;
const PN532_SPI_DRIVER_NAME: &[u8] = b"pn532_spi";
const PN532_SPI_DRIVER_NAME_CSTR: *const c_char = b"pn532_spi\0" as *const u8 as *const c_char;
const PN532_SPI_CMD_DATAREAD: u8 = 0x03;
const PN532_SPI_CMD_DATAWRITE: u8 = 0x01;
const PN532_SPI_CMD_STATREAD: u8 = 0x02;

#[repr(C)]
struct Pn532SpiData {
    port: spi_port,
    abort_flag: bool,
}

#[derive(Clone)]
struct Pn532SpiDescriptor {
    port: String,
    speed: u32,
}

fn driver_ptr() -> *const nfc_driver {
    ptr::addr_of!(PN532_SPI_DRIVER)
}

#[cfg(libnfc_driver_pn532_spi)]
pub(crate) fn builtin_driver_ptr() -> *const nfc_driver {
    driver_ptr()
}

unsafe fn driver_data<'a>(device: *mut nfc_device) -> Option<&'a mut Pn532SpiData> {
    let device = unsafe { as_mut(device) }?;
    unsafe { as_mut(device.driver_data.cast::<Pn532SpiData>()) }
}

unsafe fn alloc_driver_data(device: *mut nfc_device) -> bool {
    let Some(device) = (unsafe { as_mut(device) }) else {
        return false;
    };
    if !device.driver_data.is_null() {
        return true;
    }
    let allocation = unsafe { libc::calloc(1, std::mem::size_of::<Pn532SpiData>()) };
    if allocation.is_null() {
        unsafe { libc::perror(crate::MALLOC_LABEL) };
        return false;
    }
    device.driver_data = allocation;
    true
}

fn decode_descriptor(connstring: *const c_char) -> Option<Pn532SpiDescriptor> {
    let mut port = ptr::null_mut();
    let mut speed = ptr::null_mut();
    let level = unsafe {
        connstring_decode(
            connstring,
            PN532_SPI_DRIVER_NAME_CSTR,
            ptr::null(),
            ptr::addr_of_mut!(port),
            ptr::addr_of_mut!(speed),
        )
    };
    if level < 2 || port.is_null() {
        unsafe {
            free_decode_param(port);
            free_decode_param(speed);
        }
        return None;
    }
    let port_value = unsafe { std::ffi::CStr::from_ptr(port) }
        .to_string_lossy()
        .into_owned();
    let speed_value = if level >= 3 && !speed.is_null() {
        unsafe { std::ffi::CStr::from_ptr(speed) }
            .to_string_lossy()
            .parse()
            .ok()?
    } else {
        PN532_SPI_DEFAULT_SPEED
    };
    unsafe {
        free_decode_param(port);
        free_decode_param(speed);
    }
    Some(Pn532SpiDescriptor {
        port: port_value,
        speed: speed_value,
    })
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

unsafe extern "C" fn pn532_spi_scan(
    context: *const nfc_context,
    connstrings: *mut nfc_connstring,
    connstrings_len: usize,
) -> usize {
    if connstrings.is_null() || connstrings_len == 0 {
        return 0;
    }
    let list = unsafe { spi_list_ports() };
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

        let handle = unsafe { spi_open(entry) };
        if handle == invalid_spi_port() || handle == claimed_spi_port() {
            index += 1;
            continue;
        }

        unsafe {
            spi_set_speed(handle, PN532_SPI_DEFAULT_SPEED);
            spi_set_mode(handle, PN532_SPI_MODE);
        }

        let port_name = unsafe { std::ffi::CStr::from_ptr(entry) }
            .to_string_lossy()
            .into_owned();
        let connstring_bytes = format!(
            "{}:{}:{}",
            String::from_utf8_lossy(PN532_SPI_DRIVER_NAME),
            port_name,
            PN532_SPI_DEFAULT_SPEED
        )
        .into_bytes();
        let Ok(full_connstring) = CString::new(connstring_bytes.clone()) else {
            unsafe { spi_close(handle) };
            index += 1;
            continue;
        };
        let device = unsafe { nfc_device_new(context, full_connstring.as_ptr()) };
        if device.is_null() || !unsafe { alloc_driver_data(device) } {
            unsafe {
                spi_close(handle);
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
                .cast::<Pn532SpiData>()
                .write(Pn532SpiData {
                    port: handle,
                    abort_flag: false,
                });
        }
        let ok = if unsafe { configure_device_common(device, ptr::addr_of!(PN532_SPI_IO)) } {
            (unsafe { pn53x_check_communication_bridge(device) }) >= 0
        } else {
            false
        };
        unsafe {
            if let Some(data) = driver_data(device) {
                spi_close(data.port);
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

unsafe extern "C" fn pn532_spi_close(device: *mut nfc_device) {
    if device.is_null() {
        return;
    }
    unsafe {
        pn53x_idle_callback(device);
        if let Some(data) = driver_data(device) {
            spi_close(data.port);
        }
        pn53x_data_free_bridge(device);
        nfc_device_free(device);
    }
}

unsafe extern "C" fn pn532_spi_open(
    context: *const nfc_context,
    connstring: *const c_char,
) -> *mut nfc_device {
    let Some(descriptor) = decode_descriptor(connstring) else {
        return ptr::null_mut();
    };
    let Ok(port_c) = CString::new(descriptor.port.clone()) else {
        return ptr::null_mut();
    };
    let handle = unsafe { spi_open(port_c.as_ptr()) };
    if handle == invalid_spi_port() || handle == claimed_spi_port() {
        return ptr::null_mut();
    }
    unsafe {
        spi_set_speed(handle, descriptor.speed);
        spi_set_mode(handle, PN532_SPI_MODE);
    }

    let device = unsafe { nfc_device_new(context, connstring) };
    if device.is_null() || !unsafe { alloc_driver_data(device) } {
        unsafe {
            spi_close(handle);
            if !device.is_null() {
                nfc_device_free(device);
            }
        }
        return ptr::null_mut();
    }

    let name = format!(
        "{}:{}",
        String::from_utf8_lossy(PN532_SPI_DRIVER_NAME),
        descriptor.port
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
            .cast::<Pn532SpiData>()
            .write(Pn532SpiData {
                port: handle,
                abort_flag: false,
            });
    }

    if !unsafe { configure_device_common(device, ptr::addr_of!(PN532_SPI_IO)) }
        || unsafe { pn53x_check_communication_bridge(device) } < 0
    {
        unsafe { pn532_spi_close(device) };
        return ptr::null_mut();
    }

    unsafe { pn53x_init_bridge(device) };
    device
}

unsafe fn pn532_spi_read_spi_status(device: *mut nfc_device) -> c_int {
    let Some(data) = (unsafe { driver_data(device) }) else {
        return NFC_EIO;
    };
    let mut spi_status = 0u8;
    let cmd = [PN532_SPI_CMD_STATREAD];
    let rc = unsafe {
        spi_send_receive(
            data.port,
            cmd.as_ptr(),
            1,
            ptr::addr_of_mut!(spi_status),
            1,
            true,
        )
    };
    if rc != NFC_SUCCESS {
        return rc;
    }
    spi_status as c_int
}

unsafe extern "C" fn pn532_spi_wakeup(device: *mut nfc_device) -> c_int {
    let Some(data) = (unsafe { driver_data(device) }) else {
        return NFC_EIO;
    };
    let previous_speed = unsafe { spi_get_speed(data.port) };
    let mut byte = 0u8;
    let rc = unsafe { spi_receive(data.port, ptr::addr_of_mut!(byte), 1, true) };
    if rc != NFC_SUCCESS {
        return rc;
    }

    if let Some(chip) = unsafe { chip_data(device) } {
        chip.power_mode = pn53x_power_mode::NORMAL;
    }
    std::thread::sleep(Duration::from_millis(1));

    if byte == 0xff {
        unsafe { spi_set_speed(data.port, 5000) };
        let wake_rc =
            unsafe { pn532_sam_configuration_bridge(device, pn532_sam_mode::PSM_NORMAL, 1000) };
        unsafe { spi_set_speed(data.port, previous_speed) };
        return wake_rc;
    }
    NFC_SUCCESS
}

unsafe fn pn532_spi_wait_for_data(device: *mut nfc_device, timeout: c_int) -> c_int {
    const READY: c_int = 0x01;
    const POLL_INTERVAL_MS: c_int = 10;

    let mut timer = 0;
    loop {
        if let Some(data) = unsafe { driver_data(device) } {
            if data.abort_flag {
                data.abort_flag = false;
                return NFC_EOPABORTED;
            }
        }

        let status = unsafe { pn532_spi_read_spi_status(device) };
        if status == READY {
            return NFC_SUCCESS;
        }
        if status < 0 {
            return status;
        }

        if let Some(data) = unsafe { driver_data(device) } {
            if data.abort_flag {
                data.abort_flag = false;
                return NFC_EOPABORTED;
            }
        }

        if timeout > 0 {
            timer += POLL_INTERVAL_MS;
            if timer > timeout {
                return NFC_ETIMEOUT;
            }
            std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS as u64));
        }
    }
}

unsafe fn pn532_spi_receive_next_chunk(
    device: *mut nfc_device,
    data: *mut u8,
    data_len: usize,
) -> c_int {
    let Some(driver_data) = (unsafe { driver_data(device) }) else {
        return NFC_EIO;
    };
    let rc = unsafe { spi_receive(driver_data.port, data, 1, true) };
    if rc != NFC_SUCCESS {
        return rc;
    }
    unsafe {
        spi_send_receive(
            driver_data.port,
            [PN532_SPI_CMD_DATAREAD].as_ptr(),
            1,
            data.add(1),
            data_len.saturating_sub(1),
            true,
        )
    }
}

unsafe extern "C" fn pn532_spi_receive(
    device: *mut nfc_device,
    data: *mut u8,
    data_len: usize,
    timeout: c_int,
) -> c_int {
    let Some(_driver_data) = (unsafe { driver_data(device) }) else {
        return NFC_EIO;
    };
    let mut header = [0u8; 5];
    let len;

    let wait_rc = unsafe { pn532_spi_wait_for_data(device, timeout) };
    unsafe { (*device).last_error = wait_rc };
    if wait_rc == NFC_EOPABORTED {
        return unsafe { pn532_spi_ack(device) };
    }
    if wait_rc != NFC_SUCCESS {
        return wait_rc;
    }

    let rc = unsafe {
        spi_send_receive(
            driver_data(device).unwrap().port,
            [PN532_SPI_CMD_DATAREAD].as_ptr(),
            1,
            header.as_mut_ptr(),
            4,
            true,
        )
    };
    if rc < 0 {
        unsafe { (*device).last_error = rc };
        return rc;
    }

    if header[..3] == [0x00, 0x00, 0xff] {
        header[0] = header[1];
        header[1] = header[2];
        header[2] = header[3];
        let rc = unsafe { pn532_spi_receive_next_chunk(device, header[3..].as_mut_ptr(), 1) };
        if rc != 0 {
            unsafe { (*device).last_error = rc };
            return rc;
        }
    }

    if header[..2] != [0x00, 0xff] {
        unsafe { (*device).last_error = NFC_EIO };
        return NFC_EIO;
    }

    if header[2] == 0x01 && header[3] == 0xff {
        let rc = unsafe { pn532_spi_receive_next_chunk(device, header.as_mut_ptr(), 3) };
        unsafe { (*device).last_error = if rc == 0 { NFC_EIO } else { rc } };
        return if rc == 0 { NFC_EIO } else { rc };
    } else if header[2] == 0xff && header[3] == 0xff {
        let mut extended = [0u8; 3];
        let rc =
            unsafe { pn532_spi_receive_next_chunk(device, extended.as_mut_ptr(), extended.len()) };
        if rc != 0 {
            unsafe { (*device).last_error = rc };
            return rc;
        }
        len = ((extended[0] as usize) << 8) + extended[1] as usize - 2;
        if extended[0]
            .wrapping_add(extended[1])
            .wrapping_add(extended[2])
            != 0
        {
            unsafe { (*device).last_error = NFC_EIO };
            return NFC_EIO;
        }
    } else {
        if header[2].wrapping_add(header[3]) != 0 {
            unsafe { (*device).last_error = NFC_EIO };
            return NFC_EIO;
        }
        len = header[2] as usize - 2;
    }

    if len > data_len {
        unsafe { (*device).last_error = NFC_EIO };
        return NFC_EIO;
    }

    let mut tfi_cc = [0u8; 2];
    let rc = unsafe { pn532_spi_receive_next_chunk(device, tfi_cc.as_mut_ptr(), tfi_cc.len()) };
    if rc != 0 {
        unsafe { (*device).last_error = rc };
        return rc;
    }
    if tfi_cc[0] != 0xD5 {
        unsafe { (*device).last_error = NFC_EIO };
        return NFC_EIO;
    }

    let command = unsafe { chip_data(device).map(|chip| chip.last_command).unwrap_or(0) };
    if tfi_cc[1] != command.wrapping_add(1) {
        unsafe { (*device).last_error = NFC_EIO };
        return NFC_EIO;
    }

    if len > 0 {
        let rc = unsafe { pn532_spi_receive_next_chunk(device, data, len) };
        if rc != 0 {
            unsafe { (*device).last_error = rc };
            return rc;
        }
    }

    let mut trailer = [0u8; 2];
    let rc = unsafe { pn532_spi_receive_next_chunk(device, trailer.as_mut_ptr(), trailer.len()) };
    if rc != 0 {
        unsafe { (*device).last_error = rc };
        return rc;
    }

    let mut dcs = 0u8.wrapping_sub(0xD5).wrapping_sub(command.wrapping_add(1));
    for byte in unsafe { slice::from_raw_parts(data, len) } {
        dcs = dcs.wrapping_sub(*byte);
    }
    if trailer[0] != dcs || trailer[1] != 0x00 {
        unsafe { (*device).last_error = NFC_EIO };
        return NFC_EIO;
    }

    unsafe {
        (*device).last_error = len as c_int;
    }
    len as c_int
}

unsafe extern "C" fn pn532_spi_send(
    device: *mut nfc_device,
    data: *const u8,
    data_len: usize,
    timeout: c_int,
) -> c_int {
    match unsafe { chip_data(device).map(|chip| chip.power_mode) } {
        Some(pn53x_power_mode::LOWVBAT) => {
            let rc = unsafe { pn532_spi_wakeup(device) };
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
            let rc = unsafe { pn532_spi_wakeup(device) };
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
    let mut frame = [0u8; PN532_BUFFER_LEN + 1];
    frame[0] = PN532_SPI_CMD_DATAWRITE;
    frame[1..4].copy_from_slice(&[0x00, 0x00, 0xff]);
    let mut frame_len = 0usize;
    let build_rc = unsafe {
        pn53x_build_frame_bridge(
            frame[1..].as_mut_ptr(),
            ptr::addr_of_mut!(frame_len),
            data,
            data_len,
        )
    };
    if build_rc < 0 {
        unsafe { (*device).last_error = build_rc };
        return build_rc;
    }

    let rc = unsafe { spi_send(driver_data.port, frame.as_ptr(), frame_len + 1, true) };
    if rc != 0 {
        unsafe { (*device).last_error = rc };
        return rc;
    }

    let wait_rc = unsafe { pn532_spi_wait_for_data(device, timeout) };
    if wait_rc != NFC_SUCCESS {
        unsafe { (*device).last_error = wait_rc };
        return wait_rc;
    }

    let mut ack = [0u8; 6];
    let rc = unsafe {
        spi_send_receive(
            driver_data.port,
            [PN532_SPI_CMD_DATAREAD].as_ptr(),
            1,
            ack.as_mut_ptr(),
            ack.len(),
            true,
        )
    };
    if rc != 0 {
        unsafe { (*device).last_error = rc };
        return rc;
    }

    let ack_rc = unsafe { pn53x_check_ack_frame_bridge(device, ack.as_ptr(), ack.len()) };
    unsafe { (*device).last_error = ack_rc };
    ack_rc
}

unsafe extern "C" fn pn532_spi_ack(device: *mut nfc_device) -> c_int {
    let Some(data) = (unsafe { driver_data(device) }) else {
        return NFC_EIO;
    };
    let ack = pn53x_ack_frame_bytes();
    let mut tx = Vec::with_capacity(ack.len() + 1);
    tx.push(PN532_SPI_CMD_DATAWRITE);
    tx.extend_from_slice(ack);
    unsafe { spi_send(data.port, tx.as_ptr(), tx.len(), true) }
}

unsafe extern "C" fn pn532_spi_abort_command(device: *mut nfc_device) -> c_int {
    if let Some(data) = unsafe { driver_data(device) } {
        data.abort_flag = true;
    }
    NFC_SUCCESS
}

static PN532_SPI_IO: pn53x_io = pn53x_io {
    send: Some(pn532_spi_send),
    receive: Some(pn532_spi_receive),
};

static PN532_SPI_DRIVER: nfc_driver = nfc_driver {
    name: PN532_SPI_DRIVER_NAME_CSTR,
    scan_type: scan_type_enum::INTRUSIVE,
    scan: Some(pn532_spi_scan),
    open: Some(pn532_spi_open),
    close: Some(pn532_spi_close),
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
    abort_command: Some(pn532_spi_abort_command),
    idle: Some(pn53x_idle_callback),
    powerdown: Some(pn53x_powerdown_callback),
};

#[cfg(test)]
pub(crate) use crate::buses::spi::{
    test_add_port, test_queue_rx as test_queue_rx_spi_raw, test_reset as test_reset_spi,
    test_take_tx,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::{nfc_context, nfc_context_free, nfc_context_new};

    fn bit_reverse(byte: u8) -> u8 {
        let mut value = byte;
        value = ((value & 0xaa) >> 1) | ((value & 0x55) << 1);
        value = ((value & 0xcc) >> 2) | ((value & 0x33) << 2);
        ((value & 0xf0) >> 4) | ((value & 0x0f) << 4)
    }

    fn queue_logical(bytes: &[u8]) {
        let reversed = bytes.iter().copied().map(bit_reverse).collect::<Vec<_>>();
        test_queue_rx_spi_raw("/dev/spidev0.0", &reversed);
    }

    fn context() -> *mut nfc_context {
        unsafe { nfc_context_new() }
    }

    #[test]
    fn open_send_receive_and_abort() {
        let _guard = test_lock_native();
        test_reset_spi();
        test_reset_native();
        test_add_port("/dev/spidev0.0", true, false);
        test_queue_check_communication_result_native(NFC_SUCCESS);

        let ctx = context();
        let connstring = CString::new("pn532_spi:/dev/spidev0.0:1000000").unwrap();
        let device = unsafe { pn532_spi_open(ctx, connstring.as_ptr()) };
        assert!(!device.is_null());

        queue_logical(&[0x00]); // wakeup line sample
        queue_logical(&[0x01]); // status ready
        queue_logical(pn53x_ack_frame_bytes());
        let tx = [0x02, 0x00];
        assert_eq!(
            unsafe { pn532_spi_send(device, tx.as_ptr(), tx.len(), 50) },
            NFC_SUCCESS
        );
        assert!(!test_take_tx("/dev/spidev0.0").is_empty());

        queue_logical(&[0x01]); // wait ready
        queue_logical(&[0x00, 0xff, 0x03, 0xfd]); // initial 4-byte header read
        queue_logical(&[0xD5]); // first byte of TFI/CC chunk
        queue_logical(&[0x03]); // DATAREAD remainder for TFI/CC chunk
        queue_logical(&[0x28]); // payload chunk of length 1
        queue_logical(&[0x00]); // first trailer byte
        queue_logical(&[0x00]); // final DATAREAD trailer byte
        if let Some(chip) = unsafe { chip_data(device) } {
            chip.last_command = 0x02;
        }
        let mut rx = [0u8; 1];
        assert_eq!(
            unsafe { pn532_spi_receive(device, rx.as_mut_ptr(), rx.len(), 50) },
            1
        );
        assert_eq!(rx, [0x28]);

        assert_eq!(unsafe { pn532_spi_abort_command(device) }, NFC_SUCCESS);
        assert_eq!(
            unsafe { pn532_spi_wait_for_data(device, 50) },
            NFC_EOPABORTED
        );

        unsafe { pn532_spi_close(device) };
        let native = test_snapshot_native();
        assert_eq!(native.data_new_calls, 1);
        assert_eq!(native.data_free_calls, 1);
        unsafe { nfc_context_free(ctx) };
    }
}
