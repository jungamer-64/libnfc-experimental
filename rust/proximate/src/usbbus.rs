// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// USB helper layer backed by `nusb`.

#![allow(non_camel_case_types)]

use crate::ffi_support::{as_mut, as_ref, copy_bytes_with_truncation};
use libc::{c_char, c_int, c_void, size_t};
use nusb::descriptors::{ConfigurationDescriptor, TransferType, language_id};
use nusb::transfer::{Buffer, Bulk, In, Out, TransferError};
use nusb::{
    Device, DeviceInfo, Error as NusbError, ErrorKind as NusbErrorKind, Interface, MaybeFuture,
};
use std::collections::HashMap;
use std::num::NonZeroU8;
use std::ptr;
use std::slice;
use std::time::Duration;

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
const STRING_DESCRIPTOR_TIMEOUT: Duration = Duration::from_millis(250);
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

#[derive(Clone, Debug)]
struct UsbDeviceKey {
    vendor_id: u16,
    product_id: u16,
    bus_number: u8,
    device_address: u8,
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    bus_id: String,
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    port_chain: Vec<u8>,
}

#[derive(Clone, Debug)]
struct UsbNativeDevice {
    key: UsbDeviceKey,
    manufacturer_index: u8,
    product_index: u8,
    manufacturer_string: Option<String>,
    product_string: Option<String>,
}

struct UsbHandleState {
    key: UsbDeviceKey,
    device: Device,
    claimed_interfaces: HashMap<u8, Interface>,
    read_overflow: HashMap<u8, Vec<u8>>,
    string_descriptors: HashMap<u8, String>,
}

impl UsbDeviceKey {
    fn from_device_info(info: &DeviceInfo) -> Self {
        Self {
            vendor_id: info.vendor_id(),
            product_id: info.product_id(),
            bus_number: device_bus_number(info),
            device_address: info.device_address(),
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            bus_id: info.bus_id().to_owned(),
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            port_chain: info.port_chain().to_vec(),
        }
    }

    fn matches(&self, info: &DeviceInfo) -> bool {
        if info.vendor_id() != self.vendor_id || info.product_id() != self.product_id {
            return false;
        }

        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        {
            if info.bus_id() != self.bus_id {
                return false;
            }

            if !self.port_chain.is_empty() {
                return info.port_chain() == self.port_chain.as_slice();
            }
        }

        info.device_address() == self.device_address && device_bus_number(info) == self.bus_number
    }
}

fn device_bus_number(info: &DeviceInfo) -> u8 {
    #[cfg(target_os = "linux")]
    {
        return info.busnum();
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        return info.bus_id().parse::<u8>().unwrap_or(0);
    }
}

fn duration_from_timeout(timeout: c_int) -> Duration {
    if timeout <= 0 {
        Duration::MAX
    } else {
        Duration::from_millis(timeout as u64)
    }
}

fn round_up_transfer_len(size: usize, packet_size: usize) -> usize {
    if size == 0 {
        0
    } else if packet_size == 0 {
        size
    } else {
        size.div_ceil(packet_size) * packet_size
    }
}

fn map_nusb_error(error: &NusbError) -> c_int {
    match error.kind() {
        NusbErrorKind::Disconnected => USB_ERROR_NO_DEVICE,
        NusbErrorKind::Busy => USB_ERROR_BUSY,
        NusbErrorKind::PermissionDenied => USB_ERROR_ACCESS,
        NusbErrorKind::NotFound => USB_ERROR_NOT_FOUND,
        NusbErrorKind::Unsupported => USB_ERROR_NOT_SUPPORTED,
        NusbErrorKind::Other => USB_ERROR_OTHER,
        _ => USB_ERROR_OTHER,
    }
}

fn map_transfer_error(error: TransferError) -> c_int {
    match error {
        TransferError::Cancelled => USB_ERROR_TIMEOUT,
        TransferError::Stall => USB_ERROR_PIPE,
        TransferError::Disconnected => USB_ERROR_NO_DEVICE,
        TransferError::Fault => USB_ERROR_IO,
        TransferError::InvalidArgument => USB_ERROR_INVALID_PARAM,
        TransferError::Unknown(_) => USB_ERROR_OTHER,
    }
}

