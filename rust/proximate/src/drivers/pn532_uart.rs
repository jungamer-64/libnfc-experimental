// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Rust-owned PN532 UART driver.

#![allow(non_camel_case_types)]

use crate::buses::uart::{
    serial_port, uart_close, uart_flush_input, uart_list_ports, uart_open, uart_receive, uart_send,
    uart_set_speed,
};
use crate::buses::{claimed_serial_port, invalid_serial_port};
use crate::drivers::pn53x_native::{
    NFC_EIO, NFC_EOPABORTED, NFC_ESOFT, NFC_SUCCESS, PN53X_PREAMBLE_AND_START, PN532_BUFFER_LEN,
    PN532_TIMER_CORRECTION, chip_data, free_decode_param, pn53x_ack_frame_bytes,
    pn53x_build_frame_bridge, pn53x_check_ack_frame_bridge, pn53x_check_communication_bridge,
    pn53x_data_free_bridge, pn53x_data_new_bridge, pn53x_get_information_about_callback,
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
use libc::{c_char, c_int, c_void};
use std::ffi::CString;
use std::ptr;
use std::slice;

const PN532_UART_DEFAULT_SPEED: u32 = 115200;
const PN532_UART_DRIVER_NAME: &[u8] = b"pn532_uart";
const PN532_UART_DRIVER_NAME_CSTR: *const c_char = b"pn532_uart\0" as *const u8 as *const c_char;

#[repr(C)]
struct Pn532UartData {
    port: serial_port,
    #[cfg(not(windows))]
    abort_fds: [c_int; 2],
    #[cfg(not(windows))]
    abort_requested: bool,
    #[cfg(windows)]
    abort_flag: bool,
}

#[derive(Clone)]
struct Pn532UartDescriptor {
    port: String,
    speed: u32,
}

fn driver_ptr() -> *const nfc_driver {
    ptr::addr_of!(PN532_UART_DRIVER)
}

#[cfg(libnfc_driver_pn532_uart)]
pub(crate) fn builtin_driver_ptr() -> *const nfc_driver {
    driver_ptr()
}

unsafe fn driver_data<'a>(device: *mut nfc_device) -> Option<&'a mut Pn532UartData> {
    let device = unsafe { as_mut(device) }?;
    unsafe { as_mut(device.driver_data.cast::<Pn532UartData>()) }
}

unsafe fn alloc_driver_data(device: *mut nfc_device) -> bool {
    let Some(device) = (unsafe { as_mut(device) }) else {
        return false;
    };
    if !device.driver_data.is_null() {
        return true;
    }
    let allocation = unsafe { libc::calloc(1, std::mem::size_of::<Pn532UartData>()) };
    if allocation.is_null() {
        unsafe { libc::perror(crate::MALLOC_LABEL) };
        return false;
    }
    device.driver_data = allocation;
    true
}

unsafe fn create_abort_pipe(data: &mut Pn532UartData) -> bool {
    #[cfg(not(windows))]
    {
        data.abort_fds = [-1, -1];
        unsafe { libc::pipe(data.abort_fds.as_mut_ptr()) == 0 }
    }
    #[cfg(windows)]
    {
        data.abort_flag = false;
        true
    }
}

unsafe fn close_abort_pipe(data: &mut Pn532UartData) {
    #[cfg(not(windows))]
    {
        for fd in &mut data.abort_fds {
            if *fd >= 0 {
                unsafe {
                    libc::close(*fd);
                }
                *fd = -1;
            }
        }
    }
    #[cfg(windows)]
    {
        data.abort_flag = false;
    }
}

unsafe fn abort_marker_ptr(data: &mut Pn532UartData) -> *mut c_void {
    #[cfg(not(windows))]
    {
        ptr::addr_of_mut!(data.abort_fds[1]).cast::<c_void>()
    }
    #[cfg(windows)]
    {
        ptr::addr_of_mut!(data.abort_flag).cast::<c_void>()
    }
}

fn decode_descriptor(connstring: *const c_char) -> Option<Pn532UartDescriptor> {
    let mut port = ptr::null_mut();
    let mut speed = ptr::null_mut();
    let level = unsafe {
        connstring_decode(
            connstring,
            PN532_UART_DRIVER_NAME_CSTR,
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
        let raw = unsafe { std::ffi::CStr::from_ptr(speed) }
            .to_string_lossy()
            .into_owned();
        raw.parse::<u32>().ok()?
    } else {
        PN532_UART_DEFAULT_SPEED
    };

    unsafe {
        free_decode_param(port);
        free_decode_param(speed);
    }

    Some(Pn532UartDescriptor {
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

unsafe extern "C" fn pn532_uart_scan(
    context: *const nfc_context,
    connstrings: *mut nfc_connstring,
    connstrings_len: usize,
) -> usize {
    if connstrings.is_null() || connstrings_len == 0 {
        return 0;
    }

    let list = unsafe { uart_list_ports() };
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

        let handle = unsafe { uart_open(entry) };
        if handle == invalid_serial_port() || handle == claimed_serial_port() {
            index += 1;
            continue;
        }

        unsafe {
            uart_flush_input(handle, true);
            uart_set_speed(handle, PN532_UART_DEFAULT_SPEED);
        }

        let port_name = unsafe { std::ffi::CStr::from_ptr(entry) }
            .to_string_lossy()
            .into_owned();
        let connstring_bytes = format!(
            "{}:{}:{}",
            String::from_utf8_lossy(PN532_UART_DRIVER_NAME),
            port_name,
            PN532_UART_DEFAULT_SPEED
        )
        .into_bytes();
        let Ok(full_connstring) = CString::new(connstring_bytes.clone()) else {
            unsafe { uart_close(handle) };
            index += 1;
            continue;
        };

        let device = unsafe { nfc_device_new(context, full_connstring.as_ptr()) };
        if device.is_null() || !unsafe { alloc_driver_data(device) } {
            unsafe {
                uart_close(handle);
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
                .cast::<Pn532UartData>()
                .write(Pn532UartData {
                    port: handle,
                    #[cfg(not(windows))]
                    abort_fds: [-1, -1],
                    #[cfg(not(windows))]
                    abort_requested: false,
                    #[cfg(windows)]
                    abort_flag: false,
                });
        }
        let scan_ok = if unsafe { configure_device_common(device, ptr::addr_of!(PN532_UART_IO)) }
            && unsafe { create_abort_pipe(driver_data(device).unwrap()) }
        {
            (unsafe { pn53x_check_communication_bridge(device) }) >= 0
        } else {
            false
        };

        unsafe {
            if let Some(data) = driver_data(device) {
                close_abort_pipe(data);
                uart_close(data.port);
            }
            pn53x_data_free_bridge(device);
            nfc_device_free(device);
        }

        if scan_ok && copy_scan_connstring(connstrings, found, &connstring_bytes) {
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

unsafe extern "C" fn pn532_uart_close(device: *mut nfc_device) {
    if device.is_null() {
        return;
    }

    unsafe {
        pn53x_idle_callback(device);
        if let Some(data) = driver_data(device) {
            uart_close(data.port);
            close_abort_pipe(data);
        }
        pn53x_data_free_bridge(device);
        nfc_device_free(device);
    }
}

unsafe extern "C" fn pn532_uart_open(
    context: *const nfc_context,
    connstring: *const c_char,
) -> *mut nfc_device {
    let Some(descriptor) = decode_descriptor(connstring) else {
        return ptr::null_mut();
    };
    let Ok(port_c) = CString::new(descriptor.port.clone()) else {
        return ptr::null_mut();
    };

    let handle = unsafe { uart_open(port_c.as_ptr()) };
    if handle == invalid_serial_port() || handle == claimed_serial_port() {
        return ptr::null_mut();
    }

    unsafe {
        uart_flush_input(handle, true);
        uart_set_speed(handle, descriptor.speed);
    }

    let device = unsafe { nfc_device_new(context, connstring) };
    if device.is_null() || !unsafe { alloc_driver_data(device) } {
        unsafe {
            uart_close(handle);
            if !device.is_null() {
                nfc_device_free(device);
            }
        }
        return ptr::null_mut();
    }

    let name_bytes = format!(
        "{}:{}",
        String::from_utf8_lossy(PN532_UART_DRIVER_NAME),
        descriptor.port
    );
    unsafe {
        copy_bytes_to_c_buffer(
            (*device).name.as_mut_ptr(),
            DEVICE_NAME_LENGTH,
            name_bytes.as_bytes(),
        );
        (*device).driver = driver_ptr();
        (*device)
            .driver_data
            .cast::<Pn532UartData>()
            .write(Pn532UartData {
                port: handle,
                #[cfg(not(windows))]
                abort_fds: [-1, -1],
                #[cfg(not(windows))]
                abort_requested: false,
                #[cfg(windows)]
                abort_flag: false,
            });
    }

    if !unsafe { create_abort_pipe(driver_data(device).unwrap()) }
        || !unsafe { configure_device_common(device, ptr::addr_of!(PN532_UART_IO)) }
        || unsafe { pn53x_check_communication_bridge(device) } < 0
    {
        unsafe { pn532_uart_close(device) };
        return ptr::null_mut();
    }

    unsafe {
        pn53x_init_bridge(device);
    }
    device
}

unsafe extern "C" fn pn532_uart_wakeup(device: *mut nfc_device) -> c_int {
    let Some(data) = (unsafe { driver_data(device) }) else {
        return NFC_EIO;
    };

    const WAKEUP: [u8; 16] = [
        0x55, 0x55, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ];
    let rc = unsafe { uart_send(data.port, WAKEUP.as_ptr(), WAKEUP.len(), 0) };
    if let Some(chip) = unsafe { chip_data(device) } {
        chip.power_mode = pn53x_power_mode::NORMAL;
    }
    rc
}

unsafe fn handle_power_mode(device: *mut nfc_device) -> c_int {
    let Some(chip) = (unsafe { chip_data(device) }) else {
        return NFC_EIO;
    };
    match chip.power_mode {
        pn53x_power_mode::LOWVBAT => {
            let rc = unsafe { pn532_uart_wakeup(device) };
            if rc < 0 {
                return rc;
            }
            unsafe { pn532_sam_configuration_bridge(device, pn532_sam_mode::PSM_NORMAL, 1000) }
        }
        pn53x_power_mode::POWERDOWN => unsafe { pn532_uart_wakeup(device) },
        pn53x_power_mode::NORMAL => NFC_SUCCESS,
    }
}

unsafe fn wait_for_ack(device: *mut nfc_device, timeout: c_int) -> c_int {
    let Some(data) = (unsafe { driver_data(device) }) else {
        return NFC_EIO;
    };
    let mut ack = [0u8; 6];
    let rc = unsafe {
        uart_receive(
            data.port,
            ack.as_mut_ptr(),
            ack.len(),
            ptr::null_mut(),
            timeout,
        )
    };
    if rc != 0 {
        unsafe {
            (*device).last_error = rc;
        }
        return rc;
    }
    let ack_rc = unsafe { pn53x_check_ack_frame_bridge(device, ack.as_ptr(), ack.len()) };
    unsafe {
        (*device).last_error = ack_rc;
    }
    ack_rc
}

unsafe extern "C" fn pn532_uart_send(
    device: *mut nfc_device,
    data: *const u8,
    data_len: usize,
    timeout: c_int,
) -> c_int {
    let Some(driver_data) = (unsafe { driver_data(device) }) else {
        return NFC_EIO;
    };

    unsafe { uart_flush_input(driver_data.port, false) };
    let rc = unsafe { handle_power_mode(device) };
    if rc < 0 {
        unsafe { (*device).last_error = rc };
        return rc;
    }

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

    let send_rc = unsafe { uart_send(driver_data.port, frame.as_ptr(), frame_len, timeout) };
    if send_rc != 0 {
        unsafe { (*device).last_error = send_rc };
        return send_rc;
    }

    unsafe { wait_for_ack(device, timeout) }
}

unsafe extern "C" fn pn532_uart_receive(
    device: *mut nfc_device,
    data: *mut u8,
    data_len: usize,
    timeout: c_int,
) -> c_int {
    let Some(driver_data) = (unsafe { driver_data(device) }) else {
        return NFC_EIO;
    };
    #[cfg(not(windows))]
    if driver_data.abort_requested {
        driver_data.abort_requested = false;
        unsafe {
            (*device).last_error = NFC_EOPABORTED;
            pn532_uart_ack(device);
        }
        return NFC_EOPABORTED;
    }
    let abort_p = unsafe { abort_marker_ptr(driver_data) };
    let mut header = [0u8; 5];
    let mut len;
    let rc = unsafe {
        uart_receive(
            driver_data.port,
            header.as_mut_ptr(),
            header.len(),
            abort_p,
            timeout,
        )
    };
    unsafe { (*device).last_error = rc };
    if rc == NFC_EOPABORTED {
        unsafe { pn532_uart_ack(device) };
        return NFC_EOPABORTED;
    }
    if rc < 0 {
        unsafe { uart_flush_input(driver_data.port, true) };
        return rc;
    }

    if header[..3] != PN53X_PREAMBLE_AND_START {
        unsafe {
            (*device).last_error = NFC_EIO;
            uart_flush_input(driver_data.port, true);
        }
        return NFC_EIO;
    }

    if header[3] == 0x01 && header[4] == 0xff {
        let mut discard = [0u8; 3];
        unsafe {
            uart_receive(
                driver_data.port,
                discard.as_mut_ptr(),
                discard.len(),
                ptr::null_mut(),
                timeout,
            );
            (*device).last_error = NFC_EIO;
            uart_flush_input(driver_data.port, true);
        }
        return NFC_EIO;
    } else if header[3] == 0xff && header[4] == 0xff {
        let mut extended = [0u8; 3];
        let rc = unsafe {
            uart_receive(
                driver_data.port,
                extended.as_mut_ptr(),
                extended.len(),
                ptr::null_mut(),
                timeout,
            )
        };
        if rc != 0 {
            unsafe {
                (*device).last_error = rc;
                uart_flush_input(driver_data.port, true);
            }
            return rc;
        }
        len = ((extended[0] as usize) << 8) + extended[1] as usize - 2;
        if (extended[0]
            .wrapping_add(extended[1])
            .wrapping_add(extended[2]))
            != 0
        {
            unsafe {
                (*device).last_error = NFC_EIO;
                uart_flush_input(driver_data.port, true);
            }
            return NFC_EIO;
        }
    } else {
        if header[3].wrapping_add(header[4]) != 0 {
            unsafe {
                (*device).last_error = NFC_EIO;
                uart_flush_input(driver_data.port, true);
            }
            return NFC_EIO;
        }
        len = header[3] as usize - 2;
    }

    if len > data_len {
        unsafe {
            (*device).last_error = NFC_EIO;
            uart_flush_input(driver_data.port, true);
        }
        return NFC_EIO;
    }

    let mut tfi_cc = [0u8; 2];
    let rc = unsafe {
        uart_receive(
            driver_data.port,
            tfi_cc.as_mut_ptr(),
            tfi_cc.len(),
            ptr::null_mut(),
            timeout,
        )
    };
    if rc != 0 {
        unsafe {
            (*device).last_error = rc;
            uart_flush_input(driver_data.port, true);
        }
        return rc;
    }
    if tfi_cc[0] != 0xD5 {
        unsafe {
            (*device).last_error = NFC_EIO;
            uart_flush_input(driver_data.port, true);
        }
        return NFC_EIO;
    }

    let command = unsafe { chip_data(device).map(|chip| chip.last_command).unwrap_or(0) };
    if tfi_cc[1] != command.wrapping_add(1) {
        unsafe {
            (*device).last_error = NFC_EIO;
            uart_flush_input(driver_data.port, true);
        }
        return NFC_EIO;
    }

    if len > 0 {
        let rc = unsafe { uart_receive(driver_data.port, data, len, ptr::null_mut(), timeout) };
        if rc != 0 {
            unsafe {
                (*device).last_error = rc;
                uart_flush_input(driver_data.port, true);
            }
            return rc;
        }
    }

    let mut trailer = [0u8; 2];
    let rc = unsafe {
        uart_receive(
            driver_data.port,
            trailer.as_mut_ptr(),
            trailer.len(),
            ptr::null_mut(),
            timeout,
        )
    };
    if rc != 0 {
        unsafe {
            (*device).last_error = rc;
            uart_flush_input(driver_data.port, true);
        }
        return rc;
    }

    let mut dcs = 0u8.wrapping_sub(0xD5).wrapping_sub(command.wrapping_add(1));
    for byte in unsafe { slice::from_raw_parts(data, len) } {
        dcs = dcs.wrapping_sub(*byte);
    }
    if trailer[0] != dcs || trailer[1] != 0x00 {
        unsafe {
            (*device).last_error = NFC_EIO;
            uart_flush_input(driver_data.port, true);
        }
        return NFC_EIO;
    }

    unsafe {
        (*device).last_error = len as c_int;
    }
    len as c_int
}

unsafe extern "C" fn pn532_uart_ack(device: *mut nfc_device) -> c_int {
    let Some(data) = (unsafe { driver_data(device) }) else {
        return NFC_EIO;
    };
    if unsafe { chip_data(device) }
        .map(|chip| chip.power_mode == pn53x_power_mode::POWERDOWN)
        .unwrap_or(false)
    {
        let rc = unsafe { pn532_uart_wakeup(device) };
        if rc < 0 {
            return rc;
        }
    }
    let ack = pn53x_ack_frame_bytes();
    unsafe { uart_send(data.port, ack.as_ptr(), ack.len(), 0) }
}

unsafe extern "C" fn pn532_uart_abort_command(device: *mut nfc_device) -> c_int {
    let Some(data) = (unsafe { driver_data(device) }) else {
        return NFC_SUCCESS;
    };
    #[cfg(not(windows))]
    {
        data.abort_requested = true;
        if data.abort_fds[0] >= 0 {
            unsafe {
                libc::close(data.abort_fds[0]);
            }
        }
        data.abort_fds = [-1, -1];
        if unsafe { libc::pipe(data.abort_fds.as_mut_ptr()) } != 0 {
            return NFC_ESOFT;
        }
    }
    #[cfg(windows)]
    {
        data.abort_flag = true;
    }
    NFC_SUCCESS
}

static PN532_UART_IO: pn53x_io = pn53x_io {
    send: Some(pn532_uart_send),
    receive: Some(pn532_uart_receive),
};

static PN532_UART_DRIVER: nfc_driver = nfc_driver {
    name: PN532_UART_DRIVER_NAME_CSTR,
    scan_type: scan_type_enum::INTRUSIVE,
    scan: Some(pn532_uart_scan),
    open: Some(pn532_uart_open),
    close: Some(pn532_uart_close),
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
    abort_command: Some(pn532_uart_abort_command),
    idle: Some(pn53x_idle_callback),
    powerdown: Some(pn53x_powerdown_callback),
};

#[cfg(test)]
pub(crate) use crate::buses::uart::{
    test_add_port, test_queue_rx, test_reset as test_reset_uart,
    test_snapshot as test_snapshot_uart, test_take_tx,
};
#[cfg(test)]
pub(crate) use crate::drivers::pn53x_native::TestStateSnapshot as NativeTestSnapshot;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi_support::bounded_strlen;
    use std::ffi::CString;

    fn context() -> *mut nfc_context {
        unsafe { crate::nfc_context_new() }
    }

    fn connstring_at(connstrings: *mut nfc_connstring, index: usize) -> String {
        let ptr = unsafe { connstrings.add(index).cast::<c_char>() };
        let len = bounded_strlen(ptr, NFC_BUFSIZE_CONNSTRING);
        String::from_utf8_lossy(unsafe { slice::from_raw_parts(ptr.cast::<u8>(), len) })
            .into_owned()
    }

    #[test]
    fn scan_success_and_failure() {
        let _guard = test_lock_native();
        test_reset_uart();
        test_reset_native();
        test_add_port("/dev/ttyUSB0", true, false);
        test_add_port("/dev/ttyUSB1", true, false);
        test_queue_check_communication_result_native(NFC_SUCCESS);
        test_queue_check_communication_result_native(NFC_EIO);

        let ctx = context();
        let mut connstrings = [[0 as c_char; NFC_BUFSIZE_CONNSTRING]; 2];
        let found = unsafe { pn532_uart_scan(ctx, connstrings.as_mut_ptr(), connstrings.len()) };
        assert_eq!(found, 1);
        assert_eq!(
            connstring_at(connstrings.as_mut_ptr(), 0),
            "pn532_uart:/dev/ttyUSB0:115200"
        );

        let snapshot = test_snapshot_native();
        assert_eq!(snapshot.check_communication_calls, 2);
        unsafe { crate::nfc_context_free(ctx) };
    }

    #[test]
    fn open_send_receive_abort_and_close() {
        let _guard = test_lock_native();
        test_reset_uart();
        test_reset_native();
        test_add_port("/dev/ttyUSB0", true, false);
        test_queue_check_communication_result_native(NFC_SUCCESS);

        let ctx = context();
        let connstring = CString::new("pn532_uart:/dev/ttyUSB0:115200").unwrap();
        let device = unsafe { pn532_uart_open(ctx, connstring.as_ptr()) };
        assert!(!device.is_null());

        let ack = pn53x_ack_frame_bytes().to_vec();
        test_queue_rx("/dev/ttyUSB0", &ack);
        let tx = [0x02, 0x00];
        assert_eq!(
            unsafe { pn532_uart_send(device, tx.as_ptr(), tx.len(), 50) },
            NFC_SUCCESS
        );
        assert!(!test_take_tx("/dev/ttyUSB0").is_empty());

        // Queue a normal response frame for command 0x02 -> 0x03
        test_queue_rx(
            "/dev/ttyUSB0",
            &[0x00, 0x00, 0xff, 0x03, 0xfd, 0xD5, 0x03, 0x28, 0x00, 0x00],
        );
        if let Some(chip) = unsafe { chip_data(device) } {
            chip.last_command = 0x02;
        }
        let mut rx = [0u8; 1];
        assert_eq!(
            unsafe { pn532_uart_receive(device, rx.as_mut_ptr(), rx.len(), 50) },
            1
        );
        assert_eq!(rx, [0x28]);

        assert_eq!(unsafe { pn532_uart_abort_command(device) }, NFC_SUCCESS);
        test_queue_rx("/dev/ttyUSB0", &[0x00; 5]);
        assert_eq!(
            unsafe { pn532_uart_receive(device, rx.as_mut_ptr(), rx.len(), 50) },
            NFC_EOPABORTED
        );

        unsafe { pn532_uart_close(device) };
        let native = test_snapshot_native();
        assert_eq!(native.data_new_calls, 1);
        assert_eq!(native.data_free_calls, 1);
        unsafe { crate::nfc_context_free(ctx) };
    }
}
