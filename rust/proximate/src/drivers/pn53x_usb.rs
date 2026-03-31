// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Rust-owned PN53x USB driver backed by the in-tree Rust USB helper and the
// existing C pn53x chip layer.

#![allow(non_camel_case_types)]

use crate::ffi_support::{
    as_mut, as_ref, copy_bytes_to_c_buffer, copy_bytes_with_truncation, fixed_c_buffer_to_string,
};
use crate::ffi_types::{
    nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_mode, nfc_modulation, nfc_modulation_type,
    nfc_property, nfc_target,
};
use crate::lifecycle::{
    DEVICE_NAME_LENGTH, nfc_connstring, nfc_context, nfc_device, nfc_device_free, nfc_device_new,
    nfc_driver, scan_type_enum,
};
#[cfg(not(test))]
use crate::usbbus::usb_device_get_bulk_endpoints;
use crate::usbbus::{usb_bulk_endpoints, usb_dev_handle, usb_device, usb_device_list};
use crate::{
    LOG_PRIORITY_DEBUG, LOG_PRIORITY_ERROR, NFC_BUFSIZE_CONNSTRING, connstring_decode,
    emit_log_message, release_allocated_ptr, reset_last_error, set_last_error_message,
};
use libc::{c_char, c_int, c_uchar, c_void};
#[cfg(test)]
use std::collections::VecDeque;
use std::ffi::{CStr, CString};
use std::mem::size_of;
use std::ptr;
use std::slice;
#[cfg(test)]
use std::sync::{Mutex, OnceLock};

const NFC_SUCCESS: c_int = 0;
const NFC_EIO: c_int = -1;
const NFC_EINVARG: c_int = -2;
const NFC_EOPABORTED: c_int = -7;

const LOG_GROUP_DRIVER: u8 = 4;
const LOG_PRIORITY_INFO: u8 = 2;
const LOG_CATEGORY: *const c_char = b"libnfc.driver.pn53x_usb\0" as *const u8 as *const c_char;

const PN53X_USB_DRIVER_NAME_CSTR: *const c_char = b"pn53x_usb\0" as *const u8 as *const c_char;
const USB_BUS_NAME_CSTR: *const c_char = b"usb\0" as *const u8 as *const c_char;
const USB_INFINITE_TIMEOUT: c_int = 0;
const USB_TIMEOUT_PER_PASS: c_int = 200;
const PN53X_EXTENDED_FRAME_DATA_MAX_LEN: usize = 264;
const PN53X_EXTENDED_FRAME_OVERHEAD: usize = 11;
const PN53X_ACK_FRAME_LEN: usize = 6;
const PN53X_USB_BUFFER_LEN: usize =
    PN53X_EXTENDED_FRAME_DATA_MAX_LEN + PN53X_EXTENDED_FRAME_OVERHEAD;
const GET_FIRMWARE_VERSION: u8 = 0x02;
const PN53X_REG_CONTROL_SWITCH_RNG: u16 = 0x6106;
const PN53X_REG_CIU_TXSEL: u16 = 0x6306;
const PN53X_SFR_P3: u16 = 0xFFB0;
const PN53X_SFR_P3CFGB: u16 = 0xFFFD;
const SYMBOL_CURLIMOFF: u8 = 0x08;
const SYMBOL_SIC_SWITCH_EN: u8 = 0x10;
const SYMBOL_RANDOM_DATAREADY: u8 = 0x02;

const NO_TARGET_SUPPORT: [nfc_modulation_type; 1] = [nfc_modulation_type::NMT_UNDEFINED];

const BT_XRAM_USB_DESC_SCL3711: &[u8] = &[
    0x09, 0x02, 0x20, 0x00, 0x01, 0x01, 0x00, 0x80, 0x32, 0x09, 0x04, 0x00, 0x00, 0x02, 0xff, 0xff,
    0xff, 0x00, 0x07, 0x05, 0x04, 0x02, 0x40, 0x00, 0x04, 0x07, 0x05, 0x84, 0x02, 0x40, 0x00, 0x04,
    0x1e, 0x03, 0x53, 0x00, 0x43, 0x00, 0x4c, 0x00, 0x33, 0x00, 0x37, 0x00, 0x31, 0x00, 0x31, 0x00,
    0x2d, 0x00, 0x4e, 0x00, 0x46, 0x00, 0x43, 0x00, 0x26, 0x00, 0x52, 0x00, 0x57,
];

