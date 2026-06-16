use nusb::descriptors::{ConfigurationDescriptor, TransferType, language_id};
use nusb::transfer::{Buffer, Bulk, In, Out, TransferError};
use nusb::{
    Device, DeviceInfo as NusbDeviceInfo, Error as NusbError, ErrorKind as NusbErrorKind,
    Interface, MaybeFuture,
};
use std::collections::HashMap;
use std::num::NonZeroU8;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct UsbEndpointInfo {
    pub address: u8,
    pub attributes: u8,
    pub max_packet_size: u16,
}

#[derive(Clone, Debug)]
pub struct UsbInterfaceInfo {
    pub number: u8,
    pub alternate_setting: u8,
    pub endpoints: Vec<UsbEndpointInfo>,
}

#[derive(Clone, Debug)]
pub struct UsbDeviceInfo {
    key: UsbDeviceKey,
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer_string_index: u8,
    pub product_string_index: u8,
    pub bus_number: u8,
    pub device_address: u8,
    pub configuration_value: u8,
    pub interfaces: Vec<UsbInterfaceInfo>,
    pub manufacturer_string: Option<String>,
    pub product_string: Option<String>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct UsbBulkEndpoints {
    pub interface_number: u8,
    pub alternate_setting: i32,
    pub endpoint_in: u8,
    pub endpoint_out: u8,
    pub max_packet_size: u16,
}

#[derive(Clone, Debug)]
pub struct UsbDeviceSelector {
    pub vendor_id: u16,
    pub product_id: u16,
    pub bus_number: u8,
    pub device_address: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsbError {
    Io,
    InvalidParam,
    Access,
    NoDevice,
    NotFound,
    Busy,
    Timeout,
    Overflow,
    Pipe,
    Interrupted,
    NoMem,
    NotSupported,
    Other,
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

pub struct UsbHandle {
    key: UsbDeviceKey,
    device: Device,
    claimed_interfaces: HashMap<u8, Interface>,
    read_overflow: HashMap<u8, Vec<u8>>,
    string_descriptors: HashMap<u8, String>,
}

impl UsbDeviceKey {
    fn from_device_info(info: &NusbDeviceInfo) -> Self {
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

    fn matches(&self, info: &NusbDeviceInfo) -> bool {
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

fn device_bus_number(info: &NusbDeviceInfo) -> u8 {
    #[cfg(target_os = "linux")]
    {
        info.busnum()
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        info.bus_id().parse::<u8>().unwrap_or(0)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        0
    }
}

fn duration_from_timeout(timeout_ms: i32) -> Duration {
    if timeout_ms <= 0 {
        Duration::MAX
    } else {
        Duration::from_millis(timeout_ms as u64)
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

fn map_nusb_error(error: &NusbError) -> UsbError {
    match error.kind() {
        NusbErrorKind::Disconnected => UsbError::NoDevice,
        NusbErrorKind::Busy => UsbError::Busy,
        NusbErrorKind::PermissionDenied => UsbError::Access,
        NusbErrorKind::NotFound => UsbError::NotFound,
        NusbErrorKind::Unsupported => UsbError::NotSupported,
        NusbErrorKind::Other => UsbError::Other,
        _ => UsbError::Other,
    }
}

fn map_transfer_error(error: TransferError) -> UsbError {
    match error {
        TransferError::Cancelled => UsbError::Timeout,
        TransferError::Stall => UsbError::Pipe,
        TransferError::Disconnected => UsbError::NoDevice,
        TransferError::Fault => UsbError::Io,
        TransferError::InvalidArgument => UsbError::InvalidParam,
        TransferError::Unknown(_) => UsbError::Other,
    }
}

fn open_matching_device(key: &UsbDeviceKey) -> Result<(NusbDeviceInfo, Device), UsbError> {
    let mut devices = nusb::list_devices()
        .wait()
        .map_err(|error| map_nusb_error(&error))?;
    let Some(info) = devices.find(|info| key.matches(info)) else {
        return Err(UsbError::NoDevice);
    };
    let device = info.open().wait().map_err(|error| map_nusb_error(&error))?;
    Ok((info, device))
}

fn build_interface_info(config: ConfigurationDescriptor<'_>) -> Vec<UsbInterfaceInfo> {
    let mut interfaces = Vec::new();
    for group in config.interfaces() {
        let alt = group.first_alt_setting();
        let endpoints = alt
            .endpoints()
            .map(|endpoint| UsbEndpointInfo {
                address: endpoint.address(),
                attributes: endpoint.attributes(),
                max_packet_size: endpoint.max_packet_size() as u16,
            })
            .collect();
        interfaces.push(UsbInterfaceInfo {
            number: alt.interface_number(),
            alternate_setting: alt.alternate_setting(),
            endpoints,
        });
    }
    interfaces
}

fn build_device_info(info: &NusbDeviceInfo) -> UsbDeviceInfo {
    let mut device = UsbDeviceInfo {
        key: UsbDeviceKey::from_device_info(info),
        vendor_id: info.vendor_id(),
        product_id: info.product_id(),
        manufacturer_string_index: 0,
        product_string_index: 0,
        bus_number: device_bus_number(info),
        device_address: info.device_address(),
        configuration_value: 1,
        interfaces: Vec::new(),
        manufacturer_string: info.manufacturer_string().map(str::to_owned),
        product_string: info.product_string().map(str::to_owned),
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

        if let Some(config) = opened.configurations().next() {
            device.configuration_value = config.configuration_value();
            device.interfaces = build_interface_info(config);
        }
    }

    device
}

fn find_bulk_interface(handle: &UsbHandle, endpoint: u8) -> Result<&Interface, UsbError> {
    handle
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
        .ok_or(UsbError::NotFound)
}

fn copy_from_overflow(handle: &mut UsbHandle, endpoint: u8, out: &mut [u8]) -> usize {
    if out.is_empty() {
        return 0;
    }

    let mut copied = 0usize;
    let mut remove_entry = false;
    if let Some(overflow) = handle.read_overflow.get_mut(&endpoint) {
        copied = overflow.len().min(out.len());
        out[..copied].copy_from_slice(&overflow[..copied]);
        overflow.drain(..copied);
        remove_entry = overflow.is_empty();
    }

    if remove_entry {
        handle.read_overflow.remove(&endpoint);
    }

    copied
}

pub fn prepare() -> Result<(), UsbError> {
    Ok(())
}

pub fn list_devices() -> Result<Vec<UsbDeviceInfo>, UsbError> {
    let devices = nusb::list_devices()
        .wait()
        .map_err(|error| map_nusb_error(&error))?
        .collect::<Vec<_>>();
    Ok(devices.iter().map(build_device_info).collect())
}

pub fn bus_device_strings(device: &UsbDeviceInfo) -> (String, String) {
    (
        format!("{:03}", device.bus_number),
        format!("{:03}", device.device_address),
    )
}

pub fn bulk_endpoints(device: &UsbDeviceInfo) -> Option<UsbBulkEndpoints> {
    for interface in &device.interfaces {
        let mut result = UsbBulkEndpoints {
            interface_number: interface.number,
            alternate_setting: interface.alternate_setting as i32,
            ..UsbBulkEndpoints::default()
        };
        let mut found_in = false;
        let mut found_out = false;

        for endpoint in &interface.endpoints {
            if endpoint.attributes & 0x03 != 0x02 {
                continue;
            }
            if endpoint.address & 0x80 == 0x80 {
                result.endpoint_in = endpoint.address;
                result.max_packet_size = result.max_packet_size.max(endpoint.max_packet_size);
                found_in = true;
            } else {
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

impl UsbHandle {
    pub fn open(device: &UsbDeviceInfo) -> Result<Self, UsbError> {
        let (info, opened) = open_matching_device(&device.key)?;
        let mut string_descriptors = HashMap::new();
        if device.manufacturer_string_index != 0
            && let Some(value) = device.manufacturer_string.clone()
        {
            string_descriptors.insert(device.manufacturer_string_index, value);
        }
        if device.product_string_index != 0
            && let Some(value) = device.product_string.clone()
        {
            string_descriptors.insert(device.product_string_index, value);
        }

        Ok(Self {
            key: UsbDeviceKey::from_device_info(&info),
            device: opened,
            claimed_interfaces: HashMap::new(),
            read_overflow: HashMap::new(),
            string_descriptors,
        })
    }

    pub fn open_by_selector(selector: UsbDeviceSelector) -> Result<Self, UsbError> {
        let devices = list_devices()?;
        let Some(device) = devices.into_iter().find(|device| {
            device.vendor_id == selector.vendor_id
                && device.product_id == selector.product_id
                && device.bus_number == selector.bus_number
                && device.device_address == selector.device_address
        }) else {
            return Err(UsbError::NoDevice);
        };
        Self::open(&device)
    }

    pub fn set_configuration(&mut self, configuration_value: u8) -> Result<(), UsbError> {
        self.device
            .set_configuration(configuration_value)
            .wait()
            .map_err(|error| map_nusb_error(&error))
    }

    pub fn claim_interface(&mut self, interface_number: u8) -> Result<(), UsbError> {
        if self.claimed_interfaces.contains_key(&interface_number) {
            return Ok(());
        }
        let interface = self
            .device
            .claim_interface(interface_number)
            .wait()
            .map_err(|error| map_nusb_error(&error))?;
        self.claimed_interfaces.insert(interface_number, interface);
        Ok(())
    }

    pub fn release_interface(&mut self, interface_number: u8) -> Result<(), UsbError> {
        if self.claimed_interfaces.remove(&interface_number).is_some() {
            self.read_overflow.clear();
            Ok(())
        } else {
            Err(UsbError::NotFound)
        }
    }

    pub fn set_altinterface(
        &mut self,
        interface_number: u8,
        alternate_setting: u8,
    ) -> Result<(), UsbError> {
        let Some(interface) = self.claimed_interfaces.get(&interface_number) else {
            return Err(UsbError::NotFound);
        };
        interface
            .set_alt_setting(alternate_setting)
            .wait()
            .map_err(|error| map_nusb_error(&error))?;
        self.read_overflow.clear();
        Ok(())
    }

    pub fn reset(&mut self) -> Result<(), UsbError> {
        self.device
            .reset()
            .wait()
            .map_err(|error| map_nusb_error(&error))?;
        let (_, reopened) = open_matching_device(&self.key)?;
        self.device = reopened;
        self.claimed_interfaces.clear();
        self.read_overflow.clear();
        Ok(())
    }

    pub fn bulk_read(
        &mut self,
        endpoint: u8,
        out: &mut [u8],
        timeout: i32,
    ) -> Result<usize, UsbError> {
        if endpoint & 0x80 != 0x80 {
            return Err(UsbError::InvalidParam);
        }
        if out.is_empty() {
            return Ok(0);
        }

        let mut copied = copy_from_overflow(self, endpoint, out);
        if copied == out.len() {
            return Ok(copied);
        }

        let interface = find_bulk_interface(self, endpoint)?;
        let mut bulk_in = interface
            .endpoint::<Bulk, In>(endpoint)
            .map_err(|error| map_nusb_error(&error))?;
        let request_len = round_up_transfer_len(out.len() - copied, bulk_in.max_packet_size());
        let completion =
            bulk_in.transfer_blocking(Buffer::new(request_len), duration_from_timeout(timeout));
        let actual_len = completion.actual_len;
        let buffer = completion
            .into_result()
            .map(|buffer| buffer.into_vec())
            .map_err(map_transfer_error)?;

        let transfer_len = actual_len.min(buffer.len());
        let copy_len = transfer_len.min(out.len() - copied);
        out[copied..copied + copy_len].copy_from_slice(&buffer[..copy_len]);
        copied += copy_len;

        if transfer_len > copy_len {
            self.read_overflow
                .entry(endpoint)
                .or_default()
                .extend_from_slice(&buffer[copy_len..transfer_len]);
        }

        Ok(copied)
    }

    pub fn bulk_write(
        &mut self,
        endpoint: u8,
        data: &[u8],
        timeout: i32,
    ) -> Result<usize, UsbError> {
        if endpoint & 0x80 != 0x00 {
            return Err(UsbError::InvalidParam);
        }

        let interface = find_bulk_interface(self, endpoint)?;
        let mut bulk_out = interface
            .endpoint::<Bulk, Out>(endpoint)
            .map_err(|error| map_nusb_error(&error))?;
        let buffer = if data.is_empty() {
            Buffer::new(0)
        } else {
            data.to_vec().into()
        };
        let completion = bulk_out.transfer_blocking(buffer, duration_from_timeout(timeout));
        let actual_len = completion.actual_len;
        completion.into_result().map_err(map_transfer_error)?;
        Ok(actual_len)
    }

    pub fn get_string_simple(&mut self, string_index: u8) -> Result<String, UsbError> {
        if let Some(value) = self.string_descriptors.get(&string_index) {
            return Ok(value.clone());
        }

        let Some(string_index_nonzero) = NonZeroU8::new(string_index) else {
            return Ok(String::new());
        };

        let value = match self
            .device
            .get_string_descriptor(
                string_index_nonzero,
                language_id::US_ENGLISH,
                Duration::from_millis(250),
            )
            .wait()
        {
            Ok(value) => value,
            Err(error) => match error {
                nusb::GetDescriptorError::Transfer(error) => return Err(map_transfer_error(error)),
                nusb::GetDescriptorError::InvalidDescriptor => "?".to_string(),
            },
        };

        self.string_descriptors.insert(string_index, value.clone());
        Ok(value)
    }
}

pub fn strerror(error: UsbError) -> &'static str {
    match error {
        UsbError::Io => "input/output error",
        UsbError::InvalidParam => "invalid parameter",
        UsbError::Access => "access denied",
        UsbError::NoDevice => "no such device",
        UsbError::NotFound => "entity not found",
        UsbError::Busy => "resource busy",
        UsbError::Timeout => "operation timed out",
        UsbError::Overflow => "overflow",
        UsbError::Pipe => "pipe error",
        UsbError::Interrupted => "system call interrupted",
        UsbError::NoMem => "out of memory",
        UsbError::NotSupported => "operation not supported",
        UsbError::Other => "other error",
    }
}

pub fn error_is_timeout(error: UsbError) -> bool {
    error == UsbError::Timeout
}

pub fn error_is_access(error: UsbError) -> bool {
    error == UsbError::Access
}
