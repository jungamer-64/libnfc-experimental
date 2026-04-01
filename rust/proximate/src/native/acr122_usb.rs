use super::acr122;
use super::connstring::{UsbSelector, build_usb_connstring_for, decode_usb_selector_for};
use super::pn53x::{
    PN53X_ACK_FRAME, Pn53xDevice, Pn53xProfile, Pn53xTransport, build_response_frame,
    payload_from_host_frame,
};
use crate::rust_api::{ConnectionString, Context, Driver, Error, OpenedDevice, Property, ScanType};
use nusb::descriptors::TransferType;
use nusb::transfer::{Buffer, Bulk, In, Out, TransferError};
use nusb::{Device, DeviceInfo, Interface, MaybeFuture};
use std::collections::VecDeque;
#[cfg(test)]
use std::sync::{Arc, Mutex};
use std::time::Duration;

const DRIVER_NAME: &str = "acr122_usb";
const PROBE_TIMEOUT_MS: i32 = 250;
const CONTROL_TIMEOUT_MS: i32 = 1_000;
const RESPONSE_BUFFER_LEN: usize = 255 + 10;

const NFC_EIO: i32 = -1;
const NFC_ETIMEOUT: i32 = -6;

const ACR122_CCID_PC_TO_RDR_ICC_POWER_ON: u8 = 0x62;
const ACR122_CCID_PC_TO_RDR_XFR_BLOCK: u8 = 0x6F;
const ACR122_CCID_RDR_TO_PC_DATABLOCK: u8 = 0x80;
const ACR122_PN53X_READER_TO_HOST: u8 = 0xD5;

pub(crate) struct Acr122UsbDriver;

impl Acr122UsbDriver {
    pub(crate) const fn new() -> Self {
        Self
    }
}

impl Driver for Acr122UsbDriver {
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
            if !acr122::is_usb_device(info.vendor_id(), info.product_id()) {
                continue;
            }
            found.push(build_usb_connstring_for(
                DRIVER_NAME,
                device_bus_number(&info),
                info.device_address(),
            )?);
        }

        Ok(found)
    }

    fn open(
        &self,
        _context: &Context,
        connstring: &ConnectionString,
    ) -> Result<Box<dyn OpenedDevice>, Error> {
        let selector = decode_usb_selector_for(connstring, DRIVER_NAME)?;
        let info = select_usb_device(selector)?;
        let display_name = usb_display_name(&info);

        let mut handle = UsbCcidHandle::open(&info)?;
        handle.power_on(CONTROL_TIMEOUT_MS)?;
        handle.configure_operating_parameters(CONTROL_TIMEOUT_MS)?;

        let transport = Acr122UsbTransport::new(handle);
        let mut device = Pn53xDevice::probe_with_profile(
            display_name,
            connstring.clone(),
            Pn53xProfile::acr122_usb(),
            transport,
            PROBE_TIMEOUT_MS,
        )?;
        device.set_property_int(Property::TimeoutCommand, CONTROL_TIMEOUT_MS)?;
        Ok(Box::new(device))
    }
}

fn select_usb_device(selector: UsbSelector) -> Result<DeviceInfo, Error> {
    let devices = nusb::list_devices()
        .wait()
        .map_err(|error| Error::DriverOpenFailed(error.to_string()))?;

    for info in devices {
        if !acr122::is_usb_device(info.vendor_id(), info.product_id()) {
            continue;
        }
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
        return Ok(info);
    }

    Err(Error::DriverOpenFailed(
        "no supported acr122 USB device is available".into(),
    ))
}

