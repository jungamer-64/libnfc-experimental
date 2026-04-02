// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// USB helper layer backed by `proximate::native_helpers`.

#![allow(non_camel_case_types)]

use crate::ffi_support::{as_mut, as_ref, copy_bytes_with_truncation};
use ::proximate::native_helpers::usb::{
    UsbBulkEndpoints as NativeUsbBulkEndpoints, UsbDeviceInfo as NativeUsbDeviceInfo,
    UsbDeviceSelector, UsbError as NativeUsbError, UsbHandle as NativeUsbHandle,
    bulk_endpoints as native_bulk_endpoints, bus_device_strings as native_bus_device_strings,
    list_devices as native_list_devices, prepare as native_prepare,
};
use libc::{c_char, c_int, c_void, size_t};
use std::ptr;
use std::slice;

const USB_SUCCESS: c_int = 0;
const USB_ERROR_IO: c_int = -1;
const USB_ERROR_INVALID_PARAM: c_int = -2;
const USB_ERROR_ACCESS: c_int = -3;
const USB_ERROR_NO_DEVICE: c_int = -4;
const USB_ERROR_NOT_FOUND: c_int = -5;
const USB_ERROR_BUSY: c_int = -6;
const USB_ERROR_TIMEOUT: c_int = -7;
const USB_ERROR_OVERFLOW: c_int = -8;
const USB_ERROR_PIPE: c_int = -9;
const USB_ERROR_INTERRUPTED: c_int = -10;
const USB_ERROR_NO_MEM: c_int = -11;
const USB_ERROR_NOT_SUPPORTED: c_int = -12;
const USB_ERROR_OTHER: c_int = -99;

