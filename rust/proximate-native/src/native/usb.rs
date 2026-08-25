/*-
 * Free/Libre Near Field Communication (NFC) library
 *
 * Libnfc historical contributors:
 * Copyright (C) 2009      Roel Verdult
 * Copyright (C) 2009-2013 Romuald Conty
 * Copyright (C) 2010-2012 Romain Tartière
 * Copyright (C) 2010-2017 Philippe Teuwen
 * Copyright (C) 2012-2013 Ludovic Rousseau
 * See AUTHORS file for a more comprehensive list of contributors.
 * Additional contributors of this file:
 *
 * This program is free software: you can redistribute it and/or modify it
 * under the terms of the GNU Lesser General Public License as published by the
 * Free Software Foundation, either version 3 of the License, or (at your
 * option) any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
 * more details.
 *
 * You should have received a copy of the GNU Lesser General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>
 */

use super::connstring::usb::{
    UsbSelector, build_usb_connstring_for, decode_usb_selector, select_usb_candidate,
};
use super::pn53x::{
    PN53X_ACK_FRAME, Pn53xDevice, Pn53xProfile, Pn53xTransport, Pn53xUsbModel, TransportSendError,
    probe_timeout,
};
use crate::command_abort::AtomicCommandAbort;
use crate::usb::{UsbDeviceInfo, UsbError, UsbHandle, bulk_endpoints, list_devices, strerror};
use proximate_driver::{
    CommandAbort, CommandAbortHandle, ConnectionString, Context, DeviceHandle, Driver, Error,
    OperationTimeout, ScanType,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

const DRIVER_NAME: &str = "pn53x_usb";
const NFC_EIO: i32 = -1;
const USB_ABORT_POLL_INTERVAL: Duration = Duration::from_millis(200);

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

    fn scan(&self, _context: &Context) -> Result<proximate_driver::DriverScan, Error> {
        let devices = list_devices().map_err(usb_open_error)?;

        let mut found = Vec::new();
        for info in devices {
            let Some(supported) = supported_device(&info) else {
                continue;
            };
            if let Ok(connstring) = build_usb_connstring_for(DRIVER_NAME, &info) {
                found
                    .push(self.describe_discovered(usb_display_name(&info, supported), connstring));
            }
        }

        Ok(proximate_driver::DriverScan::Complete(found))
    }

    fn open(
        &self,
        _context: &Context,
        connstring: &ConnectionString,
    ) -> Result<Box<dyn DeviceHandle>, Error> {
        let selector = decode_usb_selector(connstring)?;
        let (info, supported) = select_usb_device(selector)?;
        let display_name = usb_display_name(&info, supported);
        let transport = UsbTransport::open(&info, supported)?;
        let device = Pn53xDevice::probe_with_profile(
            display_name,
            connstring.clone(),
            Pn53xProfile::pn53x_usb(supported.model),
            transport,
            probe_timeout(),
        )?;
        Ok(Box::new(device))
    }
}

fn select_usb_device(selector: UsbSelector) -> Result<(UsbDeviceInfo, SupportedUsbDevice), Error> {
    let devices = list_devices().map_err(usb_open_error)?;
    let candidates = devices
        .into_iter()
        .filter_map(|info| supported_device(&info).map(|supported| (info, supported)));
    select_usb_candidate(DRIVER_NAME, &selector, candidates)
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
    command_abort: Arc<AtomicCommandAbort>,
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
            command_abort: AtomicCommandAbort::new(),
        })
    }

    fn aborted_receive_error(&mut self) -> Error {
        let operation = Error::Aborted("usb_receive");
        let recovery = self
            .handle
            .bulk_write(self.endpoint_out, &PN53X_ACK_FRAME, 1_000)
            .map_err(|error| map_usb_error("usb_abort_ack", error))
            .and_then(|written| {
                if written == PN53X_ACK_FRAME.len() {
                    Ok(())
                } else {
                    Err(Error::Io("usb_abort_ack"))
                }
            });
        match recovery {
            Ok(()) => operation,
            Err(recovery) => Error::RecoveryFailed {
                operation: Box::new(operation),
                recovery: Box::new(recovery),
            },
        }
    }
}

impl Pn53xTransport for UsbTransport {
    fn send(
        &mut self,
        payload: &[u8],
        timeout: OperationTimeout,
    ) -> Result<(), TransportSendError> {
        let timeout_ms = timeout
            .configured_millis()
            .map_err(TransportSendError::ProtocolStable)?;
        self.command_abort.begin_command();
        let sent = self
            .handle
            .bulk_write(self.endpoint_out, payload, timeout_ms)
            .map_err(|error| {
                TransportSendError::OutcomeUnknown(map_usb_error("usb_send", error))
            })?;
        if sent != payload.len() {
            return Err(TransportSendError::OutcomeUnknown(device_error(
                "usb_send", NFC_EIO,
            )));
        }
        Ok(())
    }

    fn receive(&mut self, buffer: &mut [u8], timeout: OperationTimeout) -> Result<usize, Error> {
        let timeout_ms = timeout.configured_millis()?;
        let started = Instant::now();
        loop {
            if self.command_abort.take_requested() {
                return Err(self.aborted_receive_error());
            }

            let pass_timeout = if timeout_ms <= 0 {
                USB_ABORT_POLL_INTERVAL.as_millis() as i32
            } else {
                let elapsed = started.elapsed();
                let total = Duration::from_millis(timeout_ms as u64);
                let Some(remaining) = total.checked_sub(elapsed) else {
                    return Err(Error::Timeout("usb_receive"));
                };
                remaining.min(USB_ABORT_POLL_INTERVAL).as_millis().max(1) as i32
            };

            match self
                .handle
                .bulk_read(self.endpoint_in, buffer, pass_timeout)
            {
                Ok(received) => return Ok(received),
                Err(UsbError::Timeout) => {
                    if self.command_abort.take_requested() {
                        return Err(self.aborted_receive_error());
                    }
                    if timeout_ms > 0
                        && started.elapsed() >= Duration::from_millis(timeout_ms as u64)
                    {
                        return Err(Error::Timeout("usb_receive"));
                    }
                }
                Err(error) => return Err(map_usb_error("usb_receive", error)),
            }
        }
    }

    fn abort_command(&mut self) -> Result<(), Error> {
        self.command_abort.abort()
    }

    fn command_abort_handle(&self) -> Option<CommandAbortHandle> {
        Some(self.command_abort.clone())
    }
}

impl Drop for UsbTransport {
    fn drop(&mut self) {
        self.command_abort.revoke();
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
    match error {
        UsbError::Timeout => Error::Timeout(operation),
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
        | UsbError::Other => device_error(operation, NFC_EIO),
    }
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
            Error::Timeout("usb_receive")
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
