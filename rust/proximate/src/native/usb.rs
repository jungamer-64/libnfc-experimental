use super::connstring::{UsbSelector, build_usb_connstring, decode_usb_selector};
use super::pn53x::{Pn53xDevice, Pn53xProfile, Pn53xTransport, Pn53xUsbModel};
use crate::rust_api::{ConnectionString, Context, Driver, Error, OpenedDevice, ScanType};
use nusb::descriptors::TransferType;
use nusb::transfer::{Buffer, Bulk, In, Out, TransferError};
use nusb::{Device, DeviceInfo, Interface, MaybeFuture};
use std::time::Duration;

const DRIVER_NAME: &str = "pn53x_usb";
const PROBE_TIMEOUT_MS: i32 = 250;
const NFC_EIO: i32 = -1;
const NFC_ETIMEOUT: i32 = -6;

#[derive(Clone, Copy)]
struct SupportedUsbDevice {
    vendor_id: u16,
    product_id: u16,
    model: Pn53xUsbModel,
    display_name: &'static str,
    endpoint_in: Option<u8>,
    endpoint_out: Option<u8>,
}

const SUPPORTED_DEVICES: &[SupportedUsbDevice] = &[
    SupportedUsbDevice {
        vendor_id: 0x04CC,
        product_id: 0x0531,
        model: Pn53xUsbModel::NxpPn531,
        display_name: "Philips / PN531",
        endpoint_in: Some(0x84),
        endpoint_out: Some(0x04),
    },
    SupportedUsbDevice {
        vendor_id: 0x04CC,
        product_id: 0x2533,
        model: Pn53xUsbModel::NxpPn533,
        display_name: "NXP / PN533",
        endpoint_in: Some(0x84),
        endpoint_out: Some(0x04),
    },
    SupportedUsbDevice {
        vendor_id: 0x04E6,
        product_id: 0x5591,
        model: Pn53xUsbModel::ScmScl3711,
        display_name: "SCM Micro / SCL3711-NFC&RW",
        endpoint_in: Some(0x84),
        endpoint_out: Some(0x04),
    },
    SupportedUsbDevice {
        vendor_id: 0x04E6,
        product_id: 0x5594,
        model: Pn53xUsbModel::ScmScl3712,
        display_name: "SCM Micro / SCL3712-NFC&RW",
        endpoint_in: None,
        endpoint_out: None,
    },
    SupportedUsbDevice {
        vendor_id: 0x054C,
        product_id: 0x0193,
        model: Pn53xUsbModel::SonyPn531,
        display_name: "Sony / PN531",
        endpoint_in: Some(0x84),
        endpoint_out: Some(0x04),
    },
    SupportedUsbDevice {
        vendor_id: 0x1FD3,
        product_id: 0x0608,
        model: Pn53xUsbModel::AskLogo,
        display_name: "ASK / LoGO",
        endpoint_in: Some(0x84),
        endpoint_out: Some(0x04),
    },
    SupportedUsbDevice {
        vendor_id: 0x054C,
        product_id: 0x02E1,
        model: Pn53xUsbModel::SonyRcs360,
        display_name: "Sony / FeliCa S360 [PaSoRi]",
        endpoint_in: Some(0x84),
        endpoint_out: Some(0x04),
    },
];

pub(crate) struct Pn53xUsbDriver;

impl Pn53xUsbDriver {
    pub(crate) const fn new() -> Self {
        Self
    }
}

impl Driver for Pn53xUsbDriver {
    fn name(&self) -> &str {
        DRIVER_NAME
    }

    fn scan_type(&self) -> ScanType {
        ScanType::NotIntrusive
    }

    fn scan(&self, _context: &Context) -> Result<Vec<ConnectionString>, Error> {
        let devices = nusb::list_devices()
            .wait()
            .map_err(|error| Error::DriverOpenFailed(error.to_string()))?;

        let mut found = Vec::new();
        for info in devices {
            if supported_device(&info).is_none() {
                continue;
            }
            if let Ok(connstring) =
                build_usb_connstring(device_bus_number(&info), info.device_address())
            {
                found.push(connstring);
            }
        }

        Ok(found)
    }

    fn open(
        &self,
        _context: &Context,
        connstring: &ConnectionString,
    ) -> Result<Box<dyn OpenedDevice>, Error> {
        let selector = decode_usb_selector(connstring)?;
        let (info, supported) = select_usb_device(selector)?;
        let display_name = usb_display_name(&info, supported);
        let transport = UsbTransport::open(&info, supported)?;
        let device = Pn53xDevice::probe_with_profile(
            display_name,
            connstring.clone(),
            Pn53xProfile::pn53x_usb(supported.model),
            transport,
            PROBE_TIMEOUT_MS,
        )?;
        Ok(Box::new(device))
    }
}

fn select_usb_device(selector: UsbSelector) -> Result<(DeviceInfo, SupportedUsbDevice), Error> {
    let devices = nusb::list_devices()
        .wait()
        .map_err(|error| Error::DriverOpenFailed(error.to_string()))?;

    for info in devices {
        let Some(supported) = supported_device(&info) else {
            continue;
        };
        if let Some(bus) = selector.bus
            && device_bus_number(&info) != bus
        {
            continue;
        }
        if let Some(device) = selector.device
            && info.device_address() != device
        {
            continue;
        }
        return Ok((info, supported));
    }

    Err(Error::DriverOpenFailed(
        "no supported pn53x USB device is available".into(),
    ))
}

