use super::acr122;
use super::pcsc::{
    PcscBackend, PcscCard, PcscProtocols, PcscShareMode, ReaderFilter, SystemPcscBackend,
    resolve_reader,
};
use super::pn53x::{
    PN53X_ACK_FRAME, Pn53xDevice, Pn53xProfile, Pn53xTransport, build_response_frame,
    payload_from_host_frame,
};
use crate::rust_api::{ConnectionString, Context, Driver, Error, OpenedDevice, ScanType};
use std::collections::VecDeque;
use std::sync::Arc;

const DRIVER_NAME: &str = "acr122_pcsc";
const NFC_EIO: i32 = -1;
const ACR122_PCSC_RESPONSE_LEN: usize = 268;

#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
const IOCTL_CCID_ESCAPE_SCARD_CTL_CODE: u64 = (((0x31u32) << 16) | (3500u32 << 2)) as u64;
#[cfg(all(
    not(windows),
    not(target_os = "linux"),
    not(target_os = "macos"),
    not(target_os = "freebsd"),
    not(target_os = "openbsd"),
    not(target_os = "netbsd")
))]
const IOCTL_CCID_ESCAPE_SCARD_CTL_CODE: u64 = 3500;
#[cfg(target_os = "linux")]
const IOCTL_CCID_ESCAPE_SCARD_CTL_CODE: u64 = pcsc::ctl_code(1);
#[cfg(windows)]
const IOCTL_CCID_ESCAPE_SCARD_CTL_CODE: u64 = pcsc::ctl_code(3500);

fn device_error(operation: &'static str, code: i32) -> Error {
    Error::DeviceOperationFailed { operation, code }
}

pub(crate) struct Acr122PcscDriver {
    backend: Arc<dyn PcscBackend>,
}

impl Acr122PcscDriver {
    pub(crate) fn new() -> Self {
        Self {
            backend: Arc::new(SystemPcscBackend),
        }
    }

    #[cfg(test)]
    fn with_backend(backend: Arc<dyn PcscBackend>) -> Self {
        Self { backend }
    }
}

impl Driver for Acr122PcscDriver {
    fn name(&self) -> &str {
        DRIVER_NAME
    }

    fn scan_type(&self) -> ScanType {
        ScanType::NotIntrusive
    }

    fn scan(&self, _context: &Context) -> Result<Vec<ConnectionString>, Error> {
        super::pcsc::scan_matching_readers(self.backend.as_ref(), DRIVER_NAME, ReaderFilter::Acr122)
    }

    fn open(
        &self,
        _context: &Context,
        connstring: &ConnectionString,
    ) -> Result<Box<dyn OpenedDevice>, Error> {
        let (reader_name, resolved_connstring) = resolve_reader(
            self.backend.as_ref(),
            connstring,
            DRIVER_NAME,
            ReaderFilter::Acr122,
        )?;

        let card = self
            .backend
            .connect(&reader_name, PcscShareMode::Exclusive, PcscProtocols::ANY)
            .or_else(|_| {
                self.backend.connect(
                    &reader_name,
                    PcscShareMode::Direct,
                    PcscProtocols::UNDEFINED,
                )
            })
            .map_err(|status| {
                Error::DriverOpenFailed(format!("PC/SC connect failed: 0x{status:08X}"))
            })?;

        let protocol = card
            .status2_owned()
            .map_err(|status| {
                Error::DriverOpenFailed(format!("PC/SC status failed: 0x{status:08X}"))
            })?
            .protocol;

        let firmware = read_firmware_string(card.as_ref(), protocol)?;
        if !acr122::is_acr122u_firmware(&firmware) {
            return Err(Error::DriverOpenFailed(format!(
                "unsupported ACR122 firmware: {firmware}"
            )));
        }

        let transport = Acr122PcscTransport::new(card, protocol);
        let device = Pn53xDevice::probe_with_profile(
            format!("{reader_name} / {firmware}"),
            resolved_connstring,
            Pn53xProfile::acr122_pcsc(),
            transport,
            250,
        )?;
        Ok(Box::new(device))
    }
}

fn read_firmware_string(
    card: &dyn PcscCard,
    protocol: Option<super::pcsc::PcscProtocol>,
) -> Result<String, Error> {
    let apdu = acr122::build_get_firmware_version_apdu()?;
    let response = exchange_apdu(card, protocol, &apdu, ACR122_PCSC_RESPONSE_LEN)?;
    let data = if response.len() >= 2
        && matches!(acr122::parse_status_words(&response[response.len() - 2..]), Some(status) if status.ok)
    {
        &response[..response.len() - 2]
    } else {
        &response[..]
    };
    let end = data
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(data.len());
    Ok(String::from_utf8_lossy(&data[..end]).into_owned())
}