const BT_XRAM_USB_DESC_NXPPN533: &[u8] = &[
    0x09, 0x02, 0x20, 0x00, 0x01, 0x01, 0x00, 0x80, 0x32, 0x09, 0x04, 0x00, 0x00, 0x02, 0xff, 0xff,
    0xff, 0x00, 0x07, 0x05, 0x04, 0x02, 0x40, 0x00, 0x04, 0x07, 0x05, 0x84, 0x02, 0x40, 0x00, 0x04,
    0x0c, 0x03, 0x50, 0x00, 0x4e, 0x00, 0x35, 0x00, 0x33, 0x00, 0x33, 0x00, 0x04, 0x03, 0x09, 0x04,
    0x08, 0x03, 0x4e, 0x00, 0x58, 0x00, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const BT_XRAM_USB_DESC_ASKLOGO: &[u8] = &[
    0x09, 0x02, 0x20, 0x00, 0x01, 0x01, 0x00, 0x80, 0x96, 0x09, 0x04, 0x00, 0x00, 0x02, 0xff, 0xff,
    0xff, 0x00, 0x07, 0x05, 0x04, 0x02, 0x40, 0x00, 0x04, 0x07, 0x05, 0x84, 0x02, 0x40, 0x00, 0x04,
    0x0a, 0x03, 0x4c, 0x00, 0x6f, 0x00, 0x47, 0x00, 0x4f, 0x00, 0x04, 0x03, 0x09, 0x04, 0x08, 0x03,
    0x41, 0x00, 0x53, 0x00, 0x4b, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
enum pn53x_type {
    PN53X = 0x00,
    PN531 = 0x01,
    PN532 = 0x02,
    PN533 = 0x04,
    RCS360 = 0x08,
}

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
enum pn53x_power_mode {
    NORMAL = 0,
    POWERDOWN = 1,
    LOWVBAT = 2,
}

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
enum pn53x_operating_mode {
    IDLE = 0,
    INITIATOR = 1,
    TARGET = 2,
}

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
enum pn532_sam_mode {
    PSM_NORMAL = 0x01,
    PSM_VIRTUAL_CARD = 0x02,
    PSM_WIRED_CARD = 0x03,
    PSM_DUAL_CARD = 0x04,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct pn53x_io {
    send: Option<unsafe extern "C" fn(*mut nfc_device, *const u8, usize, c_int) -> c_int>,
    receive: Option<unsafe extern "C" fn(*mut nfc_device, *mut u8, usize, c_int) -> c_int>,
}

#[repr(C)]
#[allow(dead_code, non_snake_case)]
struct pn53x_data {
    type_: pn53x_type,
    firmware_text: [c_char; 22],
    power_mode: pn53x_power_mode,
    operating_mode: pn53x_operating_mode,
    current_target: *mut nfc_target,
    sam_mode: pn532_sam_mode,
    io: *const pn53x_io,
    last_status_byte: u8,
    ui8TxBits: u8,
    ui8Parameters: u8,
    last_command: u8,
    timer_correction: i16,
    timer_prescaler: u16,
    wb_data: [u8; 0x18],
    wb_mask: [u8; 0x18],
    wb_trigged: bool,
    timeout_command: c_int,
    timeout_atr: c_int,
    timeout_communication: c_int,
    supported_modulation_as_initiator: *mut nfc_modulation_type,
    supported_modulation_as_target: *mut nfc_modulation_type,
    progressive_field: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
enum pn53x_usb_model {
    Unknown = 0,
    NxpPn531 = 1,
    SonyPn531 = 2,
    NxpPn533 = 3,
    AskLogo = 4,
    ScmScl3711 = 5,
    ScmScl3712 = 6,
    SonyRcs360 = 7,
}

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(non_snake_case)]
struct Pn53xUsbData {
    pudh: *mut usb_dev_handle,
    model: pn53x_usb_model,
    uiEndPointIn: u32,
    uiEndPointOut: u32,
    uiMaxPacketSize: u32,
    interface_number: c_int,
    configuration_value: c_int,
    alternate_setting: c_int,
    abort_flag: bool,
    possibly_corrupted_usbdesc: bool,
}

struct Pn53xUsbSupportedDevice {
    vendor_id: u16,
    product_id: u16,
    model: pn53x_usb_model,
    name: &'static [u8],
    endpoint_in: u32,
    endpoint_out: u32,
    max_packet_size: u32,
}

static PN53X_USB_SUPPORTED_DEVICES: [Pn53xUsbSupportedDevice; 7] = [
    Pn53xUsbSupportedDevice {
        vendor_id: 0x04CC,
        product_id: 0x0531,
        model: pn53x_usb_model::NxpPn531,
        name: b"Philips / PN531",
        endpoint_in: 0x84,
        endpoint_out: 0x04,
        max_packet_size: 0x40,
    },
    Pn53xUsbSupportedDevice {
        vendor_id: 0x04CC,
        product_id: 0x2533,
        model: pn53x_usb_model::NxpPn533,
        name: b"NXP / PN533",
        endpoint_in: 0x84,
        endpoint_out: 0x04,
        max_packet_size: 0x40,
    },
    Pn53xUsbSupportedDevice {
        vendor_id: 0x04E6,
        product_id: 0x5591,
        model: pn53x_usb_model::ScmScl3711,
        name: b"SCM Micro / SCL3711-NFC&RW",
        endpoint_in: 0x84,
        endpoint_out: 0x04,
        max_packet_size: 0x40,
    },
    Pn53xUsbSupportedDevice {
        vendor_id: 0x04E6,
        product_id: 0x5594,
        model: pn53x_usb_model::ScmScl3712,
        name: b"SCM Micro / SCL3712-NFC&RW",
        endpoint_in: 0,
        endpoint_out: 0,
        max_packet_size: 0,
    },
    Pn53xUsbSupportedDevice {
        vendor_id: 0x054C,
        product_id: 0x0193,
        model: pn53x_usb_model::SonyPn531,
        name: b"Sony / PN531",
        endpoint_in: 0x84,
        endpoint_out: 0x04,
        max_packet_size: 0x40,
    },
    Pn53xUsbSupportedDevice {
        vendor_id: 0x1FD3,
        product_id: 0x0608,
        model: pn53x_usb_model::AskLogo,
        name: b"ASK / LoGO",
        endpoint_in: 0x84,
        endpoint_out: 0x04,
        max_packet_size: 0x40,
    },
    Pn53xUsbSupportedDevice {
        vendor_id: 0x054C,
        product_id: 0x02E1,
        model: pn53x_usb_model::SonyRcs360,
        name: b"Sony / FeliCa S360 [PaSoRi]",
        endpoint_in: 0x84,
        endpoint_out: 0x04,
        max_packet_size: 0x40,
    },
];

fn log_driver_message(priority: u8, message: &str) {
    if let Ok(c_message) = CString::new(message) {
        unsafe {
            emit_log_message(LOG_GROUP_DRIVER, LOG_CATEGORY, priority, c_message.as_ptr());
        }
    }
}

fn log_driver_debug(message: &str) {
    log_driver_message(LOG_PRIORITY_DEBUG, message);
}

fn log_driver_info(message: &str) {
    log_driver_message(LOG_PRIORITY_INFO, message);
}

fn log_driver_error(message: &str) {
    log_driver_message(LOG_PRIORITY_ERROR, message);
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut rendered = String::with_capacity(bytes.len().saturating_mul(3));
    for (index, byte) in bytes.iter().enumerate() {
        if index > 0 {
            rendered.push(' ');
        }
        rendered.push_str(&format!("{byte:02X}"));
    }
    rendered
}

fn bit(value: u8) -> u8 {
    1u8 << value
}

fn driver_data<'a>(device: *mut nfc_device) -> Option<&'a mut Pn53xUsbData> {
    let device = unsafe { as_mut(device) }?;
    unsafe { as_mut(device.driver_data.cast::<Pn53xUsbData>()) }
}

fn chip_data<'a>(device: *mut nfc_device) -> Option<&'a mut pn53x_data> {
    let device = unsafe { as_mut(device) }?;
    unsafe { as_mut(device.chip_data.cast::<pn53x_data>()) }
}

fn set_device_last_error(device: *mut nfc_device, value: c_int) {
    if let Some(device) = unsafe { as_mut(device) } {
        device.last_error = value;
    }
}

fn find_supported_device(
    vendor_id: u16,
    product_id: u16,
) -> Option<&'static Pn53xUsbSupportedDevice> {
    PN53X_USB_SUPPORTED_DEVICES
        .iter()
        .find(|device| device.vendor_id == vendor_id && device.product_id == product_id)
}

fn pn53x_usb_driver_ptr() -> *const nfc_driver {
    ptr::addr_of!(PN53X_USB_DRIVER)
}

#[cfg(libnfc_driver_pn53x_usb)]
pub(crate) fn builtin_driver_ptr() -> *const nfc_driver {
    pn53x_usb_driver_ptr()
}

unsafe fn free_decode_param(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe { release_allocated_ptr(ptr.cast::<c_void>()) };
    }
}

fn alloc_driver_data(device: *mut nfc_device) -> bool {
    let Some(device_ref) = (unsafe { as_mut(device) }) else {
        return false;
    };
    if !device_ref.driver_data.is_null() {
        return true;
    }

    let allocation = unsafe { libc::calloc(1, size_of::<Pn53xUsbData>()) };
    if allocation.is_null() {
        unsafe { libc::perror(crate::MALLOC_LABEL) };
        set_last_error_message("Failed to allocate pn53x_usb driver data");
        return false;
    }

    device_ref.driver_data = allocation;
    true
}

fn bus_device_strings(device: *const usb_device) -> Option<(String, String)> {
    let mut bus = [0 as c_char; 4];
    let mut node = [0 as c_char; 4];
    let rc = unsafe {
        usb_get_bus_device_strings_bridge(
            device,
            bus.as_mut_ptr(),
            bus.len(),
            node.as_mut_ptr(),
            node.len(),
        )
    };
    if rc < 0 {
        return None;
    }

    Some((
        fixed_c_buffer_to_string(&bus),
        fixed_c_buffer_to_string(&node),
    ))
}

fn connstring_bytes(bus: &str, device: &str) -> Vec<u8> {
    format!("pn53x_usb:{bus}:{device}").into_bytes()
}

fn fill_device_name(
    device: *const usb_device,
    handle: *mut usb_dev_handle,
    dst: *mut c_char,
    len: usize,
) {
    if dst.is_null() || len == 0 {
        return;
    }

    let mut rendered = String::new();
    let Some(device_ref) = (unsafe { as_ref(device) }) else {
        return;
    };

    if !handle.is_null() && device_ref.manufacturer_string_index != 0 {
        if let Some(value) =
            usb_get_string_simple_string(handle, device_ref.manufacturer_string_index)
        {
            rendered.push_str(&value);
        }
    }

    if !handle.is_null() && device_ref.product_string_index != 0 {
        if let Some(value) = usb_get_string_simple_string(handle, device_ref.product_string_index) {
            if !rendered.is_empty() {
                rendered.push_str(" / ");
            }
            rendered.push_str(&value);
        }
    }

    if rendered.is_empty() {
        if let Some(supported) = find_supported_device(device_ref.vendor_id, device_ref.product_id)
        {
            rendered = String::from_utf8_lossy(supported.name).into_owned();
        }
    }

    unsafe { copy_bytes_with_truncation(dst, len, rendered.as_bytes()) };
}

fn usb_get_string_simple_string(handle: *mut usb_dev_handle, index: u8) -> Option<String> {
    let mut buffer = [0 as c_char; DEVICE_NAME_LENGTH];
    let rc = unsafe {
        usb_get_string_simple_bridge(handle, index as c_int, buffer.as_mut_ptr(), buffer.len())
    };
    if rc <= 0 {
        return None;
    }
    Some(fixed_c_buffer_to_string(&buffer))
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

fn set_timer_correction(device: *mut nfc_device, model: pn53x_usb_model) {
    let Some(chip) = chip_data(device) else {
        return;
    };

    chip.timer_correction = match model {
        pn53x_usb_model::AskLogo => {
            chip.progressive_field = true;
            50
        }
        pn53x_usb_model::ScmScl3711 | pn53x_usb_model::ScmScl3712 | pn53x_usb_model::NxpPn533 => 46,
        pn53x_usb_model::NxpPn531 => 50,
        pn53x_usb_model::SonyPn531 => 54,
        pn53x_usb_model::SonyRcs360 | pn53x_usb_model::Unknown => 0,
    };
}

fn pn53x_usb_get_end_points_default(device: *const usb_device, data: &mut Pn53xUsbData) -> bool {
    let Some(device_ref) = (unsafe { as_ref(device) }) else {
        return false;
    };

    let Some(supported) = find_supported_device(device_ref.vendor_id, device_ref.product_id) else {
        return false;
    };

    if supported.max_packet_size == 0 {
        return false;
    }

    data.uiEndPointIn = supported.endpoint_in;
    data.uiEndPointOut = supported.endpoint_out;
    data.uiMaxPacketSize = supported.max_packet_size;
    true
}

fn pn53x_usb_get_end_points(device: *const usb_device, data: &mut Pn53xUsbData) -> bool {
    let mut endpoints = usb_bulk_endpoints::default();
    if !unsafe { usb_device_get_bulk_endpoints_bridge(device, ptr::addr_of_mut!(endpoints)) } {
        return false;
    }

    data.uiEndPointIn = endpoints.endpoint_in as u32;
    data.uiEndPointOut = endpoints.endpoint_out as u32;
    data.uiMaxPacketSize = endpoints.max_packet_size as u32;
    data.interface_number = endpoints.interface_number as c_int;
    data.alternate_setting = endpoints.alternate_setting;
    true
}

fn maybe_fix_usb_descriptor(device: *mut nfc_device) {
    let Some(data) = driver_data(device) else {
        return;
    };

    let descriptor = match data.model {
        pn53x_usb_model::NxpPn533 => BT_XRAM_USB_DESC_NXPPN533,
        pn53x_usb_model::ScmScl3711 => BT_XRAM_USB_DESC_SCL3711,
        pn53x_usb_model::AskLogo => BT_XRAM_USB_DESC_ASKLOGO,
        _ => &[],
    };

    if descriptor.is_empty() {
        return;
    }

    log_driver_info("Fixing USB descriptors corruption");

    let mut command = vec![0u8; 19 + descriptor.len()];
    command[0] = GET_FIRMWARE_VERSION;
    command[19..].copy_from_slice(descriptor);
    let mut rx = [0u8; 4];
    let rc = unsafe {
        pn53x_transceive_bridge(
            device,
            command.as_ptr(),
            command.len(),
            rx.as_mut_ptr(),
            rx.len(),
            -1,
        )
    };
    if rc >= 0 {
        if let Some(data) = driver_data(device) {
            data.possibly_corrupted_usbdesc = false;
        }
    }
}

unsafe extern "C" fn pn53x_usb_scan(
    _context: *const nfc_context,
    connstrings: *mut nfc_connstring,
    connstrings_len: usize,
) -> usize {
    reset_last_error();

    let mut devices = usb_device_list::default();
    if unsafe { usb_get_device_list_bridge(ptr::addr_of_mut!(devices)) } < 0 {
        return 0;
    }

    let mut found = 0usize;
    let device_slice = unsafe { slice::from_raw_parts(devices.devices, devices.count) };
    for device in device_slice {
        let Some(supported) = find_supported_device(device.vendor_id, device.product_id) else {
            continue;
        };

        if supported.max_packet_size == 0 {
            let mut endpoints = usb_bulk_endpoints::default();
            if !unsafe {
                usb_device_get_bulk_endpoints_bridge(
                    device as *const usb_device,
                    ptr::addr_of_mut!(endpoints),
                )
            } {
                continue;
            }
        }

        let mut handle = ptr::null_mut();
        if unsafe { usb_open_bridge(device as *const usb_device, ptr::addr_of_mut!(handle)) } < 0 {
            continue;
        }

        let configuration = if device.configuration_value != 0 {
            device.configuration_value as c_int
        } else {
            1
        };
        let set_config_rc = unsafe { usb_set_configuration_bridge(handle, configuration) };
        if set_config_rc < 0 {
            log_driver_error(&format!(
                "Unable to set USB configuration ({})",
                usb_strerror_string(set_config_rc)
            ));
            unsafe { usb_close_bridge(handle) };
            continue;
        }

        let Some((bus, node)) = bus_device_strings(device as *const usb_device) else {
            unsafe { usb_close_bridge(handle) };
            continue;
        };

        log_driver_debug(&format!("device found: Bus {bus} Device {node}"));
        unsafe { usb_close_bridge(handle) };

        if found < connstrings_len {
            let entry = connstring_bytes(&bus, &node);
            if copy_scan_connstring(connstrings, found, &entry) {
                found += 1;
            }
        }

        if found == connstrings_len {
            break;
        }
    }

    unsafe { usb_free_device_list_bridge(ptr::addr_of_mut!(devices)) };
    found
}

unsafe extern "C" fn pn53x_usb_open(
    context: *const nfc_context,
    connstring: *const c_char,
) -> *mut nfc_device {
    reset_last_error();

    if connstring.is_null() {
        set_last_error_message("pn53x_usb open received NULL connstring");
        return ptr::null_mut();
    }

    let mut dirname = ptr::null_mut();
    let mut filename = ptr::null_mut();
    let decode_level = unsafe {
        connstring_decode(
            connstring,
            PN53X_USB_DRIVER_NAME_CSTR,
            USB_BUS_NAME_CSTR,
            ptr::addr_of_mut!(dirname),
            ptr::addr_of_mut!(filename),
        )
    };
    log_driver_debug(&format!(
        "{decode_level} element(s) have been decoded from \"{}\"",
        unsafe { CStr::from_ptr(connstring) }.to_string_lossy()
    ));
    if decode_level < 1 {
        unsafe {
            free_decode_param(dirname);
            free_decode_param(filename);
        }
        return ptr::null_mut();
    }

    let mut devices = usb_device_list::default();
    if unsafe { usb_get_device_list_bridge(ptr::addr_of_mut!(devices)) } < 0 {
        unsafe {
            free_decode_param(dirname);
            free_decode_param(filename);
        }
        return ptr::null_mut();
    }

    let mut built_device = ptr::null_mut();
    let device_slice = unsafe { slice::from_raw_parts(devices.devices, devices.count) };
    for device in device_slice {
        let Some((bus, node)) = bus_device_strings(device as *const usb_device) else {
            continue;
        };

        if decode_level > 1 && !dirname.is_null() {
            let decoded = unsafe { CStr::from_ptr(dirname) }.to_string_lossy();
            if decoded != bus {
                continue;
            }
        }
        if decode_level > 2 && !filename.is_null() {
            let decoded = unsafe { CStr::from_ptr(filename) }.to_string_lossy();
            if decoded != node {
                continue;
            }
        }

        let Some(supported) = find_supported_device(device.vendor_id, device.product_id) else {
            continue;
        };

        let mut local = Pn53xUsbData {
            pudh: ptr::null_mut(),
            model: supported.model,
            uiEndPointIn: 0,
            uiEndPointOut: 0,
            uiMaxPacketSize: 0,
            interface_number: 0,
            configuration_value: if device.configuration_value != 0 {
                device.configuration_value as c_int
            } else {
                1
            },
            alternate_setting: 0,
            abort_flag: false,
            possibly_corrupted_usbdesc: false,
        };
        if unsafe { usb_open_bridge(device as *const usb_device, ptr::addr_of_mut!(local.pudh)) }
            < 0
        {
            continue;
        }

        if !pn53x_usb_get_end_points_default(device as *const usb_device, &mut local)
            && !pn53x_usb_get_end_points(device as *const usb_device, &mut local)
        {
            unsafe { usb_close_bridge(local.pudh) };
            continue;
        }

        let set_config_rc =
            unsafe { usb_set_configuration_bridge(local.pudh, local.configuration_value) };
        if set_config_rc < 0 {
            log_driver_error(&format!(
                "Unable to set USB configuration ({})",
                usb_strerror_string(set_config_rc)
            ));
            if unsafe { usb_error_is_access_bridge(set_config_rc) } {
                log_driver_info(&format!(
                    "Warning: Please double check USB permissions for device {:04x}:{:04x}",
                    device.vendor_id, device.product_id
                ));
            }
            unsafe { usb_close_bridge(local.pudh) };
            continue;
        }

        let claim_rc = unsafe { usb_claim_interface_bridge(local.pudh, local.interface_number) };
        if claim_rc < 0 {
            log_driver_error(&format!(
                "Unable to claim USB interface ({})",
                usb_strerror_string(claim_rc)
            ));
            unsafe { usb_close_bridge(local.pudh) };
            continue;
        }
        if local.alternate_setting > 0 {
            let alt_rc = unsafe {
                usb_set_altinterface_bridge(
                    local.pudh,
                    local.interface_number,
                    local.alternate_setting,
                )
            };
            if alt_rc < 0 {
                log_driver_error(&format!(
                    "Unable to set alternate setting on USB interface ({})",
                    usb_strerror_string(alt_rc)
                ));
                unsafe { usb_release_interface_bridge(local.pudh, local.interface_number) };
                unsafe { usb_close_bridge(local.pudh) };
                continue;
            }
        }

        let full_connstring = CString::new(connstring_bytes(&bus, &node)).unwrap();
        let device_ptr = unsafe { nfc_device_new(context, full_connstring.as_ptr()) };
        if device_ptr.is_null() {
            unsafe { libc::perror(crate::MALLOC_LABEL) };
            unsafe { usb_release_interface_bridge(local.pudh, local.interface_number) };
            unsafe { usb_close_bridge(local.pudh) };
            continue;
        }

        fill_device_name(
            device as *const usb_device,
            local.pudh,
            unsafe { (*device_ptr).name.as_mut_ptr() },
            DEVICE_NAME_LENGTH,
        );

        if !alloc_driver_data(device_ptr) {
            unsafe { usb_release_interface_bridge(local.pudh, local.interface_number) };
            unsafe { usb_close_bridge(local.pudh) };
            unsafe { nfc_device_free(device_ptr) };
            continue;
        }
        let handle = local.pudh;
        let interface_number = local.interface_number;
        unsafe {
            ptr::write((*device_ptr).driver_data.cast::<Pn53xUsbData>(), local);
        }

        if unsafe { pn53x_data_new_bridge(device_ptr, ptr::addr_of!(PN53X_USB_IO)) }.is_null() {
            log_driver_error("Failed to allocate chip data");
            unsafe { usb_release_interface_bridge(handle, interface_number) };
            unsafe { usb_close_bridge(handle) };
            unsafe { nfc_device_free(device_ptr) };
            continue;
        }

        set_timer_correction(device_ptr, supported.model);
        unsafe {
            (*device_ptr).driver = pn53x_usb_driver_ptr();
        }

        let ack_rc = pn53x_usb_ack(device_ptr);
        if ack_rc < 0 {
            unsafe { pn53x_data_free_bridge(device_ptr) };
            unsafe { usb_release_interface_bridge(handle, interface_number) };
            unsafe { usb_close_bridge(handle) };
            unsafe { nfc_device_free(device_ptr) };
            continue;
        }

        if pn53x_usb_init(device_ptr) < 0 {
            unsafe { pn53x_data_free_bridge(device_ptr) };
            unsafe { usb_release_interface_bridge(handle, interface_number) };
            unsafe { usb_close_bridge(handle) };
            unsafe { nfc_device_free(device_ptr) };
            continue;
        }

        if let Some(data) = driver_data(device_ptr) {
            data.abort_flag = false;
        }
        built_device = device_ptr;
        break;
    }

    unsafe {
        usb_free_device_list_bridge(ptr::addr_of_mut!(devices));
        free_decode_param(dirname);
        free_decode_param(filename);
    }
    built_device
}

unsafe extern "C" fn pn53x_usb_close(device: *mut nfc_device) {
    if device.is_null() {
        return;
    }

    let model = driver_data(device)
        .map(|data| data.model)
        .unwrap_or(pn53x_usb_model::Unknown);

    let _ = pn53x_usb_ack(device);

    if model == pn53x_usb_model::AskLogo {
        let _ = unsafe {
            pn53x_write_register_bridge(
                device,
                PN53X_SFR_P3,
                0xFF,
                bit(0) | bit(1) | bit(2) | bit(3) | bit(5),
            )
        };
    }

    if driver_data(device)
        .map(|data| data.possibly_corrupted_usbdesc)
        .unwrap_or(false)
    {
        maybe_fix_usb_descriptor(device);
    }

    let _ = unsafe { pn53x_idle_bridge(device) };

    let (handle, interface_number) = if let Some(data) = driver_data(device) {
        (data.pudh, data.interface_number)
    } else {
        (ptr::null_mut(), 0)
    };

    if !handle.is_null() {
        let release_rc = unsafe { usb_release_interface_bridge(handle, interface_number) };
        if release_rc < 0 {
            log_driver_error(&format!(
                "Unable to release USB interface ({})",
                usb_strerror_string(release_rc)
            ));
        }
        unsafe { usb_close_bridge(handle) };
    }

    unsafe { pn53x_data_free_bridge(device) };
    unsafe { nfc_device_free(device) };
}

fn pn53x_usb_bulk_read(data: &Pn53xUsbData, rx: &mut [u8], timeout: c_int) -> c_int {
    let rc = unsafe {
        usb_bulk_read_bridge(
            data.pudh,
            data.uiEndPointIn as c_uchar,
            rx.as_mut_ptr(),
            rx.len(),
            timeout,
        )
    };
    if rc > 0 {
        log_driver_debug(&format!("RX {}", encode_hex(&rx[..rc as usize])));
    } else if rc < 0 && !unsafe { usb_error_is_timeout_bridge(rc) } {
        log_driver_error(&format!(
            "Unable to read from USB ({})",
            usb_strerror_string(rc)
        ));
    }
    rc
}

fn pn53x_usb_bulk_write(data: &Pn53xUsbData, tx: &[u8], timeout: c_int) -> c_int {
    log_driver_debug(&format!("TX {}", encode_hex(tx)));
    let rc = unsafe {
        usb_bulk_write_bridge(
            data.pudh,
            data.uiEndPointOut as c_uchar,
            if tx.is_empty() {
                ptr::null()
            } else {
                tx.as_ptr()
            },
            tx.len(),
            timeout,
        )
    };
    if rc > 0 {
        if data.uiMaxPacketSize != 0 && (rc as u32 % data.uiMaxPacketSize) == 0 {
            let _ = unsafe {
                usb_bulk_write_bridge(
                    data.pudh,
                    data.uiEndPointOut as c_uchar,
                    ptr::null(),
                    0,
                    timeout,
                )
            };
        }
    } else if rc < 0 {
        log_driver_error(&format!(
            "Unable to write to USB ({})",
            usb_strerror_string(rc)
        ));
    }
    rc
}

fn pn53x_usb_ack(device: *mut nfc_device) -> c_int {
    let Some(data) = driver_data(device) else {
        return NFC_EINVARG;
    };
    pn53x_usb_bulk_write(data, pn53x_ack_frame_bytes(), 1000)
}

unsafe extern "C" fn pn53x_usb_send(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: usize,
    timeout: c_int,
) -> c_int {
    if device.is_null() || tx.is_null() || tx_len == 0 {
        set_device_last_error(device, NFC_EINVARG);
        return NFC_EINVARG;
    }

    let mut frame = [0u8; PN53X_USB_BUFFER_LEN];
    frame[..3].copy_from_slice(&[0x00, 0x00, 0xff]);
    let mut frame_len = 0usize;
    let build_rc = unsafe {
        pn53x_build_frame_bridge(frame.as_mut_ptr(), ptr::addr_of_mut!(frame_len), tx, tx_len)
    };
    if build_rc < 0 {
        set_device_last_error(device, build_rc);
        return build_rc;
    }

    if let Some(data) = driver_data(device) {
        data.possibly_corrupted_usbdesc |= tx_len > 17;
    }

    let Some(data) = driver_data(device) else {
        set_device_last_error(device, NFC_EINVARG);
        return NFC_EINVARG;
    };

    let write_rc = pn53x_usb_bulk_write(data, &frame[..frame_len], timeout);
    if write_rc < 0 {
        set_device_last_error(device, write_rc);
        return write_rc;
    }

    let mut rx = [0u8; PN53X_USB_BUFFER_LEN];
    let read_rc = pn53x_usb_bulk_read(data, &mut rx, timeout);
    if read_rc < 0 {
        let _ = pn53x_usb_ack(device);
        set_device_last_error(device, read_rc);
        return read_rc;
    }

    let ack_rc = unsafe { pn53x_check_ack_frame_bridge(device, rx.as_ptr(), read_rc as usize) };
    if ack_rc != 0 {
        let nack_rc = pn53x_usb_bulk_write(data, pn53x_nack_frame_bytes(), timeout);
        if nack_rc < 0 {
            let _ = pn53x_usb_ack(device);
            set_device_last_error(device, nack_rc);
            return nack_rc;
        }
    }

    set_device_last_error(device, NFC_SUCCESS);
    NFC_SUCCESS
}

unsafe extern "C" fn pn53x_usb_receive(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: usize,
    timeout: c_int,
) -> c_int {
    if device.is_null() || rx.is_null() || rx_len == 0 {
        set_device_last_error(device, NFC_EINVARG);
        return NFC_EINVARG;
    }

    let Some(data) = driver_data(device) else {
        set_device_last_error(device, NFC_EINVARG);
        return NFC_EINVARG;
    };

    let mut remaining = timeout;
    loop {
        let usb_timeout = if timeout == USB_INFINITE_TIMEOUT {
            USB_TIMEOUT_PER_PASS
        } else {
            remaining -= USB_TIMEOUT_PER_PASS;
            if remaining <= 0 {
                set_device_last_error(device, -6);
                return -6;
            }
            remaining.min(USB_TIMEOUT_PER_PASS)
        };

        let mut frame = [0u8; PN53X_USB_BUFFER_LEN];
        let read_rc = pn53x_usb_bulk_read(data, &mut frame, usb_timeout);
        if unsafe { usb_error_is_timeout_bridge(read_rc) } {
            if let Some(data) = driver_data(device) {
                if data.abort_flag {
                    data.abort_flag = false;
                    let _ = pn53x_usb_ack(device);
                    set_device_last_error(device, NFC_EOPABORTED);
                    return NFC_EOPABORTED;
                }
            }
            continue;
        }

        if read_rc < 0 {
            let _ = pn53x_usb_ack(device);
            set_device_last_error(device, read_rc);
            return read_rc;
        }

        let frame = &frame[..read_rc as usize];
        if frame.len() < 6 || &frame[..3] != [0x00, 0x00, 0xff] {
            log_driver_error("Frame preamble+start code mismatch");
            set_device_last_error(device, NFC_EIO);
            return NFC_EIO;
        }

        let mut offset = 3usize;
        let payload_len = if frame[offset] == 0x01 && frame[offset + 1] == 0xff {
            log_driver_error("Application level error detected");
            set_device_last_error(device, NFC_EIO);
            return NFC_EIO;
        } else if frame[offset] == 0xff && frame[offset + 1] == 0xff {
            offset += 2;
            if frame.len() < offset + 3 {
                set_device_last_error(device, NFC_EIO);
                return NFC_EIO;
            }
            let payload_len =
                (((frame[offset] as usize) << 8) | frame[offset + 1] as usize).saturating_sub(2);
            if ((frame[offset] as u16 + frame[offset + 1] as u16 + frame[offset + 2] as u16) % 256)
                != 0
            {
                log_driver_error("Length checksum mismatch");
                set_device_last_error(device, NFC_EIO);
                return NFC_EIO;
            }
            offset += 3;
            payload_len
        } else {
            if frame[offset].wrapping_add(frame[offset + 1]) != 0 {
                log_driver_error("Length checksum mismatch");
                set_device_last_error(device, NFC_EIO);
                return NFC_EIO;
            }
            let payload_len = (frame[offset] as usize).saturating_sub(2);
            offset += 2;
            payload_len
        };

        if payload_len > rx_len {
            log_driver_error(&format!(
                "Unable to receive data: buffer too small. (szDataLen: {rx_len}, len: {payload_len})"
            ));
            set_device_last_error(device, NFC_EIO);
            return NFC_EIO;
        }

        if frame.len() < offset + payload_len + 3 {
            set_device_last_error(device, NFC_EIO);
            return NFC_EIO;
        }

        if frame[offset] != 0xD5 {
            log_driver_error("TFI Mismatch");
            set_device_last_error(device, NFC_EIO);
            return NFC_EIO;
        }
        offset += 1;

        let expected_command = chip_data(device)
            .map(|chip| chip.last_command.wrapping_add(1))
            .unwrap_or(0);
        if frame[offset] != expected_command {
            log_driver_error("Command Code verification failed");
            set_device_last_error(device, NFC_EIO);
            return NFC_EIO;
        }
        offset += 1;

        unsafe { ptr::copy_nonoverlapping(frame[offset..].as_ptr(), rx, payload_len) };
        let payload = unsafe { slice::from_raw_parts(rx, payload_len) };
        offset += payload_len;

        let mut dcs = 0u8.wrapping_sub(0xD5);
        dcs = dcs.wrapping_sub(expected_command);
        for byte in payload {
            dcs = dcs.wrapping_sub(*byte);
        }
        if dcs != frame[offset] {
            log_driver_error("Data checksum mismatch");
            set_device_last_error(device, NFC_EIO);
            return NFC_EIO;
        }
        offset += 1;

        if frame[offset] != 0x00 {
            log_driver_error("Frame postamble mismatch");
            set_device_last_error(device, NFC_EIO);
            return NFC_EIO;
        }

        if let Some(data) = driver_data(device) {
            data.possibly_corrupted_usbdesc |= payload_len > 16;
        }
        set_device_last_error(device, NFC_SUCCESS);
        return payload_len as c_int;
    }
}

fn pn53x_usb_init(device: *mut nfc_device) -> c_int {
    let dummy = [GET_FIRMWARE_VERSION];
    let _ = unsafe {
        pn53x_transceive_bridge(device, dummy.as_ptr(), dummy.len(), ptr::null_mut(), 0, -1)
    };
    set_device_last_error(device, NFC_SUCCESS);

    if driver_data(device)
        .map(|data| data.model == pn53x_usb_model::SonyRcs360)
        .unwrap_or(false)
    {
        log_driver_debug("SONY RC-S360 initialization.");
        let command = [0x18, 0x01];
        let _ = unsafe {
            pn53x_transceive_bridge(
                device,
                command.as_ptr(),
                command.len(),
                ptr::null_mut(),
                0,
                -1,
            )
        };
        let _ = pn53x_usb_ack(device);
    }

    let init_rc = unsafe { pn53x_init_bridge(device) };
    if init_rc < 0 {
        return init_rc;
    }

    if driver_data(device)
        .map(|data| data.model == pn53x_usb_model::AskLogo)
        .unwrap_or(false)
    {
        log_driver_debug("ASK LoGO initialization.");
        let _ = unsafe {
            pn53x_write_register_bridge(
                device,
                PN53X_REG_CONTROL_SWITCH_RNG,
                0xFF,
                SYMBOL_CURLIMOFF | SYMBOL_SIC_SWITCH_EN | SYMBOL_RANDOM_DATAREADY,
            )
        };
        let _ = unsafe { pn53x_write_register_bridge(device, PN53X_REG_CIU_TXSEL, 0xFF, 0x14) };
        let _ = unsafe { pn53x_write_register_bridge(device, PN53X_SFR_P3CFGB, 0xFF, 0x37) };
        let _ = unsafe {
            pn53x_write_register_bridge(
                device,
                PN53X_SFR_P3,
                0xFF,
                bit(0) | bit(1) | bit(3) | bit(5),
            )
        };
    }

    if driver_data(device)
        .map(|data| data.possibly_corrupted_usbdesc)
        .unwrap_or(false)
    {
        maybe_fix_usb_descriptor(device);
    }

    NFC_SUCCESS
}

unsafe extern "C" fn pn53x_usb_set_property_bool(
    device: *mut nfc_device,
    property: nfc_property,
    enable: bool,
) -> c_int {
    let result = unsafe { pn53x_set_property_bool_bridge(device, property, enable) };
    if result < 0 {
        return result;
    }

    let Some(model) = driver_data(device).map(|data| data.model) else {
        return NFC_SUCCESS;
    };

    match model {
        pn53x_usb_model::AskLogo if property == nfc_property::NP_ACTIVATE_FIELD => unsafe {
            pn53x_write_register_bridge(
                device,
                PN53X_SFR_P3,
                bit(1) | bit(4),
                if enable { bit(4) } else { bit(1) },
            )
        },
        pn53x_usb_model::ScmScl3711 | pn53x_usb_model::ScmScl3712
            if property == nfc_property::NP_ACTIVATE_FIELD =>
        unsafe {
            pn53x_write_register_bridge(
                device,
                PN53X_SFR_P3,
                bit(2),
                if enable { 0 } else { bit(2) },
            )
        },
        _ => NFC_SUCCESS,
    }
}

unsafe extern "C" fn pn53x_usb_get_supported_modulation(
    device: *mut nfc_device,
    mode: nfc_mode,
    supported: *mut *const nfc_modulation_type,
) -> c_int {
    if driver_data(device)
        .map(|data| data.model != pn53x_usb_model::AskLogo || mode != nfc_mode::N_TARGET)
        .unwrap_or(true)
    {
        return unsafe { pn53x_get_supported_modulation_bridge(device, mode, supported) };
    }

    let Some(supported) = (unsafe { as_mut(supported) }) else {
        return NFC_EINVARG;
    };
    *supported = NO_TARGET_SUPPORT.as_ptr();
    NFC_SUCCESS
}

unsafe extern "C" fn pn53x_usb_abort_command(device: *mut nfc_device) -> c_int {
    if let Some(data) = driver_data(device) {
        data.abort_flag = true;
    }
    NFC_SUCCESS
}

static PN53X_USB_IO: pn53x_io = pn53x_io {
    send: Some(pn53x_usb_send),
    receive: Some(pn53x_usb_receive),
};

#[cfg(not(test))]
unsafe extern "C" {
    static pn53x_ack_frame: [u8; PN53X_ACK_FRAME_LEN];
    static pn53x_nack_frame: [u8; PN53X_ACK_FRAME_LEN];
    fn pn53x_build_frame(
        pbt_frame: *mut u8,
        psz_frame: *mut usize,
        pbt_data: *const u8,
        sz_data: usize,
    ) -> c_int;
    fn pn53x_check_ack_frame(
        pnd: *mut nfc_device,
        pbt_rx_frame: *const u8,
        sz_rx_frame_len: usize,
    ) -> c_int;
    fn pn53x_transceive(
        pnd: *mut nfc_device,
        pbt_tx: *const u8,
        sz_tx: usize,
        pbt_rx: *mut u8,
        sz_rx_len: usize,
        timeout: c_int,
    ) -> c_int;
    fn pn53x_write_register(pnd: *mut nfc_device, reg: u16, symbol_mask: u8, value: u8) -> c_int;
    fn pn53x_data_new(pnd: *mut nfc_device, io: *const pn53x_io) -> *mut c_void;
    fn pn53x_data_free(pnd: *mut nfc_device);
    fn pn53x_init(pnd: *mut nfc_device) -> c_int;
    fn pn53x_strerror(pnd: *const nfc_device) -> *const c_char;
    fn pn53x_initiator_init(pnd: *mut nfc_device) -> c_int;
    fn pn53x_initiator_select_passive_target(
        pnd: *mut nfc_device,
        nm: nfc_modulation,
        init_data: *const u8,
        init_data_len: usize,
        target: *mut nfc_target,
    ) -> c_int;
    fn pn53x_initiator_poll_target(
        pnd: *mut nfc_device,
        modulations: *const nfc_modulation,
        modulation_count: usize,
        poll_nr: u8,
        period: u8,
        target: *mut nfc_target,
    ) -> c_int;
    fn pn53x_initiator_select_dep_target(
        pnd: *mut nfc_device,
        ndm: nfc_dep_mode,
        nbr: nfc_baud_rate,
        initiator: *const nfc_dep_info,
        target: *mut nfc_target,
        timeout: c_int,
    ) -> c_int;
    fn pn53x_initiator_deselect_target(pnd: *mut nfc_device) -> c_int;
    fn pn53x_initiator_transceive_bytes(
        pnd: *mut nfc_device,
        tx: *const u8,
        tx_len: usize,
        rx: *mut u8,
        rx_len: usize,
        timeout: c_int,
    ) -> c_int;
    fn pn53x_initiator_transceive_bits(
        pnd: *mut nfc_device,
        tx: *const u8,
        tx_bits_len: usize,
        tx_parity: *const u8,
        rx: *mut u8,
        rx_parity: *mut u8,
    ) -> c_int;
    fn pn53x_initiator_transceive_bytes_timed(
        pnd: *mut nfc_device,
        tx: *const u8,
        tx_len: usize,
        rx: *mut u8,
        rx_len: usize,
        cycles: *mut u32,
    ) -> c_int;
    fn pn53x_initiator_transceive_bits_timed(
        pnd: *mut nfc_device,
        tx: *const u8,
        tx_bits_len: usize,
        tx_parity: *const u8,
        rx: *mut u8,
        rx_parity: *mut u8,
        cycles: *mut u32,
    ) -> c_int;
    fn pn53x_initiator_target_is_present(pnd: *mut nfc_device, target: *const nfc_target) -> c_int;
    fn pn53x_target_init(
        pnd: *mut nfc_device,
        target: *mut nfc_target,
        rx: *mut u8,
        rx_len: usize,
        timeout: c_int,
    ) -> c_int;
    fn pn53x_target_send_bytes(
        pnd: *mut nfc_device,
        tx: *const u8,
        tx_len: usize,
        timeout: c_int,
    ) -> c_int;
    fn pn53x_target_receive_bytes(
        pnd: *mut nfc_device,
        rx: *mut u8,
        rx_len: usize,
        timeout: c_int,
    ) -> c_int;
    fn pn53x_target_send_bits(
        pnd: *mut nfc_device,
        tx: *const u8,
        tx_bits_len: usize,
        tx_parity: *const u8,
    ) -> c_int;
    fn pn53x_target_receive_bits(
        pnd: *mut nfc_device,
        rx: *mut u8,
        rx_len: usize,
        rx_parity: *mut u8,
    ) -> c_int;
    fn pn53x_set_property_bool(pnd: *mut nfc_device, property: nfc_property, enable: bool)
    -> c_int;
    fn pn53x_set_property_int(pnd: *mut nfc_device, property: nfc_property, value: c_int) -> c_int;
    fn pn53x_get_supported_modulation(
        pnd: *mut nfc_device,
        mode: nfc_mode,
        supported: *mut *const nfc_modulation_type,
    ) -> c_int;
    fn pn53x_get_supported_baud_rate(
        pnd: *mut nfc_device,
        mode: nfc_mode,
        modulation_type: nfc_modulation_type,
        supported: *mut *const nfc_baud_rate,
    ) -> c_int;
    fn pn53x_get_information_about(pnd: *mut nfc_device, pbuf: *mut *mut c_char) -> c_int;
    fn pn53x_idle(pnd: *mut nfc_device) -> c_int;
    fn pn53x_PowerDown(pnd: *mut nfc_device) -> c_int;

    fn usb_get_device_list(list: *mut usb_device_list) -> c_int;
    fn usb_free_device_list(list: *mut usb_device_list);
    fn usb_get_bus_device_strings(
        device: *const usb_device,
        bus_buffer: *mut c_char,
        bus_buffer_size: usize,
        device_buffer: *mut c_char,
        device_buffer_size: usize,
    ) -> c_int;
    fn usb_open(device: *const usb_device, handle: *mut *mut usb_dev_handle) -> c_int;
    fn usb_close(handle: *mut usb_dev_handle) -> c_int;
    fn usb_set_configuration(handle: *mut usb_dev_handle, configuration_value: c_int) -> c_int;
    fn usb_claim_interface(handle: *mut usb_dev_handle, interface_number: c_int) -> c_int;
    fn usb_release_interface(handle: *mut usb_dev_handle, interface_number: c_int) -> c_int;
    fn usb_set_altinterface(
        handle: *mut usb_dev_handle,
        interface_number: c_int,
        alternate_setting: c_int,
    ) -> c_int;
    fn usb_bulk_read(
        handle: *mut usb_dev_handle,
        endpoint: c_uchar,
        data: *mut u8,
        size: usize,
        timeout: c_int,
    ) -> c_int;
    fn usb_bulk_write(
        handle: *mut usb_dev_handle,
        endpoint: c_uchar,
        data: *const u8,
        size: usize,
        timeout: c_int,
    ) -> c_int;
    fn usb_get_string_simple(
        handle: *mut usb_dev_handle,
        string_index: c_int,
        buffer: *mut c_char,
        buffer_size: usize,
    ) -> c_int;
    fn usb_strerror(result: c_int) -> *const c_char;
    fn usb_error_is_timeout(result: c_int) -> bool;
    fn usb_error_is_access(result: c_int) -> bool;
}

#[cfg(not(test))]
fn pn53x_ack_frame_bytes() -> &'static [u8] {
    unsafe { &pn53x_ack_frame }
}

#[cfg(not(test))]
fn pn53x_nack_frame_bytes() -> &'static [u8] {
    unsafe { &pn53x_nack_frame }
}