const USB_ENDPOINT_TYPE_MASK: u8 = 0x03;
const USB_ENDPOINT_TYPE_BULK: u8 = 0x02;
const USB_ENDPOINT_DIR_MASK: u8 = 0x80;
const USB_ENDPOINT_IN: u8 = 0x80;
const USB_ENDPOINT_OUT: u8 = 0x00;
const INVALID_STRING_DESCRIPTOR_FALLBACK: &[u8] = b"?";

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct usb_endpoint_descriptor {
    pub address: u8,
    pub attributes: u8,
    pub max_packet_size: u16,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct usb_interface_descriptor {
    pub number: u8,
    pub alternate_setting: u8,
    pub endpoint_count: size_t,
    pub endpoints: *mut usb_endpoint_descriptor,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct usb_device {
    pub native_device: *mut c_void,
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer_string_index: u8,
    pub product_string_index: u8,
    pub bus_number: u8,
    pub device_address: u8,
    pub configuration_value: u8,
    pub interface_count: size_t,
    pub interfaces: *mut usb_interface_descriptor,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct usb_device_list {
    pub devices: *mut usb_device,
    pub count: size_t,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct usb_bulk_endpoints {
    pub interface_number: u8,
    pub alternate_setting: c_int,
    pub endpoint_in: u8,
    pub endpoint_out: u8,
    pub max_packet_size: u16,
}

#[repr(C)]
pub struct usb_dev_handle {
    _private: [u8; 0],
}

struct UsbHandleState {
    handle: NativeUsbHandle,
}

fn map_usb_error(error: NativeUsbError) -> c_int {
    match error {
        NativeUsbError::Io => USB_ERROR_IO,
        NativeUsbError::InvalidParam => USB_ERROR_INVALID_PARAM,
        NativeUsbError::Access => USB_ERROR_ACCESS,
        NativeUsbError::NoDevice => USB_ERROR_NO_DEVICE,
        NativeUsbError::NotFound => USB_ERROR_NOT_FOUND,
        NativeUsbError::Busy => USB_ERROR_BUSY,
        NativeUsbError::Timeout => USB_ERROR_TIMEOUT,
        NativeUsbError::Overflow => USB_ERROR_OVERFLOW,
        NativeUsbError::Pipe => USB_ERROR_PIPE,
        NativeUsbError::Interrupted => USB_ERROR_INTERRUPTED,
        NativeUsbError::NoMem => USB_ERROR_NO_MEM,
        NativeUsbError::NotSupported => USB_ERROR_NOT_SUPPORTED,
        NativeUsbError::Other => USB_ERROR_OTHER,
    }
}

fn map_bulk_endpoints(value: NativeUsbBulkEndpoints) -> usb_bulk_endpoints {
    usb_bulk_endpoints {
        interface_number: value.interface_number,
        alternate_setting: value.alternate_setting,
        endpoint_in: value.endpoint_in,
        endpoint_out: value.endpoint_out,
        max_packet_size: value.max_packet_size,
    }
}

unsafe fn handle_state<'a>(handle: *mut usb_dev_handle) -> Result<&'a mut UsbHandleState, c_int> {
    let Some(handle_ref) = (unsafe { as_mut(handle.cast::<UsbHandleState>()) }) else {
        return Err(USB_ERROR_INVALID_PARAM);
    };
    Ok(handle_ref)
}

unsafe fn native_device<'a>(
    device: *const usb_device,
) -> Result<Option<&'a NativeUsbDeviceInfo>, c_int> {
    let Some(device_ref) = (unsafe { as_ref(device) }) else {
        return Err(USB_ERROR_INVALID_PARAM);
    };
    if device_ref.native_device.is_null() {
        return Ok(None);
    }
    Ok(unsafe { as_ref(device_ref.native_device.cast::<NativeUsbDeviceInfo>()) })
}

unsafe fn free_usb_device(device: &mut usb_device) {
    if !device.interfaces.is_null() {
        let interfaces = unsafe {
            Vec::from_raw_parts(
                device.interfaces,
                device.interface_count,
                device.interface_count,
            )
        };
        for interface in interfaces {
            if !interface.endpoints.is_null() {
                let _ = unsafe {
                    Vec::from_raw_parts(
                        interface.endpoints,
                        interface.endpoint_count,
                        interface.endpoint_count,
                    )
                };
            }
        }
        device.interfaces = ptr::null_mut();
        device.interface_count = 0;
    }

    if !device.native_device.is_null() {
        let _ = unsafe { Box::from_raw(device.native_device.cast::<NativeUsbDeviceInfo>()) };
        device.native_device = ptr::null_mut();
    }
}

fn build_usb_device(native: NativeUsbDeviceInfo) -> usb_device {
    let vendor_id = native.vendor_id;
    let product_id = native.product_id;
    let manufacturer_string_index = native.manufacturer_string_index;
    let product_string_index = native.product_string_index;
    let bus_number = native.bus_number;
    let device_address = native.device_address;
    let configuration_value = native.configuration_value;

    let interfaces = native
        .interfaces
        .iter()
        .map(|interface| {
            let endpoints = interface
                .endpoints
                .iter()
                .map(|endpoint| usb_endpoint_descriptor {
                    address: endpoint.address,
                    attributes: endpoint.attributes,
                    max_packet_size: endpoint.max_packet_size,
                })
                .collect::<Vec<_>>();
            let endpoint_count = endpoints.len();
            let endpoints_ptr = if endpoint_count == 0 {
                ptr::null_mut()
            } else {
                Box::into_raw(endpoints.into_boxed_slice()) as *mut usb_endpoint_descriptor
            };
            usb_interface_descriptor {
                number: interface.number,
                alternate_setting: interface.alternate_setting,
                endpoint_count,
                endpoints: endpoints_ptr,
            }
        })
        .collect::<Vec<_>>();

    let interface_count = interfaces.len();
    let interfaces_ptr = if interface_count == 0 {
        ptr::null_mut()
    } else {
        Box::into_raw(interfaces.into_boxed_slice()) as *mut usb_interface_descriptor
    };

    usb_device {
        native_device: Box::into_raw(Box::new(native)).cast(),
        vendor_id,
        product_id,
        manufacturer_string_index,
        product_string_index,
        bus_number,
        device_address,
        configuration_value,
        interface_count,
        interfaces: interfaces_ptr,
    }
}

fn bulk_endpoints_from_c(device: &usb_device) -> Option<usb_bulk_endpoints> {
    if device.interfaces.is_null() {
        return None;
    }

    let interfaces = unsafe { slice::from_raw_parts(device.interfaces, device.interface_count) };
    for interface in interfaces {
        let mut result = usb_bulk_endpoints {
            interface_number: interface.number,
            alternate_setting: interface.alternate_setting as c_int,
            ..usb_bulk_endpoints::default()
        };
        let mut found_in = false;
        let mut found_out = false;

        if interface.endpoints.is_null() {
            continue;
        }
        let endpoints =
            unsafe { slice::from_raw_parts(interface.endpoints, interface.endpoint_count) };
        for endpoint in endpoints {
            if endpoint.attributes & USB_ENDPOINT_TYPE_MASK != USB_ENDPOINT_TYPE_BULK {
                continue;
            }
            if endpoint.address & USB_ENDPOINT_DIR_MASK == USB_ENDPOINT_IN {
                result.endpoint_in = endpoint.address;
                result.max_packet_size = result.max_packet_size.max(endpoint.max_packet_size);
                found_in = true;
            } else if endpoint.address & USB_ENDPOINT_DIR_MASK == USB_ENDPOINT_OUT {
                result.endpoint_out = endpoint.address;
                result.max_packet_size = result.max_packet_size.max(endpoint.max_packet_size);
                found_out = true;
            }
        }

        if found_in && found_out {
            return Some(result);
        }
    }

    None
}

pub unsafe fn usb_prepare() -> c_int {
    match native_prepare() {
        Ok(()) => USB_SUCCESS,
        Err(error) => map_usb_error(error),
    }
}

pub unsafe fn usb_get_device_list(list: *mut usb_device_list) -> c_int {
    let Some(list_ref) = (unsafe { as_mut(list) }) else {
        return USB_ERROR_INVALID_PARAM;
    };

    list_ref.devices = ptr::null_mut();
    list_ref.count = 0;

    let devices = match native_list_devices() {
        Ok(devices) => devices,
        Err(error) => return map_usb_error(error),
    };

    let devices = devices
        .into_iter()
        .map(build_usb_device)
        .collect::<Vec<_>>();
    list_ref.count = devices.len();
    if !devices.is_empty() {
        list_ref.devices = Box::into_raw(devices.into_boxed_slice()) as *mut usb_device;
    }

    USB_SUCCESS
}

pub unsafe fn usb_free_device_list(list: *mut usb_device_list) {
    let Some(list_ref) = (unsafe { as_mut(list) }) else {
        return;
    };

    if !list_ref.devices.is_null() {
        let mut devices =
            unsafe { Vec::from_raw_parts(list_ref.devices, list_ref.count, list_ref.count) };
        for device in &mut devices {
            unsafe { free_usb_device(device) };
        }
    }

    list_ref.devices = ptr::null_mut();
    list_ref.count = 0;
}

pub unsafe fn usb_get_bus_device_strings(
    device: *const usb_device,
    bus_buffer: *mut c_char,
    bus_buffer_size: size_t,
    device_buffer: *mut c_char,
    device_buffer_size: size_t,
) -> c_int {
    let Some(device_ref) = (unsafe { as_ref(device) }) else {
        return USB_ERROR_INVALID_PARAM;
    };
    if bus_buffer.is_null()
        || device_buffer.is_null()
        || bus_buffer_size == 0
        || device_buffer_size == 0
    {
        return USB_ERROR_INVALID_PARAM;
    }

    let (bus, address) = match unsafe { native_device(device) } {
        Ok(Some(native)) => native_bus_device_strings(native),
        Ok(None) => (
            format!("{:03}", device_ref.bus_number),
            format!("{:03}", device_ref.device_address),
        ),
        Err(error) => return error,
    };

    if bus.len() >= bus_buffer_size || address.len() >= device_buffer_size {
        return USB_ERROR_OVERFLOW;
    }

    unsafe {
        copy_bytes_with_truncation(bus_buffer, bus_buffer_size, bus.as_bytes());
        copy_bytes_with_truncation(device_buffer, device_buffer_size, address.as_bytes());
    }
    USB_SUCCESS
}

pub unsafe fn usb_device_get_bulk_endpoints(
    device: *const usb_device,
    endpoints: *mut usb_bulk_endpoints,
) -> bool {
    let Some(device_ref) = (unsafe { as_ref(device) }) else {
        return false;
    };
    let Some(endpoints_ref) = (unsafe { as_mut(endpoints) }) else {
        return false;
    };

    *endpoints_ref = usb_bulk_endpoints::default();

    if let Ok(Some(native)) = unsafe { native_device(device) }
        && let Some(found) = native_bulk_endpoints(native)
    {
        *endpoints_ref = map_bulk_endpoints(found);
        return true;
    }

    if let Some(found) = bulk_endpoints_from_c(device_ref) {
        *endpoints_ref = found;
        return true;
    }

    false
}

pub unsafe fn usb_open(device: *const usb_device, handle: *mut *mut usb_dev_handle) -> c_int {
    if handle.is_null() {
        return USB_ERROR_INVALID_PARAM;
    }
    unsafe {
        *handle = ptr::null_mut();
    }

    let native_handle = match unsafe { native_device(device) } {
        Ok(Some(device)) => NativeUsbHandle::open(device),
        Ok(None) => {
            let Some(device_ref) = (unsafe { as_ref(device) }) else {
                return USB_ERROR_INVALID_PARAM;
            };
            NativeUsbHandle::open_by_selector(UsbDeviceSelector {
                vendor_id: device_ref.vendor_id,
                product_id: device_ref.product_id,
                bus_number: device_ref.bus_number,
                device_address: device_ref.device_address,
            })
        }
        Err(error) => return error,
    };

    match native_handle {
        Ok(handle_state) => {
            unsafe {
                *handle = Box::into_raw(Box::new(UsbHandleState {
                    handle: handle_state,
                }))
                .cast::<usb_dev_handle>();
            }
            USB_SUCCESS
        }
        Err(error) => map_usb_error(error),
    }
}

pub unsafe fn usb_close(handle: *mut usb_dev_handle) -> c_int {
    if handle.is_null() {
        return USB_SUCCESS;
    }
    let _ = unsafe { Box::from_raw(handle.cast::<UsbHandleState>()) };
    USB_SUCCESS
}

pub unsafe fn usb_set_configuration(
    handle: *mut usb_dev_handle,
    configuration_value: c_int,
) -> c_int {
    let state = match unsafe { handle_state(handle) } {
        Ok(state) => state,
        Err(error) => return error,
    };
    if !(0..=u8::MAX as c_int).contains(&configuration_value) {
        return USB_ERROR_INVALID_PARAM;
    }
    match state.handle.set_configuration(configuration_value as u8) {
        Ok(()) => USB_SUCCESS,
        Err(error) => map_usb_error(error),
    }
}

pub unsafe fn usb_claim_interface(handle: *mut usb_dev_handle, interface_number: c_int) -> c_int {
    let state = match unsafe { handle_state(handle) } {
        Ok(state) => state,
        Err(error) => return error,
    };
    if !(0..=u8::MAX as c_int).contains(&interface_number) {
        return USB_ERROR_INVALID_PARAM;
    }
    match state.handle.claim_interface(interface_number as u8) {
        Ok(()) => USB_SUCCESS,
        Err(error) => map_usb_error(error),
    }
}

pub unsafe fn usb_release_interface(handle: *mut usb_dev_handle, interface_number: c_int) -> c_int {
    let state = match unsafe { handle_state(handle) } {
        Ok(state) => state,
        Err(error) => return error,
    };
    if !(0..=u8::MAX as c_int).contains(&interface_number) {
        return USB_ERROR_INVALID_PARAM;
    }
    match state.handle.release_interface(interface_number as u8) {
        Ok(()) => USB_SUCCESS,
        Err(error) => map_usb_error(error),
    }
}

pub unsafe fn usb_set_altinterface(
    handle: *mut usb_dev_handle,
    interface_number: c_int,
    alternate_setting: c_int,
) -> c_int {
    let state = match unsafe { handle_state(handle) } {
        Ok(state) => state,
        Err(error) => return error,
    };
    if !(0..=u8::MAX as c_int).contains(&interface_number)
        || !(0..=u8::MAX as c_int).contains(&alternate_setting)
    {
        return USB_ERROR_INVALID_PARAM;
    }

    match state
        .handle
        .set_altinterface(interface_number as u8, alternate_setting as u8)
    {
        Ok(()) => USB_SUCCESS,
        Err(error) => map_usb_error(error),
    }
}

pub unsafe fn usb_reset(handle: *mut usb_dev_handle) -> c_int {
    let state = match unsafe { handle_state(handle) } {
        Ok(state) => state,
        Err(error) => return error,
    };
    match state.handle.reset() {
        Ok(()) => USB_SUCCESS,
        Err(error) => map_usb_error(error),
    }
}

pub unsafe fn usb_bulk_read(
    handle: *mut usb_dev_handle,
    endpoint: u8,
    data: *mut u8,
    size: size_t,
    timeout: c_int,
) -> c_int {
    let state = match unsafe { handle_state(handle) } {
        Ok(state) => state,
        Err(error) => return error,
    };
    if size != 0 && data.is_null() {
        return USB_ERROR_INVALID_PARAM;
    }
    let out = if size == 0 {
        &mut [][..]
    } else {
        unsafe { slice::from_raw_parts_mut(data, size) }
    };
    match state.handle.bulk_read(endpoint, out, timeout) {
        Ok(read_len) => read_len as c_int,
        Err(error) => map_usb_error(error),
    }
}

pub unsafe fn usb_bulk_write(
    handle: *mut usb_dev_handle,
    endpoint: u8,
    data: *const u8,
    size: size_t,
    timeout: c_int,
) -> c_int {
    let state = match unsafe { handle_state(handle) } {
        Ok(state) => state,
        Err(error) => return error,
    };
    if size != 0 && data.is_null() {
        return USB_ERROR_INVALID_PARAM;
    }
    let bytes = if size == 0 {
        &[][..]
    } else {
        unsafe { slice::from_raw_parts(data, size) }
    };
    match state.handle.bulk_write(endpoint, bytes, timeout) {
        Ok(written) => written as c_int,
        Err(error) => map_usb_error(error),
    }
}

pub unsafe fn usb_get_string_simple(
    handle: *mut usb_dev_handle,
    string_index: c_int,
    buffer: *mut c_char,
    buffer_size: size_t,
) -> c_int {
    if buffer.is_null() || buffer_size == 0 {
        return USB_ERROR_INVALID_PARAM;
    }

    unsafe {
        *buffer = 0;
    }

    if handle.is_null() || string_index <= 0 {
        return 0;
    }

    if string_index > u8::MAX as c_int {
        return USB_ERROR_INVALID_PARAM;
    }

    let state = match unsafe { handle_state(handle) } {
        Ok(state) => state,
        Err(error) => return error,
    };
    match state.handle.get_string_simple(string_index as u8) {
        Ok(value) => {
            let bytes = if value.is_empty() {
                INVALID_STRING_DESCRIPTOR_FALLBACK
            } else {
                value.as_bytes()
            };
            let copy_len = bytes.len().min(buffer_size.saturating_sub(1));
            unsafe { copy_bytes_with_truncation(buffer, buffer_size, bytes) };
            copy_len as c_int
        }
        Err(error) => map_usb_error(error),
    }
}

pub unsafe fn usb_strerror(result: c_int) -> *const c_char {
    match result {
        x if x >= 0 => c"success".as_ptr(),
        USB_ERROR_IO => c"input/output error".as_ptr(),
        USB_ERROR_INVALID_PARAM => c"invalid parameter".as_ptr(),
        USB_ERROR_ACCESS => c"access denied".as_ptr(),
        USB_ERROR_NO_DEVICE => c"no such device".as_ptr(),
        USB_ERROR_NOT_FOUND => c"entity not found".as_ptr(),
        USB_ERROR_BUSY => c"resource busy".as_ptr(),
        USB_ERROR_TIMEOUT => c"operation timed out".as_ptr(),
        USB_ERROR_OVERFLOW => c"overflow".as_ptr(),
        USB_ERROR_PIPE => c"pipe error".as_ptr(),
        USB_ERROR_INTERRUPTED => c"system call interrupted".as_ptr(),
        USB_ERROR_NO_MEM => c"out of memory".as_ptr(),
        USB_ERROR_NOT_SUPPORTED => c"operation not supported".as_ptr(),
        _ => c"other error".as_ptr(),
    }
}

pub unsafe fn usb_error_is_timeout(result: c_int) -> bool {
    result == USB_ERROR_TIMEOUT
}

pub unsafe fn usb_error_is_access(result: c_int) -> bool {
    result == USB_ERROR_ACCESS
}