fn result_string(code: c_int) -> *const c_char {
    match code {
        x if x >= 0 => b"success\0".as_ptr().cast(),
        USB_ERROR_IO => b"input/output error\0".as_ptr().cast(),
        USB_ERROR_INVALID_PARAM => b"invalid parameter\0".as_ptr().cast(),
        USB_ERROR_ACCESS => b"access denied\0".as_ptr().cast(),
        USB_ERROR_NO_DEVICE => b"no such device\0".as_ptr().cast(),
        USB_ERROR_NOT_FOUND => b"entity not found\0".as_ptr().cast(),
        USB_ERROR_BUSY => b"resource busy\0".as_ptr().cast(),
        USB_ERROR_TIMEOUT => b"operation timed out\0".as_ptr().cast(),
        USB_ERROR_OVERFLOW => b"overflow\0".as_ptr().cast(),
        USB_ERROR_PIPE => b"pipe error\0".as_ptr().cast(),
        USB_ERROR_INTERRUPTED => b"system call interrupted\0".as_ptr().cast(),
        USB_ERROR_NO_MEM => b"out of memory\0".as_ptr().cast(),
        USB_ERROR_NOT_SUPPORTED => b"operation not supported\0".as_ptr().cast(),
        _ => b"other error\0".as_ptr().cast(),
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
) -> Result<Option<&'a UsbNativeDevice>, c_int> {
    let Some(device_ref) = (unsafe { as_ref(device) }) else {
        return Err(USB_ERROR_INVALID_PARAM);
    };

    if device_ref.native_device.is_null() {
        return Ok(None);
    }

    Ok(unsafe { as_ref(device_ref.native_device.cast::<UsbNativeDevice>()) })
}

fn populate_string_cache(state: &mut UsbHandleState, native: &UsbNativeDevice) {
    if native.manufacturer_index != 0 {
        if let Some(value) = native.manufacturer_string.clone() {
            state
                .string_descriptors
                .insert(native.manufacturer_index, value);
        }
    }

    if native.product_index != 0 {
        if let Some(value) = native.product_string.clone() {
            state.string_descriptors.insert(native.product_index, value);
        }
    }
}

fn open_matching_device(key: &UsbDeviceKey) -> Result<(DeviceInfo, Device), c_int> {
    let mut devices = nusb::list_devices()
        .wait()
        .map_err(|error| map_nusb_error(&error))?;
    let Some(info) = devices.find(|info| key.matches(info)) else {
        return Err(USB_ERROR_NO_DEVICE);
    };

    let device = info.open().wait().map_err(|error| map_nusb_error(&error))?;
    Ok((info, device))
}

fn collect_interfaces(
    config: ConfigurationDescriptor<'_>,
) -> Result<Vec<usb_interface_descriptor>, c_int> {
    let mut interfaces = Vec::new();
    for group in config.interfaces() {
        let alt = group.first_alt_setting();
        let endpoints: Vec<usb_endpoint_descriptor> = alt
            .endpoints()
            .map(|endpoint| usb_endpoint_descriptor {
                address: endpoint.address(),
                attributes: endpoint.attributes(),
                max_packet_size: endpoint.max_packet_size() as u16,
            })
            .collect();
        let endpoint_count = endpoints.len();
        let endpoints_ptr = if endpoint_count == 0 {
            ptr::null_mut()
        } else {
            Box::into_raw(endpoints.into_boxed_slice()) as *mut usb_endpoint_descriptor
        };

        interfaces.push(usb_interface_descriptor {
            number: alt.interface_number(),
            alternate_setting: alt.alternate_setting(),
            endpoint_count,
            endpoints: endpoints_ptr,
        });
    }

    Ok(interfaces)
}