#[cfg(not(test))]
unsafe fn pn53x_build_frame_bridge(
    frame: *mut u8,
    frame_len: *mut usize,
    data: *const u8,
    data_len: usize,
) -> c_int {
    unsafe { pn53x_build_frame(frame, frame_len, data, data_len) }
}

#[cfg(not(test))]
unsafe fn pn53x_check_ack_frame_bridge(
    device: *mut nfc_device,
    frame: *const u8,
    frame_len: usize,
) -> c_int {
    unsafe { pn53x_check_ack_frame(device, frame, frame_len) }
}

#[cfg(not(test))]
unsafe fn pn53x_transceive_bridge(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: usize,
    rx: *mut u8,
    rx_len: usize,
    timeout: c_int,
) -> c_int {
    unsafe { pn53x_transceive(device, tx, tx_len, rx, rx_len, timeout) }
}

#[cfg(not(test))]
unsafe fn pn53x_write_register_bridge(
    device: *mut nfc_device,
    reg: u16,
    symbol_mask: u8,
    value: u8,
) -> c_int {
    unsafe { pn53x_write_register(device, reg, symbol_mask, value) }
}

#[cfg(not(test))]
unsafe fn pn53x_data_new_bridge(device: *mut nfc_device, io: *const pn53x_io) -> *mut c_void {
    unsafe { pn53x_data_new(device, io) }
}