fn exchange_apdu(
    card: &dyn PcscCard,
    protocol: Option<super::pcsc::PcscProtocol>,
    apdu: &[u8],
    receive_capacity: usize,
) -> Result<Vec<u8>, Error> {
    match protocol {
        None => card
            .control(IOCTL_CCID_ESCAPE_SCARD_CTL_CODE, apdu, receive_capacity)
            .map_err(|status| {
                Error::DriverOpenFailed(format!("PC/SC control failed: 0x{status:08X}"))
            }),
        Some(_) => card.transmit(apdu, receive_capacity).map_err(|status| {
            Error::DriverOpenFailed(format!("PC/SC transmit failed: 0x{status:08X}"))
        }),
    }
}

struct Acr122PcscTransport {
    card: Box<dyn PcscCard>,
    protocol: Option<super::pcsc::PcscProtocol>,
    pending: VecDeque<Vec<u8>>,
}

impl Acr122PcscTransport {
    fn new(card: Box<dyn PcscCard>, protocol: Option<super::pcsc::PcscProtocol>) -> Self {
        Self {
            card,
            protocol,
            pending: VecDeque::new(),
        }
    }

    fn exchange_direct_transmit(&self, payload: &[u8]) -> Result<Vec<u8>, Error> {
        let apdu = acr122::build_direct_transmit_apdu(payload)?;
        let mut response = match self.protocol {
            None => self
                .card
                .control(
                    IOCTL_CCID_ESCAPE_SCARD_CTL_CODE,
                    &apdu,
                    ACR122_PCSC_RESPONSE_LEN,
                )
                .map_err(|status| device_error("acr122_pcsc_control", status))?,
            Some(_) => self
                .card
                .transmit(&apdu, ACR122_PCSC_RESPONSE_LEN)
                .map_err(|status| device_error("acr122_pcsc_transmit", status))?,
        };

        if matches!(self.protocol, Some(super::pcsc::PcscProtocol::T0))
            && matches!(acr122::parse_status_words(&response), Some(status) if status.has_more_data)
        {
            let status = acr122::parse_status_words(&response).expect("checked above");
            let follow_up = acr122::build_get_additional_data_apdu(status.more_data_length)?;
            response = self
                .card
                .transmit(&follow_up, ACR122_PCSC_RESPONSE_LEN)
                .map_err(|status| device_error("acr122_pcsc_get_additional_data", status))?;
        }

        Ok(response)
    }
}

impl Pn53xTransport for Acr122PcscTransport {
    fn send(&mut self, payload: &[u8], _timeout_ms: i32) -> Result<(), Error> {
        let host_payload = payload_from_host_frame(payload)?;
        let command = *host_payload
            .first()
            .ok_or_else(|| device_error("acr122_pcsc_send", NFC_EIO))?;
        let response = self.exchange_direct_transmit(&host_payload)?;
        if response.len() < 4 {
            return Err(device_error("acr122_pcsc_receive", NFC_EIO));
        }

        let status = acr122::parse_status_words(&response[response.len() - 2..])
            .ok_or_else(|| device_error("acr122_pcsc_receive", NFC_EIO))?;
        if !status.ok {
            return Err(device_error("acr122_pcsc_receive", NFC_EIO));
        }

        let frame = build_response_frame(command, &response[2..response.len() - 2])?;
        self.pending.clear();
        self.pending.push_back(PN53X_ACK_FRAME.to_vec());
        self.pending.push_back(frame);
        Ok(())
    }