fn usb_display_name(info: &DeviceInfo, supported: SupportedUsbDevice) -> String {
    match (info.manufacturer_string(), info.product_string()) {
        (Some(manufacturer), Some(product)) if !manufacturer.is_empty() && !product.is_empty() => {
            format!("{manufacturer} / {product}")
        }
        _ => supported.display_name.to_string(),
    }
}

fn supported_device(info: &DeviceInfo) -> Option<SupportedUsbDevice> {
    SUPPORTED_DEVICES.iter().copied().find(|device| {
        device.vendor_id == info.vendor_id() && device.product_id == info.product_id()
    })
}

fn device_bus_number(info: &DeviceInfo) -> u8 {
    #[cfg(target_os = "linux")]
    {
        info.busnum()
    }

    #[cfg(not(target_os = "linux"))]
    {
        info.bus_id().parse::<u8>().unwrap_or(0)
    }
}

fn transfer_timeout(timeout_ms: i32) -> Duration {
    if timeout_ms <= 0 {
        Duration::from_secs(5)
    } else {
        Duration::from_millis(timeout_ms as u64)
    }
}

pub struct UsbTransport {
    _device: Device,
    interface: Interface,
    endpoint_in: u8,
    endpoint_out: u8,
}

impl UsbTransport {
    fn open(info: &DeviceInfo, supported: SupportedUsbDevice) -> Result<Self, Error> {
        let device = info
            .open()
            .wait()
            .map_err(|error| Error::DriverOpenFailed(error.to_string()))?;

        let endpoint_selection = resolve_endpoints(&device, supported)?;
        let interface = device
            .claim_interface(endpoint_selection.interface_number)
            .wait()
            .map_err(|error| Error::DriverOpenFailed(error.to_string()))?;

        if endpoint_selection.alternate_setting != 0 {
            interface
                .set_alt_setting(endpoint_selection.alternate_setting)
                .wait()
                .map_err(|error| Error::DriverOpenFailed(error.to_string()))?;
        }

        Ok(Self {
            _device: device,
            interface,
            endpoint_in: endpoint_selection.endpoint_in,
            endpoint_out: endpoint_selection.endpoint_out,
        })
    }
}

impl Pn53xTransport for UsbTransport {
    fn send(&mut self, payload: &[u8], timeout_ms: i32) -> Result<(), Error> {
        let mut bulk_out = self
            .interface
            .endpoint::<Bulk, Out>(self.endpoint_out)
            .map_err(|_| device_error("usb_send", NFC_EIO))?;
        let completion =
            bulk_out.transfer_blocking(payload.to_vec().into(), transfer_timeout(timeout_ms));
        match completion.into_result() {
            Ok(_) => Ok(()),
            Err(error) => Err(map_transfer_error("usb_send", error)),
        }
    }

    fn receive(&mut self, buffer: &mut [u8], timeout_ms: i32) -> Result<usize, Error> {
        let mut bulk_in = self
            .interface
            .endpoint::<Bulk, In>(self.endpoint_in)
            .map_err(|_| device_error("usb_receive", NFC_EIO))?;
        let completion =
            bulk_in.transfer_blocking(Buffer::new(buffer.len()), transfer_timeout(timeout_ms));
        let actual_len = completion.actual_len.min(buffer.len());
        match completion.into_result() {
            Ok(received) => {
                let data = received.into_vec();
                buffer[..actual_len].copy_from_slice(&data[..actual_len]);
                Ok(actual_len)
            }
            Err(error) => Err(map_transfer_error("usb_receive", error)),
        }
    }

    fn abort_command(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

struct EndpointSelection {
    interface_number: u8,
    alternate_setting: u8,
    endpoint_in: u8,
    endpoint_out: u8,
}

fn resolve_endpoints(
    device: &Device,
    supported: SupportedUsbDevice,
) -> Result<EndpointSelection, Error> {
    if let (Some(endpoint_in), Some(endpoint_out)) = (supported.endpoint_in, supported.endpoint_out)
    {
        return Ok(EndpointSelection {
            interface_number: 0,
            alternate_setting: 0,
            endpoint_in,
            endpoint_out,
        });
    }

    let configuration = device
        .active_configuration()
        .map_err(|error| Error::DriverOpenFailed(error.to_string()))?;
    for interface_group in configuration.interfaces() {
        let alt = interface_group.first_alt_setting();
        let mut endpoint_in = None;
        let mut endpoint_out = None;
        for endpoint in alt.endpoints() {
            if endpoint.transfer_type() != TransferType::Bulk {
                continue;
            }
            if (endpoint.address() & 0x80) != 0 {
                endpoint_in = Some(endpoint.address());
            } else {
                endpoint_out = Some(endpoint.address());
            }
        }
        if let (Some(endpoint_in), Some(endpoint_out)) = (endpoint_in, endpoint_out) {
            return Ok(EndpointSelection {
                interface_number: alt.interface_number(),
                alternate_setting: alt.alternate_setting(),
                endpoint_in,
                endpoint_out,
            });
        }
    }

    Err(Error::DriverOpenFailed(
        "failed to discover bulk USB endpoints".into(),
    ))
}

fn device_error(operation: &'static str, code: i32) -> Error {
    Error::DeviceOperationFailed { operation, code }
}

fn map_transfer_error(operation: &'static str, error: TransferError) -> Error {
    let code = match error {
        TransferError::Cancelled => NFC_ETIMEOUT,
        TransferError::Disconnected => NFC_EIO,
        TransferError::Fault => NFC_EIO,
        TransferError::InvalidArgument => NFC_EIO,
        TransferError::Stall => NFC_EIO,
        TransferError::Unknown(_) => NFC_EIO,
    };
    device_error(operation, code)
}