#[cfg(not(test))]
unsafe fn pn53x_data_free_bridge(device: *mut nfc_device) {
    unsafe { pn53x_data_free(device) };
}

#[cfg(not(test))]
unsafe fn pn53x_init_bridge(device: *mut nfc_device) -> c_int {
    unsafe { pn53x_init(device) }
}

#[cfg(not(test))]
unsafe fn pn53x_set_property_bool_bridge(
    device: *mut nfc_device,
    property: nfc_property,
    enable: bool,
) -> c_int {
    unsafe { pn53x_set_property_bool(device, property, enable) }
}

#[cfg(not(test))]
unsafe fn pn53x_get_supported_modulation_bridge(
    device: *mut nfc_device,
    mode: nfc_mode,
    supported: *mut *const nfc_modulation_type,
) -> c_int {
    unsafe { pn53x_get_supported_modulation(device, mode, supported) }
}

#[cfg(not(test))]
unsafe fn pn53x_idle_bridge(device: *mut nfc_device) -> c_int {
    unsafe { pn53x_idle(device) }
}

#[cfg(not(test))]
unsafe fn usb_get_device_list_bridge(list: *mut usb_device_list) -> c_int {
    unsafe { usb_get_device_list(list) }
}

#[cfg(not(test))]
unsafe fn usb_free_device_list_bridge(list: *mut usb_device_list) {
    unsafe { usb_free_device_list(list) };
}

