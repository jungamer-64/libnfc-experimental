use super::connstring::{UsbSelector, build_usb_connstring, decode_usb_selector};
use super::pn53x::{Pn53xDevice, Pn53xProfile, Pn53xTransport, Pn53xUsbModel};
use crate::usb::{UsbDeviceInfo, UsbError, UsbHandle, bulk_endpoints, list_devices, strerror};
use proximate_driver::{ConnectionString, Context, Driver, Error, OpenedDevice, ScanType};

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
        let devices = list_devices().map_err(usb_open_error)?;

        let mut found = Vec::new();
        for info in devices {
            if supported_device(&info).is_none() {
                continue;
            }
            if let Ok(connstring) = build_usb_connstring(info.bus_number, info.device_address) {
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

fn select_usb_device(selector: UsbSelector) -> Result<(UsbDeviceInfo, SupportedUsbDevice), Error> {
    let devices = list_devices().map_err(usb_open_error)?;

    for info in devices {
        let Some(supported) = supported_device(&info) else {
            continue;
        };
        if let Some(bus) = selector.bus
            && info.bus_number != bus
        {
            continue;
        }
        if let Some(device) = selector.device
            && info.device_address != device
        {
            continue;
        }
        return Ok((info, supported));
    }

    Err(Error::DriverOpenFailed(
        "no supported pn53x USB device is available".into(),
    ))
}

fn usb_display_name(info: &UsbDeviceInfo, supported: SupportedUsbDevice) -> String {
    match (
        info.manufacturer_string.as_deref(),
        info.product_string.as_deref(),
    ) {
        (Some(manufacturer), Some(product)) if !manufacturer.is_empty() && !product.is_empty() => {
            format!("{manufacturer} / {product}")
        }
        _ => supported.display_name.to_string(),
    }
}

fn supported_device(info: &UsbDeviceInfo) -> Option<SupportedUsbDevice> {
    SUPPORTED_DEVICES
        .iter()
        .copied()
        .find(|device| device.vendor_id == info.vendor_id && device.product_id == info.product_id)
}

fn usb_open_error(error: UsbError) -> Error {
    Error::DriverOpenFailed(strerror(error).to_string())
}

pub struct UsbTransport {
    handle: UsbHandle,
    endpoint_in: u8,
    endpoint_out: u8,
}

impl UsbTransport {
    fn open(info: &UsbDeviceInfo, supported: SupportedUsbDevice) -> Result<Self, Error> {
        let mut handle = UsbHandle::open(info).map_err(usb_open_error)?;
        let endpoint_selection = resolve_endpoints(info, supported)?;
        if info.configuration_value != 0 {
            handle
                .set_configuration(info.configuration_value)
                .map_err(usb_open_error)?;
        }
        handle
            .claim_interface(endpoint_selection.interface_number)
            .map_err(usb_open_error)?;

        if endpoint_selection.alternate_setting != 0 {
            handle
                .set_altinterface(
                    endpoint_selection.interface_number,
                    endpoint_selection.alternate_setting,
                )
                .map_err(usb_open_error)?;
        }

        Ok(Self {
            handle,
            endpoint_in: endpoint_selection.endpoint_in,
            endpoint_out: endpoint_selection.endpoint_out,
        })
    }
}

impl Pn53xTransport for UsbTransport {
    fn send(&mut self, payload: &[u8], timeout_ms: i32) -> Result<(), Error> {
        let sent = self
            .handle
            .bulk_write(self.endpoint_out, payload, timeout_ms)
            .map_err(|error| map_usb_error("usb_send", error))?;
        if sent != payload.len() {
            return Err(device_error("usb_send", NFC_EIO));
        }
        Ok(())
    }

    fn receive(&mut self, buffer: &mut [u8], timeout_ms: i32) -> Result<usize, Error> {
        self.handle
            .bulk_read(self.endpoint_in, buffer, timeout_ms)
            .map_err(|error| map_usb_error("usb_receive", error))
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
    device: &UsbDeviceInfo,
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

    let endpoints = bulk_endpoints(device)
        .ok_or_else(|| Error::DriverOpenFailed("failed to discover bulk USB endpoints".into()))?;
    Ok(EndpointSelection {
        interface_number: endpoints.interface_number,
        alternate_setting: endpoints.alternate_setting as u8,
        endpoint_in: endpoints.endpoint_in,
        endpoint_out: endpoints.endpoint_out,
    })
}

fn device_error(operation: &'static str, code: i32) -> Error {
    Error::DeviceOperationFailed { operation, code }
}

fn map_usb_error(operation: &'static str, error: UsbError) -> Error {
    let code = match error {
        UsbError::Timeout => NFC_ETIMEOUT,
        UsbError::NoDevice
        | UsbError::Io
        | UsbError::InvalidParam
        | UsbError::Access
        | UsbError::NotFound
        | UsbError::Busy
        | UsbError::Overflow
        | UsbError::Pipe
        | UsbError::Interrupted
        | UsbError::NoMem
        | UsbError::NotSupported
        | UsbError::Other => NFC_EIO,
    };
    device_error(operation, code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usb_driver_metadata_is_stable() {
        let driver = Pn53xUsbDriver::new();
        assert_eq!(driver.name(), DRIVER_NAME);
        assert_eq!(driver.scan_type(), ScanType::NotIntrusive);
    }

    #[test]
    fn usb_error_mapping_preserves_timeout_and_io_classes() {
        assert!(matches!(
            map_usb_error("usb_receive", UsbError::Timeout),
            Error::DeviceOperationFailed {
                operation: "usb_receive",
                code: NFC_ETIMEOUT
            }
        ));
        assert!(matches!(
            map_usb_error("usb_send", UsbError::Pipe),
            Error::DeviceOperationFailed {
                operation: "usb_send",
                code: NFC_EIO
            }
        ));
    }
}