fn usb_display_name(info: &DeviceInfo) -> String {
    match (info.manufacturer_string(), info.product_string()) {
        (Some(manufacturer), Some(product)) if !manufacturer.is_empty() && !product.is_empty() => {
            format!("{manufacturer} / {product}")
        }
        _ => acr122::usb_device_name(info.vendor_id(), info.product_id())
            .unwrap_or("ACS ACR122")
            .to_string(),
    }
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

fn device_error(operation: &'static str, code: i32) -> Error {
    Error::DeviceOperationFailed { operation, code }
}

fn map_transfer_error(operation: &'static str, error: TransferError) -> Error {
    let code = match error {
        TransferError::Cancelled => NFC_ETIMEOUT,
        TransferError::Disconnected
        | TransferError::Fault
        | TransferError::InvalidArgument
        | TransferError::Stall
        | TransferError::Unknown(_) => NFC_EIO,
    };
    device_error(operation, code)
}

struct EndpointSelection {
    configuration_value: u8,
    interface_number: u8,
    alternate_setting: u8,
    endpoint_in: u8,
    endpoint_out: u8,
    max_packet_size: u16,
}

fn resolve_endpoints(device: &Device) -> Result<EndpointSelection, Error> {
    for configuration in device.configurations() {
        for interface_group in configuration.interfaces() {
            let alt = interface_group.first_alt_setting();
            let mut endpoint_in = None;
            let mut endpoint_out = None;
            let mut max_packet_size = 0u16;

            for endpoint in alt.endpoints() {
                if endpoint.transfer_type() != TransferType::Bulk {
                    continue;
                }
                if endpoint.address() & 0x80 != 0 {
                    endpoint_in = Some(endpoint.address());
                } else {
                    endpoint_out = Some(endpoint.address());
                }
                max_packet_size = max_packet_size.max(endpoint.max_packet_size() as u16);
            }

            if let (Some(endpoint_in), Some(endpoint_out)) = (endpoint_in, endpoint_out) {
                return Ok(EndpointSelection {
                    configuration_value: configuration.configuration_value(),
                    interface_number: alt.interface_number(),
                    alternate_setting: alt.alternate_setting(),
                    endpoint_in,
                    endpoint_out,
                    max_packet_size,
                });
            }
        }
    }

    Err(Error::DriverOpenFailed(
        "failed to discover ACR122 USB bulk endpoints".into(),
    ))
}

trait Acr122UsbIo: Send {
    fn bulk_read(&mut self, buffer: &mut [u8], timeout_ms: i32) -> Result<usize, Error>;
    fn bulk_write(&mut self, payload: &[u8], timeout_ms: i32) -> Result<(), Error>;

    fn write_ccid_message(
        &mut self,
        message_type: u8,
        message_specific: [u8; 3],
        payload: &[u8],
        timeout_ms: i32,
    ) -> Result<(), Error> {
        let mut frame = Vec::with_capacity(10 + payload.len());
        frame.push(message_type);
        frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        frame.push(0x00);
        frame.push(0x00);
        frame.extend_from_slice(&message_specific);
        frame.extend_from_slice(payload);
        self.bulk_write(&frame, timeout_ms)
    }

    fn read_raw(&mut self, timeout_ms: i32) -> Result<Vec<u8>, Error> {
        let mut buffer = [0u8; RESPONSE_BUFFER_LEN];
        let size = self.bulk_read(&mut buffer, timeout_ms)?;
        Ok(buffer[..size].to_vec())
    }

    fn send_apdu(&mut self, apdu: &[u8], timeout_ms: i32) -> Result<Vec<u8>, Error> {
        self.write_ccid_message(ACR122_CCID_PC_TO_RDR_XFR_BLOCK, [0, 0, 0], apdu, timeout_ms)?;
        let response = self.read_raw(timeout_ms)?;
        parse_ccid_data_block(&response)
    }

    fn power_on(&mut self, timeout_ms: i32) -> Result<(), Error> {
        self.write_ccid_message(
            ACR122_CCID_PC_TO_RDR_ICC_POWER_ON,
            [0x01, 0x00, 0x00],
            &[],
            timeout_ms,
        )?;
        let _ = self.read_raw(timeout_ms)?;
        Ok(())
    }

    fn configure_operating_parameters(&mut self, timeout_ms: i32) -> Result<(), Error> {
        let apdu = acr122::build_apdu(0x00, 0x51, 0x00, &[], 0x00)?;
        let _ = self.send_apdu(&apdu, timeout_ms)?;
        Ok(())
    }

    fn direct_transmit(
        &mut self,
        command: u8,
        host_payload: &[u8],
        timeout_ms: i32,
    ) -> Result<Vec<u8>, Error> {
        let apdu = acr122::build_direct_transmit_apdu(host_payload)?;
        let response = self.send_apdu(&apdu, timeout_ms)?;
        self.complete_direct_transmit(command, response, timeout_ms)
    }

    fn complete_direct_transmit(
        &mut self,
        command: u8,
        response: Vec<u8>,
        timeout_ms: i32,
    ) -> Result<Vec<u8>, Error> {
        if let Some(payload) = extract_direct_transmit_payload(command, &response)? {
            return Ok(payload);
        }

        let status = acr122::parse_status_words(&response)
            .ok_or_else(|| device_error("acr122_usb_receive", NFC_EIO))?;
        if !status.has_more_data {
            return Err(device_error("acr122_usb_receive", NFC_EIO));
        }

        let follow_up = acr122::build_get_additional_data_apdu(status.more_data_length)?;
        let follow_up_response = self.send_apdu(&follow_up, timeout_ms)?;
        extract_direct_transmit_payload(command, &follow_up_response)?
            .ok_or_else(|| device_error("acr122_usb_receive", NFC_EIO))
    }
}

struct UsbCcidHandle {
    _device: Device,
    interface: Interface,
    endpoint_in: u8,
    endpoint_out: u8,
    max_packet_size: u16,
}

impl UsbCcidHandle {
    fn open(info: &DeviceInfo) -> Result<Self, Error> {
        let device = info
            .open()
            .wait()
            .map_err(|error| Error::DriverOpenFailed(error.to_string()))?;
        let selection = resolve_endpoints(&device)?;
        device
            .reset()
            .wait()
            .map_err(|error| Error::DriverOpenFailed(error.to_string()))?;
        drop(device);

        let device = info
            .open()
            .wait()
            .map_err(|error| Error::DriverOpenFailed(error.to_string()))?;
        device
            .set_configuration(selection.configuration_value)
            .wait()
            .map_err(|error| Error::DriverOpenFailed(error.to_string()))?;
        let interface = device
            .claim_interface(selection.interface_number)
            .wait()
            .map_err(|error| Error::DriverOpenFailed(error.to_string()))?;
        if selection.alternate_setting != 0 {
            interface
                .set_alt_setting(selection.alternate_setting)
                .wait()
                .map_err(|error| Error::DriverOpenFailed(error.to_string()))?;
        }

        Ok(Self {
            _device: device,
            interface,
            endpoint_in: selection.endpoint_in,
            endpoint_out: selection.endpoint_out,
            max_packet_size: selection.max_packet_size,
        })
    }
}

impl Acr122UsbIo for UsbCcidHandle {
    fn bulk_read(&mut self, buffer: &mut [u8], timeout_ms: i32) -> Result<usize, Error> {
        let mut bulk_in = self
            .interface
            .endpoint::<Bulk, In>(self.endpoint_in)
            .map_err(|_| device_error("acr122_usb_bulk_read", NFC_EIO))?;
        let completion =
            bulk_in.transfer_blocking(Buffer::new(buffer.len()), transfer_timeout(timeout_ms));
        let actual_len = completion.actual_len.min(buffer.len());
        match completion.into_result() {
            Ok(received) => {
                let data = received.into_vec();
                buffer[..actual_len].copy_from_slice(&data[..actual_len]);
                Ok(actual_len)
            }
            Err(error) => Err(map_transfer_error("acr122_usb_bulk_read", error)),
        }
    }

    fn bulk_write(&mut self, payload: &[u8], timeout_ms: i32) -> Result<(), Error> {
        let mut bulk_out = self
            .interface
            .endpoint::<Bulk, Out>(self.endpoint_out)
            .map_err(|_| device_error("acr122_usb_bulk_write", NFC_EIO))?;
        let completion =
            bulk_out.transfer_blocking(payload.to_vec().into(), transfer_timeout(timeout_ms));
        let actual_len = completion.actual_len;
        completion
            .into_result()
            .map_err(|error| map_transfer_error("acr122_usb_bulk_write", error))?;
        if actual_len != payload.len() {
            return Err(device_error("acr122_usb_bulk_write", NFC_EIO));
        }
        if self.max_packet_size != 0
            && !payload.is_empty()
            && actual_len % usize::from(self.max_packet_size) == 0
        {
            let completion =
                bulk_out.transfer_blocking(Buffer::new(0), transfer_timeout(timeout_ms));
            completion
                .into_result()
                .map_err(|error| map_transfer_error("acr122_usb_bulk_write", error))?;
        }
        Ok(())
    }
}

fn parse_ccid_data_block(frame: &[u8]) -> Result<Vec<u8>, Error> {
    if frame.len() < 10 || frame[0] != ACR122_CCID_RDR_TO_PC_DATABLOCK {
        return Err(device_error("acr122_usb_receive", NFC_EIO));
    }

    let payload_len = u32::from_le_bytes([frame[1], frame[2], frame[3], frame[4]]) as usize;
    let error = frame[8];
    if payload_len == 0 && error == 0xFE {
        return Err(device_error("acr122_usb_receive", NFC_ETIMEOUT));
    }
    if frame.len() < 10 + payload_len {
        return Err(device_error("acr122_usb_receive", NFC_EIO));
    }
    Ok(frame[10..10 + payload_len].to_vec())
}

fn extract_direct_transmit_payload(command: u8, response: &[u8]) -> Result<Option<Vec<u8>>, Error> {
    if response.is_empty() || response[0] != ACR122_PN53X_READER_TO_HOST {
        return Ok(None);
    }
    if response.len() < 4 || response[1] != command.wrapping_add(1) {
        return Err(device_error("acr122_usb_receive", NFC_EIO));
    }
    let status = acr122::parse_status_words(&response[response.len() - 2..])
        .ok_or_else(|| device_error("acr122_usb_receive", NFC_EIO))?;
    if !status.ok {
        return Err(device_error("acr122_usb_receive", NFC_EIO));
    }
    Ok(Some(response[2..response.len() - 2].to_vec()))
}

struct Acr122UsbTransport<IO> {
    io: IO,
    pending: VecDeque<Vec<u8>>,
}

impl<IO> Acr122UsbTransport<IO> {
    fn new(io: IO) -> Self {
        Self {
            io,
            pending: VecDeque::new(),
        }
    }
}

impl<IO: Acr122UsbIo> Pn53xTransport for Acr122UsbTransport<IO> {
    fn send(&mut self, payload: &[u8], timeout_ms: i32) -> Result<(), Error> {
        let host_payload = payload_from_host_frame(payload)?;
        let command = *host_payload
            .first()
            .ok_or_else(|| device_error("acr122_usb_send", NFC_EIO))?;
        let response = self
            .io
            .direct_transmit(command, &host_payload, timeout_ms)?;
        let frame = build_response_frame(command, &response)?;
        self.pending.clear();
        self.pending.push_back(PN53X_ACK_FRAME.to_vec());
        self.pending.push_back(frame);
        Ok(())
    }

    fn receive(&mut self, buffer: &mut [u8], _timeout_ms: i32) -> Result<usize, Error> {
        let payload = self
            .pending
            .pop_front()
            .ok_or_else(|| device_error("acr122_usb_pending_receive", NFC_EIO))?;
        if payload.len() > buffer.len() {
            return Err(Error::BufferTooSmall {
                needed: payload.len(),
                available: buffer.len(),
            });
        }
        buffer[..payload.len()].copy_from_slice(&payload);
        Ok(payload.len())
    }

    fn abort_command(&mut self) -> Result<(), Error> {
        self.pending.clear();
        Ok(())
    }
}

impl Acr122UsbIo for Box<dyn Acr122UsbIo> {
    fn bulk_read(&mut self, buffer: &mut [u8], timeout_ms: i32) -> Result<usize, Error> {
        self.as_mut().bulk_read(buffer, timeout_ms)
    }

    fn bulk_write(&mut self, payload: &[u8], timeout_ms: i32) -> Result<(), Error> {
        self.as_mut().bulk_write(payload, timeout_ms)
    }
}

#[cfg(test)]
#[derive(Default)]
struct FakeUsbIoState {
    writes: Vec<Vec<u8>>,
    reads: VecDeque<Vec<u8>>,
}

#[cfg(test)]
#[derive(Clone, Default)]
struct FakeUsbIo {
    state: Arc<Mutex<FakeUsbIoState>>,
}

#[cfg(test)]
impl FakeUsbIo {
    fn with_reads(reads: impl IntoIterator<Item = Vec<u8>>) -> Self {
        let state = FakeUsbIoState {
            writes: Vec::new(),
            reads: reads.into_iter().collect(),
        };
        Self {
            state: Arc::new(Mutex::new(state)),
        }
    }

    fn writes(&self) -> Vec<Vec<u8>> {
        self.state
            .lock()
            .expect("poisoned fake USB state")
            .writes
            .clone()
    }
}

#[cfg(test)]
impl Acr122UsbIo for FakeUsbIo {
    fn bulk_read(&mut self, buffer: &mut [u8], _timeout_ms: i32) -> Result<usize, Error> {
        let mut state = self.state.lock().expect("poisoned fake USB state");
        let payload = state
            .reads
            .pop_front()
            .ok_or_else(|| device_error("fake_bulk_read", NFC_EIO))?;
        if payload.len() > buffer.len() {
            return Err(device_error("fake_bulk_read", NFC_EIO));
        }
        buffer[..payload.len()].copy_from_slice(&payload);
        Ok(payload.len())
    }

    fn bulk_write(&mut self, payload: &[u8], _timeout_ms: i32) -> Result<(), Error> {
        self.state
            .lock()
            .expect("poisoned fake USB state")
            .writes
            .push(payload.to_vec());
        Ok(())
    }
}

#[cfg(test)]
fn build_ccid_data_block(payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(10 + payload.len());
    frame.push(ACR122_CCID_RDR_TO_PC_DATABLOCK);
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00]);
    frame.extend_from_slice(payload);
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_builds_pending_ack_and_response_frames() {
        let io = FakeUsbIo::with_reads([build_ccid_data_block(&[0xD5, 0x03, 0x90, 0x00])]);
        let mut transport = Acr122UsbTransport::new(io);

        let frame = crate::native::pn53x::build_frame(&[0x02]).unwrap();
        transport.send(&frame, 25).unwrap();

        let mut ack = [0u8; 6];
        assert_eq!(transport.receive(&mut ack, 25).unwrap(), 6);
        assert_eq!(ack, PN53X_ACK_FRAME);

        let mut response = [0u8; 32];
        let size = transport.receive(&mut response, 25).unwrap();
        assert!(size > 0);
    }

    #[test]
    fn transport_fetches_additional_data_when_reader_requests_follow_up() {
        let io = FakeUsbIo::with_reads([
            build_ccid_data_block(&[0x61, 0x04]),
            build_ccid_data_block(&[0xD5, 0x03, 0x32, 0x01, 0x06, 0x07, 0x90, 0x00]),
        ]);
        let writes = io.clone();
        let mut transport = Acr122UsbTransport::new(io);

        let frame = crate::native::pn53x::build_frame(&[0x02]).unwrap();
        transport.send(&frame, 25).unwrap();

        let writes = writes.writes();
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[1][10..15], [0xFF, 0xC0, 0x00, 0x00, 0x04]);
    }
}