#[cfg(not(test))]
unsafe fn usb_get_bus_device_strings_bridge(
    device: *const usb_device,
    bus_buffer: *mut c_char,
    bus_buffer_size: usize,
    device_buffer: *mut c_char,
    device_buffer_size: usize,
) -> c_int {
    unsafe {
        usb_get_bus_device_strings(
            device,
            bus_buffer,
            bus_buffer_size,
            device_buffer,
            device_buffer_size,
        )
    }
}

#[cfg(not(test))]
unsafe fn usb_device_get_bulk_endpoints_bridge(
    device: *const usb_device,
    endpoints: *mut usb_bulk_endpoints,
) -> bool {
    unsafe { usb_device_get_bulk_endpoints(device, endpoints) }
}

#[cfg(not(test))]
unsafe fn usb_open_bridge(device: *const usb_device, handle: *mut *mut usb_dev_handle) -> c_int {
    unsafe { usb_open(device, handle) }
}

#[cfg(not(test))]
unsafe fn usb_close_bridge(handle: *mut usb_dev_handle) -> c_int {
    unsafe { usb_close(handle) }
}

#[cfg(not(test))]
unsafe fn usb_set_configuration_bridge(handle: *mut usb_dev_handle, configuration: c_int) -> c_int {
    unsafe { usb_set_configuration(handle, configuration) }
}

#[cfg(not(test))]
unsafe fn usb_claim_interface_bridge(
    handle: *mut usb_dev_handle,
    interface_number: c_int,
) -> c_int {
    unsafe { usb_claim_interface(handle, interface_number) }
}

#[cfg(not(test))]
unsafe fn usb_release_interface_bridge(
    handle: *mut usb_dev_handle,
    interface_number: c_int,
) -> c_int {
    unsafe { usb_release_interface(handle, interface_number) }
}

#[cfg(not(test))]
unsafe fn usb_set_altinterface_bridge(
    handle: *mut usb_dev_handle,
    interface_number: c_int,
    alternate_setting: c_int,
) -> c_int {
    unsafe { usb_set_altinterface(handle, interface_number, alternate_setting) }
}

#[cfg(not(test))]
unsafe fn usb_bulk_read_bridge(
    handle: *mut usb_dev_handle,
    endpoint: c_uchar,
    data: *mut u8,
    size: usize,
    timeout: c_int,
) -> c_int {
    unsafe { usb_bulk_read(handle, endpoint, data, size, timeout) }
}

#[cfg(not(test))]
unsafe fn usb_bulk_write_bridge(
    handle: *mut usb_dev_handle,
    endpoint: c_uchar,
    data: *const u8,
    size: usize,
    timeout: c_int,
) -> c_int {
    unsafe { usb_bulk_write(handle, endpoint, data, size, timeout) }
}

#[cfg(not(test))]
unsafe fn usb_get_string_simple_bridge(
    handle: *mut usb_dev_handle,
    string_index: c_int,
    buffer: *mut c_char,
    buffer_size: usize,
) -> c_int {
    unsafe { usb_get_string_simple(handle, string_index, buffer, buffer_size) }
}

#[cfg(not(test))]
unsafe fn usb_error_is_timeout_bridge(result: c_int) -> bool {
    unsafe { usb_error_is_timeout(result) }
}

#[cfg(not(test))]
unsafe fn usb_error_is_access_bridge(result: c_int) -> bool {
    unsafe { usb_error_is_access(result) }
}

#[cfg(not(test))]
fn usb_strerror_string(result: c_int) -> String {
    unsafe { CStr::from_ptr(usb_strerror(result)) }
        .to_string_lossy()
        .into_owned()
}