fn build_usb_device(info: &DeviceInfo) -> usb_device {
    let mut native = UsbNativeDevice {
        key: UsbDeviceKey::from_device_info(info),
        manufacturer_index: 0,
        product_index: 0,
        manufacturer_string: info.manufacturer_string().map(str::to_owned),
        product_string: info.product_string().map(str::to_owned),
    };

    let mut device = usb_device {
        native_device: ptr::null_mut(),
        vendor_id: info.vendor_id(),
        product_id: info.product_id(),
        manufacturer_string_index: 0,
        product_string_index: 0,
        bus_number: device_bus_number(info),
        device_address: info.device_address(),
        configuration_value: 1,
        interface_count: 0,
        interfaces: ptr::null_mut(),
    };

    if let Ok(opened) = info.open().wait() {
        let descriptor = opened.device_descriptor();
        device.manufacturer_string_index = descriptor
            .manufacturer_string_index()
            .map(NonZeroU8::get)
            .unwrap_or(0);
        device.product_string_index = descriptor
            .product_string_index()
            .map(NonZeroU8::get)
            .unwrap_or(0);
        native.manufacturer_index = device.manufacturer_string_index;
        native.product_index = device.product_string_index;

        if let Some(config) = opened.configurations().next() {
            device.configuration_value = config.configuration_value();
            if let Ok(interfaces) = collect_interfaces(config) {
                device.interface_count = interfaces.len();
                if !interfaces.is_empty() {
                    device.interfaces = Box::into_raw(interfaces.into_boxed_slice())
                        as *mut usb_interface_descriptor;
                }
            }
        }
    }

    device.native_device = Box::into_raw(Box::new(native)).cast();
    device
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
        let _ = unsafe { Box::from_raw(device.native_device.cast::<UsbNativeDevice>()) };
        device.native_device = ptr::null_mut();
    }
}

fn clear_interface_state(state: &mut UsbHandleState) {
    state.claimed_interfaces.clear();
    state.read_overflow.clear();
}

fn find_bulk_interface(state: &UsbHandleState, endpoint: u8) -> Result<&Interface, c_int> {
    state
        .claimed_interfaces
        .values()
        .find(|interface| {
            interface
                .descriptor()
                .map(|descriptor| {
                    descriptor.endpoints().any(|candidate| {
                        candidate.address() == endpoint
                            && candidate.transfer_type() == TransferType::Bulk
                    })
                })
                .unwrap_or(false)
        })
        .ok_or(USB_ERROR_NOT_FOUND)
}

fn copy_from_overflow(state: &mut UsbHandleState, endpoint: u8, out: &mut [u8]) -> usize {
    if out.is_empty() {
        return 0;
    }

    let mut copied = 0;
    let mut remove_entry = false;
    if let Some(overflow) = state.read_overflow.get_mut(&endpoint) {
        copied = overflow.len().min(out.len());
        out[..copied].copy_from_slice(&overflow[..copied]);
        overflow.drain(..copied);
        remove_entry = overflow.is_empty();
    }

    if remove_entry {
        state.read_overflow.remove(&endpoint);
    }

    copied
}

pub unsafe fn usb_prepare() -> c_int {
    USB_SUCCESS
}

