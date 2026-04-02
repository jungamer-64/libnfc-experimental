use super::acr122;
#[cfg(feature = "pcsc_helper")]
use crate::pcsc as platform_pcsc;
use proximate_driver::{
    BaudRate, ConnectionString, Context, DeviceCaps, Driver, Error, Mode, Modulation,
    ModulationType, OpenedDevice, Property, ScanType, Target, TargetInfo, decode_connstring,
    device_error_message,
};
#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;
#[cfg(test)]
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

const NFC_SUCCESS: i32 = 0;
const NFC_EIO: i32 = -1;
const NFC_EINVARG: i32 = -2;
const NFC_EDEVNOTSUPP: i32 = -3;
const NFC_ENOTSUCHDEV: i32 = -4;
const NFC_ESOFT: i32 = -80;
const NFC_ECHIP: i32 = -90;

const PCSC_DRIVER_NAME: &str = "pcsc";

const ICC_TYPE_UNKNOWN: u8 = 0;
const ICC_TYPE_14443A: u8 = 5;
const ICC_TYPE_14443B: u8 = 6;

const PCSC_SUPPORTED_BAUD_RATES: &[BaudRate] = &[BaudRate::Br106, BaudRate::Br424];
const PCSC_SUPPORTED_MODULATIONS: &[ModulationType] =
    &[ModulationType::Iso14443A, ModulationType::Iso14443B];

fn device_error(operation: &'static str, code: i32) -> Error {
    Error::DeviceOperationFailed { operation, code }
}

fn invalid_connection(message: impl Into<String>) -> Error {
    Error::InvalidConnectionString(message.into())
}

fn pcsc_error_message(code: i32) -> Option<&'static str> {
    #[cfg(feature = "pcsc_helper")]
    {
        platform_pcsc::error_message(code)
    }

    #[cfg(not(feature = "pcsc_helper"))]
    {
        let _ = code;
        None
    }
}