#[cfg(not(test))]
static PN53X_USB_DRIVER: nfc_driver = nfc_driver {
    name: PN53X_USB_DRIVER_NAME_CSTR,
    scan_type: scan_type_enum::NOT_INTRUSIVE,
    scan: Some(pn53x_usb_scan),
    open: Some(pn53x_usb_open),
    close: Some(pn53x_usb_close),
    strerror: Some(pn53x_strerror),
    initiator_init: Some(pn53x_initiator_init),
    initiator_init_secure_element: None,
    initiator_select_passive_target: Some(pn53x_initiator_select_passive_target),
    initiator_poll_target: Some(pn53x_initiator_poll_target),
    initiator_select_dep_target: Some(pn53x_initiator_select_dep_target),
    initiator_deselect_target: Some(pn53x_initiator_deselect_target),
    initiator_transceive_bytes: Some(pn53x_initiator_transceive_bytes),
    initiator_transceive_bits: Some(pn53x_initiator_transceive_bits),
    initiator_transceive_bytes_timed: Some(pn53x_initiator_transceive_bytes_timed),
    initiator_transceive_bits_timed: Some(pn53x_initiator_transceive_bits_timed),
    initiator_target_is_present: Some(pn53x_initiator_target_is_present),
    target_init: Some(pn53x_target_init),
    target_send_bytes: Some(pn53x_target_send_bytes),
    target_receive_bytes: Some(pn53x_target_receive_bytes),
    target_send_bits: Some(pn53x_target_send_bits),
    target_receive_bits: Some(pn53x_target_receive_bits),
    device_set_property_bool: Some(pn53x_usb_set_property_bool),
    device_set_property_int: Some(pn53x_set_property_int),
    get_supported_modulation: Some(pn53x_usb_get_supported_modulation),
    get_supported_baud_rate: Some(pn53x_get_supported_baud_rate),
    device_get_information_about: Some(pn53x_get_information_about),
    abort_command: Some(pn53x_usb_abort_command),
    idle: Some(pn53x_idle),
    powerdown: Some(pn53x_PowerDown),
};

#[cfg(test)]
const TEST_USB_ERROR_ACCESS: c_int = -3;
#[cfg(test)]
const TEST_USB_ERROR_TIMEOUT: c_int = -7;

#[cfg(test)]
const TEST_PN53X_ACK_FRAME: [u8; PN53X_ACK_FRAME_LEN] = [0x00, 0x00, 0xff, 0x00, 0xff, 0x00];
#[cfg(test)]
const TEST_PN53X_NACK_FRAME: [u8; PN53X_ACK_FRAME_LEN] = [0x00, 0x00, 0xff, 0xff, 0x00, 0x00];
#[cfg(test)]
static TEST_BAUD_RATES: [nfc_baud_rate; 2] = [nfc_baud_rate::NBR_106, nfc_baud_rate::NBR_UNDEFINED];
#[cfg(test)]
static TEST_MODULATION_TYPES: [nfc_modulation_type; 2] = [
    nfc_modulation_type::NMT_ISO14443A,
    nfc_modulation_type::NMT_UNDEFINED,
];

#[cfg(test)]
#[derive(Clone, Debug)]
struct FakeUsbDeviceDescriptor {
    vendor_id: u16,
    product_id: u16,
    configuration_value: u8,
    manufacturer_string_index: u8,
    product_string_index: u8,
    bus: String,
    node: String,
    manufacturer_string: Option<String>,
    product_string: Option<String>,
    bulk_endpoints: Option<usb_bulk_endpoints>,
    open_result: c_int,
    set_configuration_result: c_int,
    claim_interface_result: c_int,
    set_altinterface_result: c_int,
}

#[cfg(test)]
impl Default for FakeUsbDeviceDescriptor {
    fn default() -> Self {
        Self {
            vendor_id: 0,
            product_id: 0,
            configuration_value: 1,
            manufacturer_string_index: 1,
            product_string_index: 2,
            bus: "001".to_string(),
            node: "001".to_string(),
            manufacturer_string: None,
            product_string: None,
            bulk_endpoints: Some(usb_bulk_endpoints {
                interface_number: 0,
                alternate_setting: 0,
                endpoint_in: 0x84,
                endpoint_out: 0x04,
                max_packet_size: 0x40,
            }),
            open_result: 0,
            set_configuration_result: 0,
            claim_interface_result: 0,
            set_altinterface_result: 0,
        }
    }
}

#[cfg(test)]
struct FakeUsbHandle {
    manufacturer_string: Option<String>,
    product_string: Option<String>,
    set_configuration_result: c_int,
    claim_interface_result: c_int,
    set_altinterface_result: c_int,
}

#[cfg(test)]
#[derive(Clone, Debug, Default)]
struct FakePn53xTransceiveResponse {
    rc: c_int,
    response: Vec<u8>,
}

#[cfg(test)]
#[derive(Clone, Debug, Default)]
struct FakeUsbBulkRead {
    rc: c_int,
    data: Vec<u8>,
}

#[cfg(test)]
#[derive(Clone, Debug, Default)]
struct Pn53xUsbTestState {
    devices: Vec<FakeUsbDeviceDescriptor>,
    get_device_list_calls: usize,
    free_device_list_calls: usize,
    open_calls: usize,
    close_calls: usize,
    set_configuration_calls: Vec<c_int>,
    claim_interface_calls: Vec<c_int>,
    release_interface_calls: Vec<c_int>,
    set_altinterface_calls: Vec<(c_int, c_int)>,
    bulk_reads: VecDeque<FakeUsbBulkRead>,
    bulk_writes: Vec<Vec<u8>>,
    bulk_write_results: VecDeque<c_int>,
    pn53x_data_new_calls: usize,
    pn53x_data_free_calls: usize,
    pn53x_init_result: c_int,
    pn53x_init_calls: usize,
    pn53x_transceive_calls: Vec<Vec<u8>>,
    pn53x_transceive_responses: VecDeque<FakePn53xTransceiveResponse>,
    pn53x_write_register_calls: Vec<(u16, u8, u8)>,
    pn53x_set_property_bool_calls: Vec<(nfc_property, bool)>,
    pn53x_get_supported_modulation_calls: Vec<nfc_mode>,
    pn53x_idle_calls: usize,
    pn53x_powerdown_calls: usize,
}

#[cfg(test)]
fn test_state() -> &'static Mutex<Pn53xUsbTestState> {
    static STATE: OnceLock<Mutex<Pn53xUsbTestState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(Pn53xUsbTestState::default()))
}

#[cfg(test)]
fn pn53x_ack_frame_bytes() -> &'static [u8] {
    &TEST_PN53X_ACK_FRAME
}

#[cfg(test)]
fn pn53x_nack_frame_bytes() -> &'static [u8] {
    &TEST_PN53X_NACK_FRAME
}