pub unsafe fn usb_get_device_list(list: *mut usb_device_list) -> c_int {
    let Some(list_ref) = (unsafe { as_mut(list) }) else {
        return USB_ERROR_INVALID_PARAM;
    };

    list_ref.devices = ptr::null_mut();
    list_ref.count = 0;

    let devices = match nusb::list_devices().wait() {
        Ok(devices) => devices.collect::<Vec<_>>(),
        Err(error) => return map_nusb_error(&error),
    };

    let usb_devices: Vec<usb_device> = devices.iter().map(build_usb_device).collect();
    list_ref.count = usb_devices.len();
    if !usb_devices.is_empty() {
        list_ref.devices = Box::into_raw(usb_devices.into_boxed_slice()) as *mut usb_device;
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

    let bus = format!("{:03}", device_ref.bus_number);
    let address = format!("{:03}", device_ref.device_address);

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

    if device_ref.interfaces.is_null() {
        return false;
    }

    let interfaces =
        unsafe { slice::from_raw_parts(device_ref.interfaces, device_ref.interface_count) };
    for interface in interfaces {
        let mut found_in = false;
        let mut found_out = false;
        endpoints_ref.interface_number = interface.number;
        endpoints_ref.alternate_setting = interface.alternate_setting as c_int;

        if interface.endpoints.is_null() {
            continue;
        }

        let endpoint_slice =
            unsafe { slice::from_raw_parts(interface.endpoints, interface.endpoint_count) };
        for endpoint in endpoint_slice {
            if (endpoint.attributes & USB_ENDPOINT_TYPE_MASK) != USB_ENDPOINT_TYPE_BULK {
                continue;
            }

            if (endpoint.address & USB_ENDPOINT_DIR_MASK) == USB_ENDPOINT_IN {
                endpoints_ref.endpoint_in = endpoint.address;
                endpoints_ref.max_packet_size =
                    endpoints_ref.max_packet_size.max(endpoint.max_packet_size);
                found_in = true;
            } else if (endpoint.address & USB_ENDPOINT_DIR_MASK) == USB_ENDPOINT_OUT {
                endpoints_ref.endpoint_out = endpoint.address;
                endpoints_ref.max_packet_size =
                    endpoints_ref.max_packet_size.max(endpoint.max_packet_size);
                found_out = true;
            }
        }

        if found_in && found_out {
            return true;
        }
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

    let Some(device_ref) = (unsafe { as_ref(device) }) else {
        return USB_ERROR_INVALID_PARAM;
    };

    let native = match unsafe { native_device(device) } {
        Ok(Some(native)) => native.clone(),
        Ok(None) => UsbNativeDevice {
            key: UsbDeviceKey {
                vendor_id: device_ref.vendor_id,
                product_id: device_ref.product_id,
                bus_number: device_ref.bus_number,
                device_address: device_ref.device_address,
                #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
                bus_id: format!("{}", device_ref.bus_number),
                #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
                port_chain: Vec::new(),
            },
            manufacturer_index: device_ref.manufacturer_string_index,
            product_index: device_ref.product_string_index,
            manufacturer_string: None,
            product_string: None,
        },
        Err(error) => return error,
    };

    let (info, opened) = match open_matching_device(&native.key) {
        Ok(result) => result,
        Err(error) => return error,
    };

    let mut state = UsbHandleState {
        key: UsbDeviceKey::from_device_info(&info),
        device: opened,
        claimed_interfaces: HashMap::new(),
        read_overflow: HashMap::new(),
        string_descriptors: HashMap::new(),
    };
    populate_string_cache(&mut state, &native);

    let raw = Box::into_raw(Box::new(state)).cast::<usb_dev_handle>();
    unsafe {
        *handle = raw;
    }
    USB_SUCCESS
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

    match state
        .device
        .set_configuration(configuration_value as u8)
        .wait()
    {
        Ok(()) => USB_SUCCESS,
        Err(error) => map_nusb_error(&error),
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
    let interface_number = interface_number as u8;

    if state.claimed_interfaces.contains_key(&interface_number) {
        return USB_SUCCESS;
    }

    match state.device.claim_interface(interface_number).wait() {
        Ok(interface) => {
            state.claimed_interfaces.insert(interface_number, interface);
            USB_SUCCESS
        }
        Err(error) => map_nusb_error(&error),
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

    if state
        .claimed_interfaces
        .remove(&(interface_number as u8))
        .is_some()
    {
        state.read_overflow.clear();
        USB_SUCCESS
    } else {
        USB_ERROR_NOT_FOUND
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

    let Some(interface) = state.claimed_interfaces.get(&(interface_number as u8)) else {
        return USB_ERROR_NOT_FOUND;
    };

    match interface.set_alt_setting(alternate_setting as u8).wait() {
        Ok(()) => {
            state.read_overflow.clear();
            USB_SUCCESS
        }
        Err(error) => map_nusb_error(&error),
    }
}

pub unsafe fn usb_reset(handle: *mut usb_dev_handle) -> c_int {
    let state = match unsafe { handle_state(handle) } {
        Ok(state) => state,
        Err(error) => return error,
    };

    if let Err(error) = state.device.reset().wait() {
        return map_nusb_error(&error);
    }

    let (_, reopened) = match open_matching_device(&state.key) {
        Ok(result) => result,
        Err(error) => return error,
    };

    state.device = reopened;
    clear_interface_state(state);
    USB_SUCCESS
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
    if (endpoint & USB_ENDPOINT_DIR_MASK) != USB_ENDPOINT_IN {
        return USB_ERROR_INVALID_PARAM;
    }

    if size == 0 {
        return 0;
    }

    let out = unsafe { slice::from_raw_parts_mut(data, size) };
    let mut copied = copy_from_overflow(state, endpoint, out);
    if copied == size {
        return copied as c_int;
    }

    let interface = match find_bulk_interface(state, endpoint) {
        Ok(interface) => interface,
        Err(error) => return error,
    };

    let mut bulk_in = match interface.endpoint::<Bulk, In>(endpoint) {
        Ok(endpoint) => endpoint,
        Err(error) => return map_nusb_error(&error),
    };

    let request_len = round_up_transfer_len(size - copied, bulk_in.max_packet_size());
    let completion =
        bulk_in.transfer_blocking(Buffer::new(request_len), duration_from_timeout(timeout));
    let actual_len = completion.actual_len;
    let buffer = match completion.into_result() {
        Ok(buffer) => buffer.into_vec(),
        Err(error) => return map_transfer_error(error),
    };

    let transfer_len = actual_len.min(buffer.len());
    let copy_len = transfer_len.min(size - copied);
    out[copied..copied + copy_len].copy_from_slice(&buffer[..copy_len]);
    copied += copy_len;

    if transfer_len > copy_len {
        state
            .read_overflow
            .entry(endpoint)
            .or_default()
            .extend_from_slice(&buffer[copy_len..transfer_len]);
    }

    copied as c_int
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
    if (endpoint & USB_ENDPOINT_DIR_MASK) != USB_ENDPOINT_OUT {
        return USB_ERROR_INVALID_PARAM;
    }

    let interface = match find_bulk_interface(state, endpoint) {
        Ok(interface) => interface,
        Err(error) => return error,
    };

    let mut bulk_out = match interface.endpoint::<Bulk, Out>(endpoint) {
        Ok(endpoint) => endpoint,
        Err(error) => return map_nusb_error(&error),
    };

    let buffer = if size == 0 {
        Buffer::new(0)
    } else {
        unsafe { slice::from_raw_parts(data, size) }.to_vec().into()
    };

    let completion = bulk_out.transfer_blocking(buffer, duration_from_timeout(timeout));
    let actual_len = completion.actual_len;
    match completion.into_result() {
        Ok(_) => actual_len as c_int,
        Err(error) => map_transfer_error(error),
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

    let state = match unsafe { handle_state(handle) } {
        Ok(state) => state,
        Err(error) => return error,
    };
    if string_index > u8::MAX as c_int {
        return USB_ERROR_INVALID_PARAM;
    }

    let string_index = string_index as u8;
    if let Some(value) = state.string_descriptors.get(&string_index) {
        let copy_len = value.len().min(buffer_size.saturating_sub(1));
        unsafe { copy_bytes_with_truncation(buffer, buffer_size, value.as_bytes()) };
        return copy_len as c_int;
    }

    let Some(string_index_nonzero) = NonZeroU8::new(string_index) else {
        return 0;
    };

    let value = match state
        .device
        .get_string_descriptor(
            string_index_nonzero,
            language_id::US_ENGLISH,
            STRING_DESCRIPTOR_TIMEOUT,
        )
        .wait()
    {
        Ok(value) => value,
        Err(error) => {
            return match error {
                nusb::GetDescriptorError::Transfer(error) => map_transfer_error(error),
                nusb::GetDescriptorError::InvalidDescriptor => {
                    unsafe {
                        copy_bytes_with_truncation(
                            buffer,
                            buffer_size,
                            INVALID_STRING_DESCRIPTOR_FALLBACK,
                        )
                    };
                    INVALID_STRING_DESCRIPTOR_FALLBACK.len() as c_int
                }
            };
        }
    };

    let copy_len = value.len().min(buffer_size.saturating_sub(1));
    unsafe { copy_bytes_with_truncation(buffer, buffer_size, value.as_bytes()) };
    state.string_descriptors.insert(string_index, value);
    copy_len as c_int
}

pub unsafe fn usb_strerror(result: c_int) -> *const c_char {
    result_string(result)
}

pub unsafe fn usb_error_is_timeout(result: c_int) -> bool {
    result == USB_ERROR_TIMEOUT
}

pub unsafe fn usb_error_is_access(result: c_int) -> bool {
    result == USB_ERROR_ACCESS
}