fn stringify_pcsc_error(code: i32) -> String {
    pcsc_error_message(code)
        .map(str::to_string)
        .unwrap_or_else(|| format!("Unknown error: 0x{:08X}", code as u32))
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum PcscShareMode {
    Exclusive,
    Shared,
    Direct,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) enum PcscDisposition {
    LeaveCard,
    ResetCard,
    UnpowerCard,
    EjectCard,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum PcscProtocol {
    T0,
    T1,
    Raw,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct PcscProtocols(u8);

impl PcscProtocols {
    pub(super) const UNDEFINED: Self = Self(0);
    pub(super) const T0: Self = Self(1 << 0);
    pub(super) const T1: Self = Self(1 << 1);
    pub(super) const RAW: Self = Self(1 << 2);
    pub(super) const ANY: Self = Self(Self::T0.0 | Self::T1.0);

    const fn contains(self, protocol: PcscProtocol) -> bool {
        let mask = match protocol {
            PcscProtocol::T0 => Self::T0.0,
            PcscProtocol::T1 => Self::T1.0,
            PcscProtocol::Raw => Self::RAW.0,
        };
        self.0 & mask != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum PcscAttribute {
    VendorName,
    VendorIfdType,
    VendorIfdVersion,
    VendorIfdSerialNo,
    IccTypePerAtr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PcscCardStatus {
    pub present: bool,
    pub atr: Vec<u8>,
    pub protocol: Option<PcscProtocol>,
}

pub(super) trait PcscCard: Send {
    fn reconnect(
        &mut self,
        share_mode: PcscShareMode,
        preferred_protocols: PcscProtocols,
        disposition: PcscDisposition,
    ) -> Result<(), i32>;

    fn status2_owned(&self) -> Result<PcscCardStatus, i32>;

    fn get_attribute_owned(&self, attribute: PcscAttribute) -> Result<Vec<u8>, i32>;

    fn transmit(&self, send_buffer: &[u8], receive_capacity: usize) -> Result<Vec<u8>, i32>;

    fn control(
        &self,
        control_code: u64,
        send_buffer: &[u8],
        receive_capacity: usize,
    ) -> Result<Vec<u8>, i32>;
}

pub(super) trait PcscBackend: Send + Sync {
    fn list_readers_owned(&self) -> Result<Vec<String>, i32>;

    fn connect(
        &self,
        reader: &str,
        share_mode: PcscShareMode,
        preferred_protocols: PcscProtocols,
    ) -> Result<Box<dyn PcscCard>, i32>;
}

#[cfg(feature = "pcsc_helper")]
fn map_share_mode(value: PcscShareMode) -> platform_pcsc::ShareMode {
    match value {
        PcscShareMode::Exclusive => platform_pcsc::ShareMode::Exclusive,
        PcscShareMode::Shared => platform_pcsc::ShareMode::Shared,
        PcscShareMode::Direct => platform_pcsc::ShareMode::Direct,
    }
}

#[cfg(feature = "pcsc_helper")]
fn map_protocols(value: PcscProtocols) -> platform_pcsc::Protocols {
    let mut protocols = platform_pcsc::Protocols::UNDEFINED;
    if value.contains(PcscProtocol::T0) {
        protocols = platform_pcsc::Protocols(protocols.0 | platform_pcsc::Protocols::T0.0);
    }
    if value.contains(PcscProtocol::T1) {
        protocols = platform_pcsc::Protocols(protocols.0 | platform_pcsc::Protocols::T1.0);
    }
    if value.contains(PcscProtocol::Raw) {
        protocols = platform_pcsc::Protocols(protocols.0 | platform_pcsc::Protocols::RAW.0);
    }
    protocols
}

#[cfg(feature = "pcsc_helper")]
fn map_disposition(value: PcscDisposition) -> platform_pcsc::Disposition {
    match value {
        PcscDisposition::LeaveCard => platform_pcsc::Disposition::LeaveCard,
        PcscDisposition::ResetCard => platform_pcsc::Disposition::ResetCard,
        PcscDisposition::UnpowerCard => platform_pcsc::Disposition::UnpowerCard,
        PcscDisposition::EjectCard => platform_pcsc::Disposition::EjectCard,
    }
}

#[cfg(feature = "pcsc_helper")]
fn map_protocol(value: platform_pcsc::Protocol) -> PcscProtocol {
    match value {
        platform_pcsc::Protocol::T0 => PcscProtocol::T0,
        platform_pcsc::Protocol::T1 => PcscProtocol::T1,
        platform_pcsc::Protocol::Raw => PcscProtocol::Raw,
    }
}

#[cfg(feature = "pcsc_helper")]
fn map_attribute(value: PcscAttribute) -> platform_pcsc::Attribute {
    match value {
        PcscAttribute::VendorName => platform_pcsc::Attribute::VendorName,
        PcscAttribute::VendorIfdType => platform_pcsc::Attribute::VendorIfdType,
        PcscAttribute::VendorIfdVersion => platform_pcsc::Attribute::VendorIfdVersion,
        PcscAttribute::VendorIfdSerialNo => platform_pcsc::Attribute::VendorIfdSerialNo,
        PcscAttribute::IccTypePerAtr => platform_pcsc::Attribute::IccTypePerAtr,
    }
}

#[cfg(feature = "pcsc_helper")]
pub(super) struct SystemPcscBackend;

#[cfg(feature = "pcsc_helper")]
struct SystemPcscCard {
    inner: Box<dyn platform_pcsc::Card>,
}

#[cfg(feature = "pcsc_helper")]
impl PcscCard for SystemPcscCard {
    fn reconnect(
        &mut self,
        share_mode: PcscShareMode,
        preferred_protocols: PcscProtocols,
        disposition: PcscDisposition,
    ) -> Result<(), i32> {
        self.inner.reconnect(
            map_share_mode(share_mode),
            map_protocols(preferred_protocols),
            map_disposition(disposition),
        )
    }

    fn status2_owned(&self) -> Result<PcscCardStatus, i32> {
        self.inner.status2_owned().map(|status| PcscCardStatus {
            present: status.present,
            atr: status.atr,
            protocol: status.protocol.map(map_protocol),
        })
    }

    fn get_attribute_owned(&self, attribute: PcscAttribute) -> Result<Vec<u8>, i32> {
        self.inner.get_attribute_owned(map_attribute(attribute))
    }

    fn transmit(&self, send_buffer: &[u8], receive_capacity: usize) -> Result<Vec<u8>, i32> {
        self.inner.transmit(send_buffer, receive_capacity)
    }

    fn control(
        &self,
        control_code: u64,
        send_buffer: &[u8],
        receive_capacity: usize,
    ) -> Result<Vec<u8>, i32> {
        self.inner
            .control(control_code, send_buffer, receive_capacity)
    }
}

#[cfg(feature = "pcsc_helper")]
impl PcscBackend for SystemPcscBackend {
    fn list_readers_owned(&self) -> Result<Vec<String>, i32> {
        platform_pcsc::Backend::list_readers_owned(&platform_pcsc::SystemBackend)
    }

    fn connect(
        &self,
        reader: &str,
        share_mode: PcscShareMode,
        preferred_protocols: PcscProtocols,
    ) -> Result<Box<dyn PcscCard>, i32> {
        let card = platform_pcsc::Backend::connect(
            &platform_pcsc::SystemBackend,
            reader,
            map_share_mode(share_mode),
            map_protocols(preferred_protocols),
        )?;
        Ok(Box::new(SystemPcscCard { inner: card }))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReaderFilter {
    Generic,
    Acr122,
}

fn reader_matches(filter: ReaderFilter, reader: &str) -> bool {
    match filter {
        ReaderFilter::Generic => !acr122::is_pcsc_reader_name(reader),
        ReaderFilter::Acr122 => acr122::is_pcsc_reader_name(reader),
    }
}

pub(super) fn scan_matching_readers(
    backend: &dyn PcscBackend,
    driver_name: &str,
    filter: ReaderFilter,
) -> Result<Vec<ConnectionString>, Error> {
    let readers = match backend.list_readers_owned() {
        Ok(readers) => readers,
        Err(_) => return Ok(Vec::new()),
    };
    readers
        .into_iter()
        .filter(|reader| reader_matches(filter, reader))
        .map(|reader| ConnectionString::new(format!("{driver_name}:{reader}")))
        .collect()
}

fn parse_reader_index(value: &str) -> Option<usize> {
    if value.is_empty() || value.len() > 4 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse::<usize>().ok()
}

pub(super) fn resolve_reader(
    backend: &dyn PcscBackend,
    connstring: &ConnectionString,
    driver_name: &str,
    filter: ReaderFilter,
) -> Result<(String, ConnectionString), Error> {
    let decoded = decode_connstring(connstring, driver_name, "pcsc")?;
    if decoded.match_depth < 1 {
        return Err(invalid_connection(format!(
            "{driver_name} connection string does not match"
        )));
    }

    if decoded.match_depth == 1 {
        let devices = scan_matching_readers(backend, driver_name, filter)?;
        let Some(resolved) = devices.into_iter().next() else {
            return Err(device_error("pcsc_scan", NFC_ENOTSUCHDEV));
        };
        let resolved_decoded = decode_connstring(&resolved, driver_name, "pcsc")?;
        let reader = resolved_decoded
            .param1
            .ok_or_else(|| invalid_connection("resolved reader name is missing"))?;
        return Ok((reader, resolved));
    }

    let requested = decoded
        .param1
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid_connection("reader name is missing"))?;
    if let Some(index) = parse_reader_index(&requested) {
        let devices = scan_matching_readers(backend, driver_name, filter)?;
        let Some(resolved) = devices.into_iter().nth(index) else {
            return Err(device_error("pcsc_scan", NFC_ENOTSUCHDEV));
        };
        let resolved_decoded = decode_connstring(&resolved, driver_name, "pcsc")?;
        let reader = resolved_decoded
            .param1
            .ok_or_else(|| invalid_connection("resolved reader name is missing"))?;
        return Ok((reader, resolved));
    }

    if !reader_matches(filter, &requested) {
        return Err(device_error("pcsc_open", NFC_ENOTSUCHDEV));
    }

    Ok((
        requested.clone(),
        ConnectionString::new(format!("{driver_name}:{requested}"))?,
    ))
}

fn attr_to_string(value: &[u8]) -> Option<String> {
    let end = value
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(value.len());
    if end == 0 {
        None
    } else {
        Some(String::from_utf8_lossy(&value[..end]).into_owned())
    }
}

fn is_feitian_reader(name: &str) -> bool {
    let lowercase = name.to_ascii_lowercase();
    lowercase.contains("feitian")
}

fn command_response_data<'a>(
    response: &'a [u8],
    operation: &'static str,
) -> Result<&'a [u8], Error> {
    if response.len() < 2 {
        return Err(device_error(operation, NFC_EDEVNOTSUPP));
    }
    Ok(&response[..response.len() - 2])
}

fn icc_type_matches(icc_type: u8, expected_type: u8) -> bool {
    icc_type == ICC_TYPE_UNKNOWN || icc_type == expected_type
}

fn iso14443a_uid_length_valid(uid_length: usize) -> bool {
    matches!(uid_length, 0 | 4 | 7 | 10)
}

fn iso14443a_atr_valid(atr: &[u8]) -> bool {
    atr.len() >= 5
        && atr[0] == 0x3B
        && atr[1] == (0x80 | (atr.len() as u8 - 5))
        && atr[2] == 0x80
        && atr[3] == 0x01
}

fn iso14443b_uid_length_valid(uid_length: usize) -> bool {
    matches!(uid_length, 0 | 8)
}

fn iso14443b_atr_valid(atr: &[u8]) -> bool {
    atr.len() == 13 && atr[0] == 0x3B && atr[1] == (0x80 | 0x08) && atr[2] == 0x80 && atr[3] == 0x01
}

pub(super) struct PcscDriver {
    driver_name: &'static str,
    filter: ReaderFilter,
    backend: Arc<dyn PcscBackend>,
}

impl PcscDriver {
    pub(super) fn new() -> Self {
        Self {
            driver_name: PCSC_DRIVER_NAME,
            filter: ReaderFilter::Generic,
            backend: Arc::new(SystemPcscBackend),
        }
    }

    #[cfg(test)]
    pub(super) fn with_backend(backend: Arc<dyn PcscBackend>) -> Self {
        Self {
            driver_name: PCSC_DRIVER_NAME,
            filter: ReaderFilter::Generic,
            backend,
        }
    }
}

impl Driver for PcscDriver {
    fn name(&self) -> &str {
        self.driver_name
    }

    fn scan_type(&self) -> ScanType {
        ScanType::NotIntrusive
    }

    fn scan(&self, _context: &Context) -> Result<Vec<ConnectionString>, Error> {
        scan_matching_readers(self.backend.as_ref(), self.driver_name, self.filter)
    }

    fn open(
        &self,
        _context: &Context,
        connstring: &ConnectionString,
    ) -> Result<Box<dyn OpenedDevice>, Error> {
        let (reader_name, resolved_connstring) = resolve_reader(
            self.backend.as_ref(),
            connstring,
            self.driver_name,
            self.filter,
        )?;
        let card = self
            .backend
            .connect(&reader_name, PcscShareMode::Direct, PcscProtocols::T0)
            .map_err(|status| device_error("pcsc_connect", status))?;
        Ok(Box::new(PcscDevice::new(
            reader_name,
            resolved_connstring,
            card,
            PcscShareMode::Direct,
            PcscProtocols::T0,
        )))
    }
}

pub(super) struct PcscDevice {
    name: String,
    connstring: ConnectionString,
    card: Box<dyn PcscCard>,
    share_mode: PcscShareMode,
    preferred_protocols: PcscProtocols,
    last_error: i32,
    last_pcsc_error: Option<i32>,
}

impl fmt::Debug for PcscDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PcscDevice")
            .field("name", &self.name)
            .field("connstring", &self.connstring)
            .field("share_mode", &self.share_mode)
            .field("preferred_protocols", &self.preferred_protocols.0)
            .field("last_error", &self.last_error)
            .field("last_pcsc_error", &self.last_pcsc_error)
            .finish_non_exhaustive()
    }
}

impl PcscDevice {
    pub(super) fn new(
        name: String,
        connstring: ConnectionString,
        card: Box<dyn PcscCard>,
        share_mode: PcscShareMode,
        preferred_protocols: PcscProtocols,
    ) -> Self {
        Self {
            name,
            connstring,
            card,
            share_mode,
            preferred_protocols,
            last_error: 0,
            last_pcsc_error: None,
        }
    }

    fn succeed<T>(&mut self, value: T) -> Result<T, Error> {
        self.last_error = NFC_SUCCESS;
        self.last_pcsc_error = None;
        Ok(value)
    }

    fn fail<T>(&mut self, operation: &'static str, code: i32) -> Result<T, Error> {
        self.last_error = code;
        self.last_pcsc_error = None;
        Err(device_error(operation, code))
    }

    fn status(&mut self) -> Result<PcscCardStatus, Error> {
        self.card.status2_owned().map_err(|status| {
            self.last_error = NFC_EIO;
            self.last_pcsc_error = Some(status);
            device_error("pcsc_status", NFC_EIO)
        })
    }

    fn reconnect(
        &mut self,
        share_mode: PcscShareMode,
        preferred_protocols: PcscProtocols,
        disposition: PcscDisposition,
    ) -> Result<(), Error> {
        self.card
            .reconnect(share_mode, preferred_protocols, disposition)
            .map_err(|status| {
                self.last_error = NFC_EIO;
                self.last_pcsc_error = Some(status);
                device_error("pcsc_reconnect", NFC_EIO)
            })?;
        self.share_mode = share_mode;
        self.preferred_protocols = preferred_protocols;
        self.last_error = NFC_SUCCESS;
        self.last_pcsc_error = None;
        Ok(())
    }

    fn attribute(&mut self, attribute: PcscAttribute) -> Result<Vec<u8>, Error> {
        self.card.get_attribute_owned(attribute).map_err(|status| {
            self.last_error = NFC_EIO;
            self.last_pcsc_error = Some(status);
            device_error("pcsc_get_attribute", NFC_EIO)
        })
    }

    fn transmit_card(
        &mut self,
        tx: &[u8],
        rx_capacity: usize,
        operation: &'static str,
    ) -> Result<Vec<u8>, Error> {
        self.card.transmit(tx, rx_capacity).map_err(|status| {
            self.last_error = NFC_EIO;
            self.last_pcsc_error = Some(status);
            device_error(operation, NFC_EIO)
        })
    }

    fn get_atqa(&mut self) -> Result<Vec<u8>, Error> {
        let response = self.transmit_card(&[0xFF, 0xCA, 0x03, 0x00, 0x00], 258, "pcsc_transmit")?;
        let data = command_response_data(&response, "pcsc_get_atqa")?;
        if data.len() > 2 {
            return self.fail("pcsc_get_atqa", NFC_ESOFT);
        }
        self.succeed(data.to_vec())
    }

    fn get_ats(&mut self) -> Result<Vec<u8>, Error> {
        let response = self.transmit_card(&[0xFF, 0xCA, 0x01, 0x00, 0x00], 258, "pcsc_transmit")?;
        let data = command_response_data(&response, "pcsc_get_ats")?;
        if data.is_empty() {
            return self.fail("pcsc_get_ats", NFC_EDEVNOTSUPP);
        }
        self.succeed(data[1..].to_vec())
    }

    fn get_sak(&mut self) -> Result<Vec<u8>, Error> {
        let response = self.transmit_card(&[0xFF, 0xCA, 0x02, 0x00, 0x00], 258, "pcsc_transmit")?;
        let data = command_response_data(&response, "pcsc_get_sak")?;
        if data.is_empty() {
            return self.fail("pcsc_get_sak", NFC_EDEVNOTSUPP);
        }
        self.succeed(data.to_vec())
    }

    fn get_uid(&mut self) -> Result<Vec<u8>, Error> {
        let response = self.transmit_card(&[0xFF, 0xCA, 0x00, 0x00, 0x00], 258, "pcsc_transmit")?;
        let data = command_response_data(&response, "pcsc_get_uid")?;
        self.succeed(data.to_vec())
    }

    fn get_icc_type(&mut self) -> Result<u8, Error> {
        let attr = self.attribute(PcscAttribute::IccTypePerAtr)?;
        self.succeed(attr.first().copied().unwrap_or(ICC_TYPE_UNKNOWN))
    }

    fn enrich_iso14443a_for_feitian(
        &mut self,
        atqa: &mut [u8; 2],
        sak: &mut u8,
        ats: &mut Vec<u8>,
    ) -> Result<(), Error> {
        match self.get_atqa() {
            Ok(value) if value.len() >= 2 => {
                atqa.copy_from_slice(&value[..2]);
                if value[0] != 0x00 && value[0] != 0x03 {
                    atqa.swap(0, 1);
                }
            }
            Err(error) if error.device_code() == Some(NFC_EDEVNOTSUPP) => {}
            Err(error) => return Err(error),
            _ => {}
        }

        match self.get_sak() {
            Ok(value) if !value.is_empty() => *sak = value[0],
            Err(error) if error.device_code() == Some(NFC_EDEVNOTSUPP) => {}
            Err(error) => return Err(error),
            _ => {}
        }

        match self.get_ats() {
            Ok(value) if !value.is_empty() => *ats = value,
            Err(error) if error.device_code() == Some(NFC_EDEVNOTSUPP) => {}
            Err(error) => return Err(error),
            _ => {}
        }

        self.last_error = NFC_SUCCESS;
        self.last_pcsc_error = None;
        Ok(())
    }

    fn fill_iso14443a_target(
        &mut self,
        icc_type: u8,
        atr: &[u8],
        uid: &[u8],
    ) -> Result<Target, Error> {
        if !icc_type_matches(icc_type, ICC_TYPE_14443A)
            || !iso14443a_uid_length_valid(uid.len())
            || !iso14443a_atr_valid(atr)
        {
            return self.fail("pcsc_fill_iso14443a_target", NFC_EINVARG);
        }

        let mut atqa = [0u8; 2];
        let mut sak = 0x20;
        let mut ats = vec![0x75, 0x77, 0x81, 0x02];
        ats.extend_from_slice(&atr[4..]);

        if is_feitian_reader(&self.name) {
            ats.clear();
            self.enrich_iso14443a_for_feitian(&mut atqa, &mut sak, &mut ats)?;
        }

        self.succeed(Target {
            modulation: Modulation {
                modulation_type: ModulationType::Iso14443A,
                baud_rate: BaudRate::Br106,
            },
            info: TargetInfo::Iso14443A {
                atqa,
                sak,
                uid: uid.to_vec(),
                ats,
            },
        })
    }

    fn fill_iso14443b_target(
        &mut self,
        icc_type: u8,
        atr: &[u8],
        uid_len: usize,
    ) -> Result<Target, Error> {
        if !icc_type_matches(icc_type, ICC_TYPE_14443B)
            || !iso14443b_uid_length_valid(uid_len)
            || !iso14443b_atr_valid(atr)
        {
            return self.fail("pcsc_fill_iso14443b_target", NFC_EINVARG);
        }

        let mut application_data = [0u8; 4];
        application_data.copy_from_slice(&atr[4..8]);
        let mut protocol_info = [0u8; 3];
        protocol_info.copy_from_slice(&atr[8..11]);
        protocol_info[1] = 0x01;

        self.succeed(Target {
            modulation: Modulation {
                modulation_type: ModulationType::Iso14443B,
                baud_rate: BaudRate::Br106,
            },
            info: TargetInfo::Iso14443B {
                pupi: [0u8; 4],
                application_data,
                protocol_info,
                card_identifier: 0,
            },
        })
    }

    fn props_to_target(
        &mut self,
        icc_type: u8,
        atr: &[u8],
        uid: &[u8],
        modulation_type: ModulationType,
    ) -> Result<Target, Error> {
        match modulation_type {
            ModulationType::Iso14443A => self.fill_iso14443a_target(icc_type, atr, uid),
            ModulationType::Iso14443B => self.fill_iso14443b_target(icc_type, atr, uid.len()),
            _ => self.fail("pcsc_props_to_target", NFC_EINVARG),
        }
    }

    fn transmit_bytes_internal(&mut self, tx: &[u8], rx: &mut [u8]) -> Result<usize, Error> {
        let receive_capacity = if is_feitian_reader(&self.name) {
            if rx.len() == 1 {
                2
            } else {
                rx.len().saturating_add(2)
            }
        } else {
            rx.len()
        };
        let response = self.transmit_card(tx, receive_capacity, "pcsc_transmit")?;
        if response.len() > rx.len() {
            return self.fail("pcsc_transceive_bytes", NFC_ECHIP);
        }
        rx[..response.len()].copy_from_slice(&response);
        self.succeed(response.len())
    }

    fn feitian_execute_apdu(&mut self, apdu: &[u8], rx: &mut [u8]) -> Result<usize, Error> {
        self.transmit_bytes_internal(apdu, rx)
    }

    fn feitian_handle_read(&mut self, tx: &[u8], rx: &mut [u8]) -> Result<usize, Error> {
        if tx.len() < 2 {
            return self.fail("feitian_read", NFC_EINVARG);
        }
        let apdu = [0xFF, 0xB0, 0x00, tx[1], 0x10];
        self.feitian_execute_apdu(&apdu, rx)
    }

    fn feitian_handle_write(&mut self, tx: &[u8], rx: &mut [u8]) -> Result<usize, Error> {
        if tx.len() < 2 || tx.len() - 2 > 251 {
            return self.fail("feitian_write", NFC_EINVARG);
        }
        let mut apdu = Vec::with_capacity(tx.len() + 3);
        apdu.extend_from_slice(&[0xFF, 0xD6, 0x00, tx[1], (tx.len() - 2) as u8]);
        apdu.extend_from_slice(&tx[2..]);
        self.feitian_execute_apdu(&apdu, rx)
    }

    fn feitian_handle_auth(
        &mut self,
        command: u8,
        tx: &[u8],
        rx: &mut [u8],
    ) -> Result<usize, Error> {
        if tx.len() < 8 {
            return self.fail("feitian_auth", NFC_EINVARG);
        }

        let mut load_key = [0u8; 11];
        load_key[..5].copy_from_slice(&[0xFF, 0x82, 0x00, 0x01, 0x06]);
        load_key[5..11].copy_from_slice(&tx[2..8]);
        let mut discard = [0u8; 258];
        let _ = self.transmit_bytes_internal(&load_key, &mut discard)?;
        thread::sleep(Duration::from_millis(500));

        let apdu = [
            0xFF, 0x86, 0x00, 0x00, 0x05, 0x01, 0x00, tx[1], command, 0x01,
        ];
        self.feitian_execute_apdu(&apdu, rx)
    }

    fn feitian_handle_value_operation(
        &mut self,
        command: u8,
        tx: &[u8],
        rx: &mut [u8],
    ) -> Result<usize, Error> {
        if tx.len() < 2 {
            return self.fail("feitian_value_operation", NFC_EINVARG);
        }

        let payload_len = tx.len() - 2;
        if payload_len > 251 {
            return self.fail("feitian_value_operation", NFC_ECHIP);
        }

        let mut apdu = Vec::with_capacity(tx.len() + 3);
        apdu.extend_from_slice(&[
            0xFF,
            if command == 0xC2 { 0xD8 } else { 0xD7 },
            0x00,
            tx[1],
            if command == 0xC2 {
                payload_len as u8
            } else {
                0x05
            },
        ]);
        apdu.extend_from_slice(&tx[2..]);
        self.feitian_execute_apdu(&apdu, rx)
    }

    fn feitian_route_command(&mut self, tx: &[u8], rx: &mut [u8]) -> Result<usize, Error> {
        let Some(&command) = tx.first() else {
            return self.fail("feitian_route_command", NFC_EINVARG);
        };
        match command {
            0x30 => self.feitian_handle_read(tx, rx),
            0xA0 | 0xA2 => self.feitian_handle_write(tx, rx),
            0x60 | 0x61 | 0x1A => self.feitian_handle_auth(command, tx, rx),
            0xC0 | 0xC1 | 0xC2 => self.feitian_handle_value_operation(command, tx, rx),
            _ => self.feitian_execute_apdu(tx, rx),
        }
    }
}

impl OpenedDevice for PcscDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn connstring(&self) -> &ConnectionString {
        &self.connstring
    }

    fn caps(&self) -> DeviceCaps {
        DeviceCaps::INFO
            | DeviceCaps::SET_PROPERTY_BOOL
            | DeviceCaps::SUPPORTED_MODULATIONS
            | DeviceCaps::SUPPORTED_BAUD_RATES
            | DeviceCaps::INITIATOR_INIT
            | DeviceCaps::SELECT_PASSIVE_TARGET
            | DeviceCaps::POLL_TARGET
            | DeviceCaps::TARGET_IS_PRESENT
            | DeviceCaps::TRANSCEIVE_BYTES
    }

    fn last_error(&self) -> i32 {
        self.last_error
    }

    fn strerror(&self) -> String {
        self.last_pcsc_error
            .map(stringify_pcsc_error)
            .unwrap_or_else(|| device_error_message(self.last_error).to_string())
    }

    fn information_about(&mut self) -> Result<String, Error> {
        let model = self
            .attribute(PcscAttribute::VendorName)
            .ok()
            .and_then(|value| attr_to_string(&value));
        let version = self
            .attribute(PcscAttribute::VendorIfdVersion)
            .ok()
            .and_then(|value| attr_to_string(&value));
        let vendor = self
            .attribute(PcscAttribute::VendorIfdType)
            .ok()
            .and_then(|value| attr_to_string(&value));
        let serial = self
            .attribute(PcscAttribute::VendorIfdSerialNo)
            .ok()
            .and_then(|value| attr_to_string(&value));

        let mut message = format!(
            "{}{}{} ({})",
            model.as_deref().unwrap_or("unknown model"),
            if version.is_some() { " " } else { "" },
            version.as_deref().unwrap_or(""),
            vendor.as_deref().unwrap_or("unknown vendor")
        );
        if let Some(serial) = serial {
            message.push_str("\nserial: ");
            message.push_str(&serial);
        }
        message.push('\n');
        self.succeed(message)
    }

    fn set_property_bool(&mut self, property: Property, enable: bool) -> Result<(), Error> {
        let is_feitian = is_feitian_reader(&self.name);
        match property {
            Property::InfiniteSelect => self.succeed(()),
            Property::AutoIso14443_4 | Property::EasyFraming => {
                if enable || is_feitian {
                    self.succeed(())
                } else {
                    self.fail("pcsc_set_property_bool", NFC_EDEVNOTSUPP)
                }
            }
            Property::ForceIso14443A
            | Property::HandleCrc
            | Property::HandleParity
            | Property::ForceSpeed106 => {
                if enable {
                    self.succeed(())
                } else {
                    self.fail("pcsc_set_property_bool", NFC_EDEVNOTSUPP)
                }
            }
            Property::AcceptInvalidFrames | Property::AcceptMultipleFrames => {
                if enable {
                    self.fail("pcsc_set_property_bool", NFC_EDEVNOTSUPP)
                } else {
                    self.succeed(())
                }
            }
            Property::ActivateField => {
                if !enable {
                    self.reconnect(
                        self.share_mode,
                        self.preferred_protocols,
                        PcscDisposition::LeaveCard,
                    )?;
                }
                self.succeed(())
            }
            _ => self.fail("pcsc_set_property_bool", NFC_EDEVNOTSUPP),
        }
    }

    fn set_property_int(&mut self, _property: Property, _value: i32) -> Result<(), Error> {
        self.fail("pcsc_set_property_int", NFC_EDEVNOTSUPP)
    }

    fn supported_modulations(&mut self, mode: Mode) -> Result<Vec<ModulationType>, Error> {
        if mode == Mode::Target {
            return self.fail("pcsc_get_supported_modulation", NFC_EINVARG);
        }
        self.succeed(PCSC_SUPPORTED_MODULATIONS.to_vec())
    }

    fn supported_baud_rates(
        &mut self,
        mode: Mode,
        _modulation_type: ModulationType,
    ) -> Result<Vec<BaudRate>, Error> {
        if mode == Mode::Target {
            return self.fail("pcsc_get_supported_baud_rate", NFC_EINVARG);
        }
        self.succeed(PCSC_SUPPORTED_BAUD_RATES.to_vec())
    }

    fn initiator_init_driver(&mut self) -> Result<i32, Error> {
        self.succeed(0)
    }

    fn select_passive_target_driver(
        &mut self,
        nm: Modulation,
        _init_data: &[u8],
    ) -> Result<Option<Target>, Error> {
        if nm.baud_rate != BaudRate::Br106 && nm.baud_rate != BaudRate::Br424 {
            return self.fail("pcsc_select_passive_target", NFC_EINVARG);
        }

        let status = self.status()?;
        if !status.present {
            return self.succeed(None);
        }

        let icc_type = self.get_icc_type()?;
        let uid = self.get_uid().unwrap_or_default();
        let target = match self.props_to_target(icc_type, &status.atr, &uid, nm.modulation_type) {
            Ok(target) => target,
            Err(error) if error.device_code() == Some(NFC_EINVARG) => {
                return self.fail("pcsc_select_passive_target", NFC_EDEVNOTSUPP);
            }
            Err(error) => return Err(error),
        };

        self.reconnect(
            PcscShareMode::Shared,
            PcscProtocols::ANY,
            PcscDisposition::LeaveCard,
        )?;
        self.succeed(Some(target))
    }

    fn poll_target_driver(
        &mut self,
        modulations: &[Modulation],
        poll_nr: u8,
        period: u8,
    ) -> Result<Option<Target>, Error> {
        let delay = Duration::from_micros(period as u64 * 150_000);
        for _ in 0..poll_nr {
            for modulation in modulations {
                if let Some(target) = self.select_passive_target_driver(*modulation, &[])? {
                    return self.succeed(Some(target));
                }
            }
            thread::sleep(delay);
        }
        self.succeed(None)
    }

    fn target_is_present_driver(&mut self, target: Option<&Target>) -> Result<bool, Error> {
        let status = self.status()?;
        if !status.present {
            return self.fail("pcsc_target_is_present", NFC_ENOTSUCHDEV);
        }
        if let Some(target) = target {
            let current = self.props_to_target(
                ICC_TYPE_UNKNOWN,
                &status.atr,
                &[],
                target.modulation.modulation_type,
            )?;
            if current.modulation != target.modulation {
                return self.fail("pcsc_target_is_present", NFC_ENOTSUCHDEV);
            }
        }
        self.succeed(true)
    }

    fn transceive_bytes_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        _timeout: i32,
    ) -> Result<usize, Error> {
        if is_feitian_reader(&self.name) {
            self.feitian_route_command(tx, rx)
        } else {
            self.transmit_bytes_internal(tx, rx)
        }
    }
}

#[cfg(test)]
#[derive(Default)]
pub(super) struct FakeCardState {
    pub(super) status_responses: VecDeque<Result<PcscCardStatus, i32>>,
    pub(super) attributes: HashMap<PcscAttribute, Result<Vec<u8>, i32>>,
    pub(super) transmit_responses: VecDeque<Result<Vec<u8>, i32>>,
    pub(super) control_responses: VecDeque<Result<Vec<u8>, i32>>,
    pub(super) reconnect_calls: Vec<(PcscShareMode, PcscProtocols, PcscDisposition)>,
}

#[cfg(test)]
#[derive(Clone)]
pub(super) struct FakePcscCard {
    state: Arc<Mutex<FakeCardState>>,
}

#[cfg(test)]
impl FakePcscCard {
    pub(super) fn new(state: FakeCardState) -> Self {
        Self {
            state: Arc::new(Mutex::new(state)),
        }
    }
}

#[cfg(test)]
impl PcscCard for FakePcscCard {
    fn reconnect(
        &mut self,
        share_mode: PcscShareMode,
        preferred_protocols: PcscProtocols,
        disposition: PcscDisposition,
    ) -> Result<(), i32> {
        self.state.lock().unwrap().reconnect_calls.push((
            share_mode,
            preferred_protocols,
            disposition,
        ));
        Ok(())
    }

    fn status2_owned(&self) -> Result<PcscCardStatus, i32> {
        self.state
            .lock()
            .unwrap()
            .status_responses
            .pop_front()
            .unwrap_or(Ok(PcscCardStatus {
                present: true,
                atr: Vec::new(),
                protocol: Some(PcscProtocol::T0),
            }))
    }

    fn get_attribute_owned(&self, attribute: PcscAttribute) -> Result<Vec<u8>, i32> {
        self.state
            .lock()
            .unwrap()
            .attributes
            .get(&attribute)
            .cloned()
            .unwrap_or(Ok(Vec::new()))
    }

    fn transmit(&self, _send_buffer: &[u8], _receive_capacity: usize) -> Result<Vec<u8>, i32> {
        self.state
            .lock()
            .unwrap()
            .transmit_responses
            .pop_front()
            .unwrap_or(Ok(Vec::new()))
    }

    fn control(
        &self,
        _control_code: u64,
        _send_buffer: &[u8],
        _receive_capacity: usize,
    ) -> Result<Vec<u8>, i32> {
        self.state
            .lock()
            .unwrap()
            .control_responses
            .pop_front()
            .unwrap_or(Ok(Vec::new()))
    }
}

#[cfg(test)]
#[derive(Default)]
pub(super) struct FakePcscBackend {
    readers: Vec<String>,
    cards: HashMap<String, Arc<Mutex<FakeCardState>>>,
}

#[cfg(test)]
impl FakePcscBackend {
    pub(super) fn with_reader(mut self, reader: &str, state: FakeCardState) -> Self {
        self.readers.push(reader.to_string());
        self.cards
            .insert(reader.to_string(), Arc::new(Mutex::new(state)));
        self
    }
}

#[cfg(test)]
impl PcscBackend for FakePcscBackend {
    fn list_readers_owned(&self) -> Result<Vec<String>, i32> {
        Ok(self.readers.clone())
    }

    fn connect(
        &self,
        reader: &str,
        _share_mode: PcscShareMode,
        _preferred_protocols: PcscProtocols,
    ) -> Result<Box<dyn PcscCard>, i32> {
        let state = self.cards.get(reader).cloned().ok_or(NFC_ENOTSUCHDEV)?;
        Ok(Box::new(FakePcscCard { state }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iso14443a_status() -> PcscCardStatus {
        PcscCardStatus {
            present: true,
            atr: vec![0x3B, 0x83, 0x80, 0x01, 0xAA, 0xBB, 0xCC, 0xDD],
            protocol: Some(PcscProtocol::T0),
        }
    }

    #[test]
    fn scan_filters_out_acr122_readers() {
        let backend = Arc::new(
            FakePcscBackend::default()
                .with_reader("ACS ACR122U PICC Interface 00 00", FakeCardState::default())
                .with_reader("Feitian R502 CL Reader 0", FakeCardState::default()),
        );
        let driver = PcscDriver::with_backend(backend);
        let context = Context::new();

        let devices = driver.scan(&context).unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].as_str(), "pcsc:Feitian R502 CL Reader 0");
    }

    #[test]
    fn open_resolves_index_connstrings() {
        let backend = Arc::new(
            FakePcscBackend::default()
                .with_reader("Reader A", FakeCardState::default())
                .with_reader("Reader B", FakeCardState::default()),
        );
        let driver = PcscDriver::with_backend(backend);
        let context = Context::new();

        let connstring = ConnectionString::new("pcsc:1").unwrap();
        let device = driver.open(&context, &connstring).unwrap();
        assert_eq!(device.connstring().as_str(), "pcsc:Reader B");
    }

    #[test]
    fn select_passive_target_builds_iso14443a_target() {
        let mut state = FakeCardState::default();
        state.status_responses.push_back(Ok(iso14443a_status()));
        state
            .attributes
            .insert(PcscAttribute::IccTypePerAtr, Ok(vec![ICC_TYPE_14443A]));
        state
            .transmit_responses
            .push_back(Ok(vec![0x01, 0x02, 0x03, 0x04, 0x90, 0x00]));
        let backend = Arc::new(FakePcscBackend::default().with_reader("Reader A", state));
        let driver = PcscDriver::with_backend(backend);
        let context = Context::new();
        let connstring = ConnectionString::new("pcsc:Reader A").unwrap();
        let mut device = driver.open(&context, &connstring).unwrap();

        let target = device
            .select_passive_target(
                Modulation {
                    modulation_type: ModulationType::Iso14443A,
                    baud_rate: BaudRate::Br106,
                },
                None,
            )
            .unwrap()
            .unwrap();
        assert_eq!(target.modulation.modulation_type, ModulationType::Iso14443A);
        match target.info {
            TargetInfo::Iso14443A { uid, .. } => assert_eq!(uid, vec![0x01, 0x02, 0x03, 0x04]),
            _ => panic!("unexpected target info"),
        }
    }

    #[test]
    fn feitian_transceive_routes_through_apdu_translation() {
        let mut state = FakeCardState::default();
        state.transmit_responses.push_back(Ok(vec![0x90, 0x00]));
        let card = Box::new(FakePcscCard {
            state: Arc::new(Mutex::new(state)),
        });
        let mut device = PcscDevice::new(
            "Feitian Reader".into(),
            ConnectionString::new("pcsc:Feitian Reader").unwrap(),
            card,
            PcscShareMode::Direct,
            PcscProtocols::T0,
        );
        let mut rx = [0u8; 8];
        let size = device.transceive_bytes(&[0x30, 0x04], &mut rx, 75).unwrap();
        assert_eq!(size, 2);
        assert_eq!(&rx[..size], &[0x90, 0x00]);
    }

    #[test]
    fn information_about_formats_vendor_attributes() {
        let mut state = FakeCardState::default();
        state
            .attributes
            .insert(PcscAttribute::VendorName, Ok(b"Model\0".to_vec()));
        state
            .attributes
            .insert(PcscAttribute::VendorIfdType, Ok(b"Vendor\0".to_vec()));
        state
            .attributes
            .insert(PcscAttribute::VendorIfdVersion, Ok(b"1.0\0".to_vec()));
        state
            .attributes
            .insert(PcscAttribute::VendorIfdSerialNo, Ok(b"ABC123\0".to_vec()));
        let card = Box::new(FakePcscCard {
            state: Arc::new(Mutex::new(state)),
        });
        let mut device = PcscDevice::new(
            "Reader".into(),
            ConnectionString::new("pcsc:Reader").unwrap(),
            card,
            PcscShareMode::Direct,
            PcscProtocols::T0,
        );

        assert_eq!(
            device.information_about().unwrap(),
            "Model 1.0 (Vendor)\nserial: ABC123\n"
        );
    }
}