    fn receive(&mut self, buffer: &mut [u8], _timeout_ms: i32) -> Result<usize, Error> {
        let payload = self
            .pending
            .pop_front()
            .ok_or_else(|| device_error("acr122_pcsc_pending_receive", NFC_EIO))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::pcsc::{FakeCardState, FakePcscBackend, PcscCardStatus, PcscProtocol};

    #[test]
    fn firmware_probe_accepts_acr122u_responses() {
        let mut state = FakeCardState::default();
        state.status_responses.push_back(Ok(PcscCardStatus {
            present: true,
            atr: Vec::new(),
            protocol: Some(PcscProtocol::T1),
        }));
        state
            .transmit_responses
            .push_back(Ok(b"ACR122U203\x90\x00".to_vec()));
        state
            .transmit_responses
            .push_back(Ok(vec![0x00, 0x00, 0x32, 0x01, 0x06, 0x07, 0x90, 0x00]));
        let backend = Arc::new(
            FakePcscBackend::default().with_reader("ACS ACR122U PICC Interface 00 00", state),
        );
        let driver = Acr122PcscDriver::with_backend(backend);
        let context = Context::new();
        let connstring =
            ConnectionString::new("acr122_pcsc:ACS ACR122U PICC Interface 00 00").unwrap();

        let device = driver.open(&context, &connstring).unwrap();
        assert!(device.name().contains("ACR122U203"));
    }

    #[test]
    fn transport_builds_pending_ack_and_response_frames() {
        let mut state = FakeCardState::default();
        state
            .transmit_responses
            .push_back(Ok(vec![0x00, 0x00, 0x90, 0x00]));
        let card = Box::new(crate::native::pcsc::FakePcscCard::new(state));
        let mut transport = Acr122PcscTransport::new(card, Some(PcscProtocol::T1));

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
    fn driver_routes_shared_pn53x_commands_through_direct_transmit() {
        let mut state = FakeCardState::default();
        state.status_responses.push_back(Ok(PcscCardStatus {
            present: true,
            atr: Vec::new(),
            protocol: Some(PcscProtocol::T1),
        }));
        state
            .transmit_responses
            .push_back(Ok(b"ACR122U203\x90\x00".to_vec()));
        state
            .transmit_responses
            .push_back(Ok(vec![0x00, 0x00, 0x32, 0x01, 0x06, 0x07, 0x90, 0x00]));
        state.transmit_responses.push_back(Ok(vec![
            0x00, 0x00, 0x01, 0x01, 0x04, 0x00, 0x08, 0x04, 0xde, 0xad, 0xbe, 0xef, 0x05, 0x75,
            0x77, 0x81, 0x02, 0x90, 0x00,
        ]));
        state
            .transmit_responses
            .push_back(Ok(vec![0x00, 0x00, 0x00, 0x90, 0x00, 0x90, 0x00]));
        state.transmit_responses.push_back(Ok(vec![
            0x00, 0x00, 0x00, 0x01, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
            0x22, 0x33, 0x44, 0x55, 0x66, 0xaa, 0xbb, 0x90, 0x00,
        ]));
        state
            .transmit_responses
            .push_back(Ok(vec![0x00, 0x00, 0x00, 0x90, 0x00]));

        let backend = Arc::new(
            FakePcscBackend::default().with_reader("ACS ACR122U PICC Interface 00 00", state),
        );
        let driver = Acr122PcscDriver::with_backend(backend);
        let context = Context::new();
        let connstring =
            ConnectionString::new("acr122_pcsc:ACS ACR122U PICC Interface 00 00").unwrap();

        let mut device = driver.open(&context, &connstring).unwrap();
        let target = device
            .select_passive_target(
                crate::rust_api::Modulation {
                    modulation_type: crate::rust_api::ModulationType::Iso14443A,
                    baud_rate: crate::rust_api::BaudRate::Br106,
                },
                None,
            )
            .unwrap()
            .unwrap();
        assert!(matches!(
            target.info,
            crate::rust_api::TargetInfo::Iso14443A { .. }
        ));

        let mut rx = [0u8; 8];
        let len = device
            .transceive_bytes(&[0x30, 0x04], &mut rx, 250)
            .unwrap();
        assert_eq!(len, 2);
        assert_eq!(&rx[..len], &[0x90, 0x00]);

        let dep = device
            .select_dep_target(
                crate::rust_api::DepMode::Passive,
                crate::rust_api::BaudRate::Br106,
                None,
                250,
            )
            .unwrap()
            .unwrap();
        assert!(matches!(dep.info, crate::rust_api::TargetInfo::Dep(_)));
        device.deselect_target().unwrap();
    }

    #[test]
    fn timed_bit_flow_uses_shared_runtime_over_t0_follow_up() {
        let mut state = FakeCardState::default();
        state.status_responses.push_back(Ok(PcscCardStatus {
            present: true,
            atr: Vec::new(),
            protocol: Some(PcscProtocol::T0),
        }));
        state
            .transmit_responses
            .push_back(Ok(b"ACR122U203\x90\x00".to_vec()));
        state
            .transmit_responses
            .push_back(Ok(vec![0x00, 0x00, 0x32, 0x01, 0x06, 0x07, 0x90, 0x00]));
        state
            .transmit_responses
            .push_back(Ok(vec![0x00, 0x00, 0x90, 0x00]));
        state
            .transmit_responses
            .push_back(Ok(vec![0x00, 0x00, 0x90, 0x00]));
        state.transmit_responses.push_back(Ok(vec![0x61, 0x01]));
        state
            .transmit_responses
            .push_back(Ok(vec![0x00, 0x00, 0x02, 0x90, 0x00]));
        state
            .transmit_responses
            .push_back(Ok(vec![0x00, 0x00, 0x04, 0x00, 0x00, 0x90, 0x00]));
        state
            .transmit_responses
            .push_back(Ok(vec![0x00, 0x00, 0x00, 0x90, 0x00]));
        state
            .transmit_responses
            .push_back(Ok(vec![0x00, 0x00, 0xf0, 0x00, 0x90, 0x00]));

        let backend = Arc::new(
            FakePcscBackend::default().with_reader("ACS ACR122U PICC Interface 00 00", state),
        );
        let driver = Acr122PcscDriver::with_backend(backend);
        let context = Context::new();
        let connstring =
            ConnectionString::new("acr122_pcsc:ACS ACR122U PICC Interface 00 00").unwrap();

        let mut device = driver.open(&context, &connstring).unwrap();
        device
            .set_property_bool(crate::rust_api::Property::EasyFraming, false)
            .unwrap();
        device
            .set_property_bool(crate::rust_api::Property::HandleCrc, false)
            .unwrap();

        let mut rx = [0u8; 8];
        let (bits, cycles) = device
            .transceive_bits_timed(&[0x26], 7, None, &mut rx, None)
            .unwrap();
        assert_eq!(bits, 16);
        assert_eq!(&rx[..2], &[0x04, 0x00]);
        assert_eq!(cycles, 3506);
    }
}