#[cfg(test)]
unsafe fn pn53x_build_frame_bridge(
    frame: *mut u8,
    frame_len: *mut usize,
    data: *const u8,
    data_len: usize,
) -> c_int {
    if frame.is_null() || frame_len.is_null() || data.is_null() || data_len == 0 {
        return NFC_EINVARG;
    }

    let source = unsafe { slice::from_raw_parts(data, data_len) };
    let destination = unsafe { slice::from_raw_parts_mut(frame, PN53X_USB_BUFFER_LEN) };
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
        destination[..9].copy_from_slice(&[
            0x00,
            0x00,
            0xff,
            0xff,
            0xff,
            ((data_len + 1) >> 8) as u8,
            ((data_len + 1) & 0xff) as u8,
            (0u8).wrapping_sub(
                (((data_len + 1) >> 8) as u8).wrapping_add(((data_len + 1) & 0xff) as u8),
            ),
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
unsafe fn pn53x_check_ack_frame_bridge(
    _device: *mut nfc_device,
    frame: *const u8,
    frame_len: usize,
) -> c_int {
    if frame_len >= TEST_PN53X_ACK_FRAME.len()
        && unsafe { slice::from_raw_parts(frame, TEST_PN53X_ACK_FRAME.len()) }
            == TEST_PN53X_ACK_FRAME
    {
        0
    } else {
        NFC_EIO
    }
}

#[cfg(test)]
unsafe fn pn53x_transceive_bridge(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: usize,
    rx: *mut u8,
    rx_len: usize,
    _timeout: c_int,
) -> c_int {
    let tx_bytes = if tx.is_null() {
        Vec::new()
    } else {
        unsafe { slice::from_raw_parts(tx, tx_len) }.to_vec()
    };
    let mut state = test_state().lock().unwrap();
    state.pn53x_transceive_calls.push(tx_bytes.clone());
    drop(state);

    if let Some(chip) = chip_data(device) {
        if let Some(first) = tx_bytes.first() {
            chip.last_command = *first;
        }
    }

    let mut state = test_state().lock().unwrap();
    let response = state
        .pn53x_transceive_responses
        .pop_front()
        .unwrap_or_default();
    if response.rc >= 0 && !rx.is_null() && rx_len != 0 {
        let copy_len = response.response.len().min(rx_len);
        unsafe { ptr::copy_nonoverlapping(response.response.as_ptr(), rx, copy_len) };
    }
    response.rc
}

#[cfg(test)]
unsafe fn pn53x_write_register_bridge(
    _device: *mut nfc_device,
    reg: u16,
    symbol_mask: u8,
    value: u8,
) -> c_int {
    test_state()
        .lock()
        .unwrap()
        .pn53x_write_register_calls
        .push((reg, symbol_mask, value));
    NFC_SUCCESS
}

#[cfg(test)]
unsafe fn pn53x_data_new_bridge(device: *mut nfc_device, io: *const pn53x_io) -> *mut c_void {
    let allocation = unsafe { libc::calloc(1, size_of::<pn53x_data>()) } as *mut pn53x_data;
    if allocation.is_null() {
        return ptr::null_mut();
    }

    unsafe {
        (*allocation).io = io;
        (*allocation).type_ = pn53x_type::PN53X;
        (*allocation).power_mode = pn53x_power_mode::NORMAL;
        (*allocation).operating_mode = pn53x_operating_mode::INITIATOR;
        (*allocation).sam_mode = pn532_sam_mode::PSM_NORMAL;
        (*allocation).timeout_command = 350;
        (*allocation).timeout_atr = 103;
        (*allocation).timeout_communication = 52;
        (*device).chip_data = allocation.cast::<c_void>();
    }
    test_state().lock().unwrap().pn53x_data_new_calls += 1;
    allocation.cast::<c_void>()
}

#[cfg(test)]
unsafe fn pn53x_data_free_bridge(device: *mut nfc_device) {
    if let Some(device) = unsafe { as_mut(device) } {
        if !device.chip_data.is_null() {
            unsafe { crate::release_allocated_ptr(device.chip_data) };
            device.chip_data = ptr::null_mut();
        }
    }
    test_state().lock().unwrap().pn53x_data_free_calls += 1;
}

#[cfg(test)]
unsafe fn pn53x_init_bridge(_device: *mut nfc_device) -> c_int {
    let mut state = test_state().lock().unwrap();
    state.pn53x_init_calls += 1;
    state.pn53x_init_result
}

#[cfg(test)]
unsafe fn pn53x_set_property_bool_bridge(
    _device: *mut nfc_device,
    property: nfc_property,
    enable: bool,
) -> c_int {
    test_state()
        .lock()
        .unwrap()
        .pn53x_set_property_bool_calls
        .push((property, enable));
    NFC_SUCCESS
}

#[cfg(test)]
unsafe fn pn53x_get_supported_modulation_bridge(
    _device: *mut nfc_device,
    mode: nfc_mode,
    supported: *mut *const nfc_modulation_type,
) -> c_int {
    test_state()
        .lock()
        .unwrap()
        .pn53x_get_supported_modulation_calls
        .push(mode);
    if let Some(supported) = unsafe { as_mut(supported) } {
        *supported = TEST_MODULATION_TYPES.as_ptr();
    }
    NFC_SUCCESS
}

#[cfg(test)]
unsafe fn pn53x_idle_bridge(_device: *mut nfc_device) -> c_int {
    test_state().lock().unwrap().pn53x_idle_calls += 1;
    NFC_SUCCESS
}

#[cfg(test)]
unsafe fn usb_get_device_list_bridge(list: *mut usb_device_list) -> c_int {
    let Some(list) = (unsafe { as_mut(list) }) else {
        return NFC_EINVARG;
    };
    let mut state = test_state().lock().unwrap();
    state.get_device_list_calls += 1;
    let templates = state.devices.clone();
    drop(state);

    let mut devices = Vec::with_capacity(templates.len());
    for template in templates {
        let native = Box::into_raw(Box::new(template.clone())) as *mut c_void;
        devices.push(usb_device {
            native_device: native,
            vendor_id: template.vendor_id,
            product_id: template.product_id,
            manufacturer_string_index: template.manufacturer_string_index,
            product_string_index: template.product_string_index,
            bus_number: template.bus.parse().unwrap_or(0),
            device_address: template.node.parse().unwrap_or(0),
            configuration_value: template.configuration_value,
            interface_count: 0,
            interfaces: ptr::null_mut(),
        });
    }

    let boxed = devices.into_boxed_slice();
    list.count = boxed.len();
    list.devices = Box::into_raw(boxed) as *mut usb_device;
    0
}

#[cfg(test)]
unsafe fn usb_free_device_list_bridge(list: *mut usb_device_list) {
    let Some(list) = (unsafe { as_mut(list) }) else {
        return;
    };
    if !list.devices.is_null() {
        let devices = unsafe { Vec::from_raw_parts(list.devices, list.count, list.count) };
        for device in devices {
            if !device.native_device.is_null() {
                unsafe {
                    drop(Box::from_raw(
                        device.native_device.cast::<FakeUsbDeviceDescriptor>(),
                    ))
                };
            }
        }
    }
    list.devices = ptr::null_mut();
    list.count = 0;
    test_state().lock().unwrap().free_device_list_calls += 1;
}

#[cfg(test)]
unsafe fn usb_get_bus_device_strings_bridge(
    device: *const usb_device,
    bus_buffer: *mut c_char,
    bus_buffer_size: usize,
    device_buffer: *mut c_char,
    device_buffer_size: usize,
) -> c_int {
    let Some(device) = (unsafe { as_ref(device) }) else {
        return NFC_EINVARG;
    };
    let Some(template) =
        (unsafe { as_ref(device.native_device.cast::<FakeUsbDeviceDescriptor>()) })
    else {
        return NFC_EINVARG;
    };
    let ok_bus =
        unsafe { copy_bytes_to_c_buffer(bus_buffer, bus_buffer_size, template.bus.as_bytes()) };
    let ok_device = unsafe {
        copy_bytes_to_c_buffer(device_buffer, device_buffer_size, template.node.as_bytes())
    };
    if ok_bus && ok_device { 0 } else { NFC_EINVARG }
}

#[cfg(test)]
unsafe fn usb_device_get_bulk_endpoints_bridge(
    device: *const usb_device,
    endpoints: *mut usb_bulk_endpoints,
) -> bool {
    let Some(device) = (unsafe { as_ref(device) }) else {
        return false;
    };
    let Some(template) =
        (unsafe { as_ref(device.native_device.cast::<FakeUsbDeviceDescriptor>()) })
    else {
        return false;
    };
    let Some(endpoints) = (unsafe { as_mut(endpoints) }) else {
        return false;
    };
    if let Some(fake) = template.bulk_endpoints {
        *endpoints = fake;
        true
    } else {
        false
    }
}

#[cfg(test)]
unsafe fn usb_open_bridge(device: *const usb_device, handle: *mut *mut usb_dev_handle) -> c_int {
    let Some(device) = (unsafe { as_ref(device) }) else {
        return NFC_EINVARG;
    };
    let Some(handle_out) = (unsafe { as_mut(handle) }) else {
        return NFC_EINVARG;
    };
    let Some(template) =
        (unsafe { as_ref(device.native_device.cast::<FakeUsbDeviceDescriptor>()) })
    else {
        return NFC_EINVARG;
    };
    test_state().lock().unwrap().open_calls += 1;
    if template.open_result < 0 {
        *handle_out = ptr::null_mut();
        return template.open_result;
    }

    let boxed = Box::new(FakeUsbHandle {
        manufacturer_string: template.manufacturer_string.clone(),
        product_string: template.product_string.clone(),
        set_configuration_result: template.set_configuration_result,
        claim_interface_result: template.claim_interface_result,
        set_altinterface_result: template.set_altinterface_result,
    });
    *handle_out = Box::into_raw(boxed) as *mut usb_dev_handle;
    0
}

#[cfg(test)]
unsafe fn usb_close_bridge(handle: *mut usb_dev_handle) -> c_int {
    if !handle.is_null() {
        unsafe { drop(Box::from_raw(handle.cast::<FakeUsbHandle>())) };
    }
    test_state().lock().unwrap().close_calls += 1;
    0
}

#[cfg(test)]
unsafe fn usb_set_configuration_bridge(handle: *mut usb_dev_handle, configuration: c_int) -> c_int {
    let Some(handle) = (unsafe { as_ref(handle.cast::<FakeUsbHandle>()) }) else {
        return NFC_EINVARG;
    };
    test_state()
        .lock()
        .unwrap()
        .set_configuration_calls
        .push(configuration);
    handle.set_configuration_result
}

#[cfg(test)]
unsafe fn usb_claim_interface_bridge(
    handle: *mut usb_dev_handle,
    interface_number: c_int,
) -> c_int {
    let Some(handle) = (unsafe { as_ref(handle.cast::<FakeUsbHandle>()) }) else {
        return NFC_EINVARG;
    };
    test_state()
        .lock()
        .unwrap()
        .claim_interface_calls
        .push(interface_number);
    handle.claim_interface_result
}

#[cfg(test)]
unsafe fn usb_release_interface_bridge(
    _handle: *mut usb_dev_handle,
    interface_number: c_int,
) -> c_int {
    test_state()
        .lock()
        .unwrap()
        .release_interface_calls
        .push(interface_number);
    0
}

#[cfg(test)]
unsafe fn usb_set_altinterface_bridge(
    handle: *mut usb_dev_handle,
    interface_number: c_int,
    alternate_setting: c_int,
) -> c_int {
    let Some(handle) = (unsafe { as_ref(handle.cast::<FakeUsbHandle>()) }) else {
        return NFC_EINVARG;
    };
    test_state()
        .lock()
        .unwrap()
        .set_altinterface_calls
        .push((interface_number, alternate_setting));
    handle.set_altinterface_result
}

#[cfg(test)]
unsafe fn usb_bulk_read_bridge(
    _handle: *mut usb_dev_handle,
    _endpoint: c_uchar,
    data: *mut u8,
    size: usize,
    _timeout: c_int,
) -> c_int {
    let mut state = test_state().lock().unwrap();
    let response = state.bulk_reads.pop_front().unwrap_or_default();
    if response.rc >= 0 && !data.is_null() {
        let copy_len = response.data.len().min(size);
        unsafe { ptr::copy_nonoverlapping(response.data.as_ptr(), data, copy_len) };
    }
    if response.rc != 0 {
        response.rc
    } else {
        response.data.len() as c_int
    }
}

#[cfg(test)]
unsafe fn usb_bulk_write_bridge(
    _handle: *mut usb_dev_handle,
    _endpoint: c_uchar,
    data: *const u8,
    size: usize,
    _timeout: c_int,
) -> c_int {
    let bytes = if size == 0 || data.is_null() {
        Vec::new()
    } else {
        unsafe { slice::from_raw_parts(data, size) }.to_vec()
    };
    let mut state = test_state().lock().unwrap();
    state.bulk_writes.push(bytes);
    state
        .bulk_write_results
        .pop_front()
        .unwrap_or(size as c_int)
}

#[cfg(test)]
unsafe fn usb_get_string_simple_bridge(
    handle: *mut usb_dev_handle,
    string_index: c_int,
    buffer: *mut c_char,
    buffer_size: usize,
) -> c_int {
    let Some(handle) = (unsafe { as_ref(handle.cast::<FakeUsbHandle>()) }) else {
        return NFC_EINVARG;
    };
    let value = match string_index {
        1 => handle.manufacturer_string.as_deref(),
        2 => handle.product_string.as_deref(),
        _ => None,
    };
    let Some(value) = value else {
        return 0;
    };
    if unsafe { copy_bytes_to_c_buffer(buffer, buffer_size, value.as_bytes()) } {
        value.len() as c_int
    } else {
        NFC_EINVARG
    }
}

#[cfg(test)]
unsafe fn usb_error_is_timeout_bridge(result: c_int) -> bool {
    result == TEST_USB_ERROR_TIMEOUT
}

#[cfg(test)]
unsafe fn usb_error_is_access_bridge(result: c_int) -> bool {
    result == TEST_USB_ERROR_ACCESS
}

#[cfg(test)]
fn usb_strerror_string(result: c_int) -> String {
    match result {
        TEST_USB_ERROR_ACCESS => "access denied".to_string(),
        TEST_USB_ERROR_TIMEOUT => "timeout".to_string(),
        _ => format!("usb error {result}"),
    }
}

#[cfg(test)]
unsafe extern "C" fn test_pn53x_strerror(_device: *const nfc_device) -> *const c_char {
    b"pn53x test error\0".as_ptr().cast()
}

#[cfg(test)]
unsafe extern "C" fn test_pn53x_simple_success(_device: *mut nfc_device) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
unsafe extern "C" fn test_pn53x_simple_success_with_target(
    _device: *mut nfc_device,
    _target: *const nfc_target,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
unsafe extern "C" fn test_pn53x_select_passive_target(
    _device: *mut nfc_device,
    _nm: nfc_modulation,
    _init_data: *const u8,
    _init_data_len: usize,
    _target: *mut nfc_target,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
unsafe extern "C" fn test_pn53x_poll_target(
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
unsafe extern "C" fn test_pn53x_select_dep_target(
    _device: *mut nfc_device,
    _ndm: nfc_dep_mode,
    _nbr: nfc_baud_rate,
    _initiator: *const nfc_dep_info,
    _target: *mut nfc_target,
    _timeout: c_int,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
unsafe extern "C" fn test_pn53x_transceive_bytes(
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
unsafe extern "C" fn test_pn53x_transceive_bits(
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
unsafe extern "C" fn test_pn53x_transceive_bytes_timed(
    _device: *mut nfc_device,
    _tx: *const u8,
    _tx_len: usize,
    _rx: *mut u8,
    _rx_len: usize,
    cycles: *mut u32,
) -> c_int {
    if let Some(cycles) = unsafe { as_mut(cycles) } {
        *cycles = 0;
    }
    NFC_SUCCESS
}

#[cfg(test)]
unsafe extern "C" fn test_pn53x_transceive_bits_timed(
    _device: *mut nfc_device,
    _tx: *const u8,
    _tx_bits_len: usize,
    _tx_parity: *const u8,
    _rx: *mut u8,
    _rx_parity: *mut u8,
    cycles: *mut u32,
) -> c_int {
    if let Some(cycles) = unsafe { as_mut(cycles) } {
        *cycles = 0;
    }
    NFC_SUCCESS
}

#[cfg(test)]
unsafe extern "C" fn test_pn53x_target_init(
    _device: *mut nfc_device,
    _target: *mut nfc_target,
    _rx: *mut u8,
    _rx_len: usize,
    _timeout: c_int,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
unsafe extern "C" fn test_pn53x_target_send_bytes(
    _device: *mut nfc_device,
    _tx: *const u8,
    _tx_len: usize,
    _timeout: c_int,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
unsafe extern "C" fn test_pn53x_target_receive_bytes(
    _device: *mut nfc_device,
    _rx: *mut u8,
    _rx_len: usize,
    _timeout: c_int,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
unsafe extern "C" fn test_pn53x_target_send_bits(
    _device: *mut nfc_device,
    _tx: *const u8,
    _tx_bits_len: usize,
    _tx_parity: *const u8,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
unsafe extern "C" fn test_pn53x_target_receive_bits(
    _device: *mut nfc_device,
    _rx: *mut u8,
    _rx_len: usize,
    _rx_parity: *mut u8,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
unsafe extern "C" fn test_pn53x_set_property_int(
    _device: *mut nfc_device,
    _property: nfc_property,
    _value: c_int,
) -> c_int {
    NFC_SUCCESS
}

#[cfg(test)]
unsafe extern "C" fn test_pn53x_get_supported_baud_rate(
    _device: *mut nfc_device,
    _mode: nfc_mode,
    _modulation_type: nfc_modulation_type,
    supported: *mut *const nfc_baud_rate,
) -> c_int {
    if let Some(supported) = unsafe { as_mut(supported) } {
        *supported = TEST_BAUD_RATES.as_ptr();
    }
    NFC_SUCCESS
}

#[cfg(test)]
unsafe extern "C" fn test_pn53x_get_information_about(
    _device: *mut nfc_device,
    buffer: *mut *mut c_char,
) -> c_int {
    let Some(buffer) = (unsafe { as_mut(buffer) }) else {
        return NFC_EINVARG;
    };
    let bytes = b"chip: test\n";
    let allocation = unsafe { libc::malloc(bytes.len() + 1) as *mut c_char };
    if allocation.is_null() {
        return NFC_EIO;
    }
    let _ = unsafe { copy_bytes_to_c_buffer(allocation, bytes.len() + 1, bytes) };
    *buffer = allocation;
    bytes.len() as c_int + 1
}

#[cfg(test)]
unsafe extern "C" fn test_pn53x_idle(device: *mut nfc_device) -> c_int {
    unsafe { pn53x_idle_bridge(device) }
}

#[cfg(test)]
unsafe extern "C" fn test_pn53x_powerdown(_device: *mut nfc_device) -> c_int {
    test_state().lock().unwrap().pn53x_powerdown_calls += 1;
    NFC_SUCCESS
}

#[cfg(test)]
static PN53X_USB_DRIVER: nfc_driver = nfc_driver {
    name: PN53X_USB_DRIVER_NAME_CSTR,
    scan_type: scan_type_enum::NOT_INTRUSIVE,
    scan: Some(pn53x_usb_scan),
    open: Some(pn53x_usb_open),
    close: Some(pn53x_usb_close),
    strerror: Some(test_pn53x_strerror),
    initiator_init: Some(test_pn53x_simple_success),
    initiator_init_secure_element: None,
    initiator_select_passive_target: Some(test_pn53x_select_passive_target),
    initiator_poll_target: Some(test_pn53x_poll_target),
    initiator_select_dep_target: Some(test_pn53x_select_dep_target),
    initiator_deselect_target: Some(test_pn53x_simple_success),
    initiator_transceive_bytes: Some(test_pn53x_transceive_bytes),
    initiator_transceive_bits: Some(test_pn53x_transceive_bits),
    initiator_transceive_bytes_timed: Some(test_pn53x_transceive_bytes_timed),
    initiator_transceive_bits_timed: Some(test_pn53x_transceive_bits_timed),
    initiator_target_is_present: Some(test_pn53x_simple_success_with_target),
    target_init: Some(test_pn53x_target_init),
    target_send_bytes: Some(test_pn53x_target_send_bytes),
    target_receive_bytes: Some(test_pn53x_target_receive_bytes),
    target_send_bits: Some(test_pn53x_target_send_bits),
    target_receive_bits: Some(test_pn53x_target_receive_bits),
    device_set_property_bool: Some(pn53x_usb_set_property_bool),
    device_set_property_int: Some(test_pn53x_set_property_int),
    get_supported_modulation: Some(pn53x_usb_get_supported_modulation),
    get_supported_baud_rate: Some(test_pn53x_get_supported_baud_rate),
    device_get_information_about: Some(test_pn53x_get_information_about),
    abort_command: Some(pn53x_usb_abort_command),
    idle: Some(test_pn53x_idle),
    powerdown: Some(test_pn53x_powerdown),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::{nfc_context_alloc_defaults, nfc_context_free};

    fn test_guard() -> &'static Mutex<()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD.get_or_init(|| Mutex::new(()))
    }

    fn reset_test_world() {
        *test_state().lock().unwrap() = Pn53xUsbTestState::default();
    }

    fn supported_device(vendor_id: u16, product_id: u16) -> FakeUsbDeviceDescriptor {
        FakeUsbDeviceDescriptor {
            vendor_id,
            product_id,
            manufacturer_string: Some("Test Maker".to_string()),
            product_string: Some("USB Reader".to_string()),
            ..Default::default()
        }
    }

    fn build_response_frame(command: u8, payload: &[u8]) -> Vec<u8> {
        let len = (payload.len() + 2) as u8;
        let mut frame = vec![0x00, 0x00, 0xff, len, len.wrapping_neg(), 0xD5, command + 1];
        frame.extend_from_slice(payload);
        let mut dcs = 0u8.wrapping_sub(0xD5).wrapping_sub(command + 1);
        for byte in payload {
            dcs = dcs.wrapping_sub(*byte);
        }
        frame.push(dcs);
        frame.push(0x00);
        frame
    }

    fn open_device(connstring: &CString) -> *mut nfc_device {
        let context = unsafe { nfc_context_alloc_defaults() };
        let device = unsafe { pn53x_usb_open(context, connstring.as_ptr()) };
        assert!(!device.is_null());
        unsafe { nfc_context_free(context) };
        device
    }

    #[test]
    fn scan_lists_supported_usb_devices() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        test_state().lock().unwrap().devices = vec![
            supported_device(0x04CC, 0x2533),
            supported_device(0xFFFF, 0xEEEE),
            FakeUsbDeviceDescriptor {
                vendor_id: 0x04E6,
                product_id: 0x5594,
                bus: "001".into(),
                node: "003".into(),
                manufacturer_string: Some("SCM".into()),
                product_string: Some("SCL3712".into()),
                ..Default::default()
            },
        ];

        let context = unsafe { nfc_context_alloc_defaults() };
        let mut connstrings = [[0 as c_char; NFC_BUFSIZE_CONNSTRING]; 4];
        let found = unsafe { pn53x_usb_scan(context, connstrings.as_mut_ptr(), connstrings.len()) };
        unsafe { nfc_context_free(context) };

        assert_eq!(found, 2);
        assert_eq!(
            fixed_c_buffer_to_string(&connstrings[0]),
            "pn53x_usb:001:001"
        );
        assert_eq!(
            fixed_c_buffer_to_string(&connstrings[1]),
            "pn53x_usb:001:003"
        );

        let snapshot = test_state().lock().unwrap().clone();
        assert_eq!(snapshot.get_device_list_calls, 1);
        assert_eq!(snapshot.open_calls, 2);
        assert_eq!(snapshot.close_calls, 2);
        assert_eq!(snapshot.set_configuration_calls, vec![1, 1]);
    }

    #[test]
    fn open_and_close_manage_usb_and_chip_state() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        test_state().lock().unwrap().devices = vec![supported_device(0x04CC, 0x2533)];
        let connstring = CString::new("pn53x_usb").unwrap();
        let device = open_device(&connstring);

        assert_eq!(
            fixed_c_buffer_to_string(unsafe { &(*device).name }),
            "Test Maker / USB Reader"
        );
        let driver = driver_data(device).unwrap();
        assert_eq!(driver.model, pn53x_usb_model::NxpPn533);
        assert_eq!(driver.uiEndPointIn, 0x84);
        assert_eq!(driver.uiEndPointOut, 0x04);
        assert_eq!(driver.uiMaxPacketSize, 0x40);

        unsafe { pn53x_usb_close(device) };

        let snapshot = test_state().lock().unwrap().clone();
        assert_eq!(snapshot.pn53x_data_new_calls, 1);
        assert_eq!(snapshot.pn53x_init_calls, 1);
        assert_eq!(snapshot.pn53x_idle_calls, 1);
        assert_eq!(snapshot.pn53x_data_free_calls, 1);
        assert_eq!(snapshot.release_interface_calls, vec![0]);
        assert_eq!(snapshot.close_calls, 1);
    }

    #[test]
    fn send_and_receive_follow_ack_and_frame_rules() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        test_state().lock().unwrap().devices = vec![supported_device(0x04CC, 0x2533)];
        let connstring = CString::new("pn53x_usb").unwrap();
        let device = open_device(&connstring);

        {
            let mut state = test_state().lock().unwrap();
            state.bulk_reads.push_back(FakeUsbBulkRead {
                rc: TEST_PN53X_ACK_FRAME.len() as c_int,
                data: TEST_PN53X_ACK_FRAME.to_vec(),
            });
        }

        let command = [0x4A, 0x01, 0x00];
        let send_rc = unsafe { pn53x_usb_send(device, command.as_ptr(), command.len(), 250) };
        assert_eq!(send_rc, NFC_SUCCESS);
        assert!(!test_state().lock().unwrap().bulk_writes.is_empty());

        chip_data(device).unwrap().last_command = command[0];
        test_state()
            .lock()
            .unwrap()
            .bulk_reads
            .push_back(FakeUsbBulkRead {
                rc: 0,
                data: build_response_frame(command[0], &[0x11, 0x22, 0x33]),
            });

        let mut rx = [0u8; 8];
        let receive_rc = unsafe { pn53x_usb_receive(device, rx.as_mut_ptr(), rx.len(), 250) };
        assert_eq!(receive_rc, 3);
        assert_eq!(&rx[..3], &[0x11, 0x22, 0x33]);

        unsafe { pn53x_usb_close(device) };
    }

    #[test]
    fn receive_honors_abort_flag_after_timeout() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        test_state().lock().unwrap().devices = vec![supported_device(0x04CC, 0x2533)];
        let connstring = CString::new("pn53x_usb").unwrap();
        let device = open_device(&connstring);

        driver_data(device).unwrap().abort_flag = true;
        test_state()
            .lock()
            .unwrap()
            .bulk_reads
            .push_back(FakeUsbBulkRead {
                rc: TEST_USB_ERROR_TIMEOUT,
                data: Vec::new(),
            });

        let mut rx = [0u8; 4];
        let rc =
            unsafe { pn53x_usb_receive(device, rx.as_mut_ptr(), rx.len(), USB_INFINITE_TIMEOUT) };
        assert_eq!(rc, NFC_EOPABORTED);
        assert!(!driver_data(device).unwrap().abort_flag);

        unsafe { pn53x_usb_close(device) };
    }

    #[test]
    fn ask_logo_reports_no_target_support() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        test_state().lock().unwrap().devices = vec![supported_device(0x1FD3, 0x0608)];
        let connstring = CString::new("pn53x_usb").unwrap();
        let device = open_device(&connstring);
        let mut supported = ptr::null();

        let rc = unsafe {
            pn53x_usb_get_supported_modulation(
                device,
                nfc_mode::N_TARGET,
                ptr::addr_of_mut!(supported),
            )
        };
        assert_eq!(rc, NFC_SUCCESS);
        assert!(!supported.is_null());
        assert_eq!(unsafe { *supported }, nfc_modulation_type::NMT_UNDEFINED);

        unsafe { pn53x_usb_close(device) };
    }
}
