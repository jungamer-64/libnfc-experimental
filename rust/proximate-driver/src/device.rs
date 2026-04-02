use std::any::Any;

use crate::{
    BaudRate, ConnectionString, DepInfo, DepMode, DeviceCaps, Error, Mode, Modulation,
    ModulationType, Property, Target,
};

pub(crate) const POLL_DEP_PERIOD_MS: i32 = 300;

fn missing_capability(operation: &'static str) -> Error {
    Error::MissingCapability(operation)
}

fn ensure_device_caps(
    caps: DeviceCaps,
    required: DeviceCaps,
    operation: &'static str,
) -> Result<(), Error> {
    if caps.contains(required) {
        Ok(())
    } else {
        Err(missing_capability(operation))
    }
}

fn ensure_any_device_caps(
    caps: DeviceCaps,
    supported: DeviceCaps,
    operation: &'static str,
) -> Result<(), Error> {
    if caps.intersects(supported) {
        Ok(())
    } else {
        Err(missing_capability(operation))
    }
}

fn initiator_view_caps() -> DeviceCaps {
    DeviceCaps::INITIATOR_INIT
        | DeviceCaps::INITIATOR_INIT_SECURE_ELEMENT
        | DeviceCaps::SELECT_PASSIVE_TARGET
        | DeviceCaps::POLL_TARGET
        | DeviceCaps::SELECT_DEP_TARGET
        | DeviceCaps::DESELECT_TARGET
        | DeviceCaps::TARGET_IS_PRESENT
        | DeviceCaps::TRANSCEIVE_BYTES
        | DeviceCaps::TRANSCEIVE_BITS
        | DeviceCaps::TRANSCEIVE_BYTES_TIMED
        | DeviceCaps::TRANSCEIVE_BITS_TIMED
        | DeviceCaps::ABORT_COMMAND
        | DeviceCaps::IDLE
        | DeviceCaps::POWERDOWN
}

fn target_view_caps() -> DeviceCaps {
    DeviceCaps::TARGET_INIT
        | DeviceCaps::TARGET_SEND_BYTES
        | DeviceCaps::TARGET_RECEIVE_BYTES
        | DeviceCaps::TARGET_SEND_BITS
        | DeviceCaps::TARGET_RECEIVE_BITS
}

fn pn53x_view_caps() -> DeviceCaps {
    DeviceCaps::PN53X_TRANSCEIVE
        | DeviceCaps::PN53X_READ_REGISTER
        | DeviceCaps::PN53X_WRITE_REGISTER
        | DeviceCaps::PN532_SAM_CONFIGURATION
}

pub trait Logger: Send + Sync {
    fn log(&self, _priority: u8, _message: &str) {}
}

pub trait DeviceMeta: Send + Any {
    fn name(&self) -> &str;
    fn connstring(&self) -> &ConnectionString;

    fn caps(&self) -> DeviceCaps {
        DeviceCaps::NONE
    }

    fn last_error(&self) -> i32 {
        0
    }

    fn strerror(&self) -> String {
        crate::device_error_message(self.last_error()).to_string()
    }
}

pub trait InfoBackend: DeviceMeta {
    fn information_about(&mut self) -> Result<String, Error> {
        Err(Error::UnsupportedOperation("information_about"))
    }
}

pub trait PropertyBackend: DeviceMeta {
    fn set_property_bool(&mut self, property: Property, enable: bool) -> Result<(), Error>;

    fn set_property_int(&mut self, property: Property, value: i32) -> Result<(), Error>;

    fn supported_modulations(&mut self, mode: Mode) -> Result<Vec<ModulationType>, Error>;

    fn supported_baud_rates(
        &mut self,
        mode: Mode,
        modulation_type: ModulationType,
    ) -> Result<Vec<BaudRate>, Error>;

    fn property_bool_state(&self, _property: Property) -> Option<bool> {
        None
    }
}

pub trait InitiatorBackend: DeviceMeta {
    fn initiator_init_driver(&mut self) -> Result<i32, Error> {
        Err(Error::UnsupportedOperation("initiator_init"))
    }

    fn initiator_init_secure_element_driver(&mut self) -> Result<i32, Error> {
        Err(Error::UnsupportedOperation("initiator_init_secure_element"))
    }

    fn select_passive_target_driver(
        &mut self,
        _nm: Modulation,
        _init_data: &[u8],
    ) -> Result<Option<Target>, Error> {
        Err(Error::UnsupportedOperation("select_passive_target"))
    }

    fn poll_target_driver(
        &mut self,
        _modulations: &[Modulation],
        _poll_nr: u8,
        _period: u8,
    ) -> Result<Option<Target>, Error> {
        Err(Error::UnsupportedOperation("poll_target"))
    }

    fn select_dep_target_driver(
        &mut self,
        _ndm: DepMode,
        _nbr: BaudRate,
        _initiator: Option<&DepInfo>,
        _timeout: i32,
    ) -> Result<Option<Target>, Error> {
        Err(Error::UnsupportedOperation("select_dep_target"))
    }

    fn deselect_target_driver(&mut self) -> Result<(), Error> {
        Err(Error::UnsupportedOperation("deselect_target"))
    }

    fn target_is_present_driver(&mut self, _target: Option<&Target>) -> Result<bool, Error> {
        Err(Error::UnsupportedOperation("target_is_present"))
    }

    fn transceive_bytes_driver(
        &mut self,
        _tx: &[u8],
        _rx: &mut [u8],
        _timeout: i32,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("transceive_bytes"))
    }

    fn transceive_bits_driver(
        &mut self,
        _tx: &[u8],
        _tx_bits_len: usize,
        _tx_parity: Option<&[u8]>,
        _rx: &mut [u8],
        _rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("transceive_bits"))
    }

    fn transceive_bytes_timed_driver(
        &mut self,
        _tx: &[u8],
        _rx: &mut [u8],
    ) -> Result<(usize, u32), Error> {
        Err(Error::UnsupportedOperation("transceive_bytes_timed"))
    }

    fn transceive_bits_timed_driver(
        &mut self,
        _tx: &[u8],
        _tx_bits_len: usize,
        _tx_parity: Option<&[u8]>,
        _rx: &mut [u8],
        _rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), Error> {
        Err(Error::UnsupportedOperation("transceive_bits_timed"))
    }

    fn abort_command_driver(&mut self) -> Result<(), Error> {
        Err(Error::UnsupportedOperation("abort_command"))
    }

    fn idle_driver(&mut self) -> Result<(), Error> {
        Err(Error::UnsupportedOperation("idle"))
    }

    fn powerdown_driver(&mut self) -> Result<(), Error> {
        Err(Error::UnsupportedOperation("powerdown"))
    }
}

pub trait TargetBackend: DeviceMeta {
    fn target_init_driver(
        &mut self,
        _target: &mut Target,
        _rx: &mut [u8],
        _timeout: i32,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("target_init"))
    }

    fn target_send_bytes_driver(&mut self, _tx: &[u8], _timeout: i32) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("target_send_bytes"))
    }

    fn target_receive_bytes_driver(
        &mut self,
        _rx: &mut [u8],
        _timeout: i32,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("target_receive_bytes"))
    }

    fn target_send_bits_driver(
        &mut self,
        _tx: &[u8],
        _tx_bits_len: usize,
        _tx_parity: Option<&[u8]>,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("target_send_bits"))
    }

    fn target_receive_bits_driver(
        &mut self,
        _rx: &mut [u8],
        _rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("target_receive_bits"))
    }
}

pub trait Pn53xBackend: DeviceMeta {
    fn pn53x_transceive_driver(
        &mut self,
        _tx: &[u8],
        _rx: &mut [u8],
        _timeout: i32,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("pn53x_transceive"))
    }

    fn pn53x_read_register_driver(&mut self, _register: u16) -> Result<u8, Error> {
        Err(Error::UnsupportedOperation("pn53x_read_register"))
    }

    fn pn53x_write_register_driver(
        &mut self,
        _register: u16,
        _symbol_mask: u8,
        _value: u8,
    ) -> Result<(), Error> {
        Err(Error::UnsupportedOperation("pn53x_write_register"))
    }

    fn pn532_sam_configuration_driver(&mut self, _mode: u8, _timeout: i32) -> Result<i32, Error> {
        Err(Error::UnsupportedOperation("pn532_SAMConfiguration"))
    }
}

pub trait DeviceBackend:
    DeviceMeta + InfoBackend + PropertyBackend + InitiatorBackend + TargetBackend + Pn53xBackend
{
}

impl<T> DeviceBackend for T where
    T: DeviceMeta + InfoBackend + PropertyBackend + InitiatorBackend + TargetBackend + Pn53xBackend
{
}

#[doc(hidden)]
pub trait OpenedDevice: DeviceBackend {}

impl<T: DeviceBackend + ?Sized> OpenedDevice for T {}

pub struct Device {
    display_name: Option<String>,
    handle: Box<dyn DeviceBackend>,
}

impl Device {
    pub(crate) fn new(handle: Box<dyn DeviceBackend>, display_name: Option<String>) -> Self {
        Self {
            display_name,
            handle,
        }
    }

    pub fn name(&self) -> &str {
        self.display_name
            .as_deref()
            .unwrap_or_else(|| self.handle.name())
    }

    pub fn connstring(&self) -> &ConnectionString {
        self.handle.connstring()
    }

    pub fn caps(&self) -> DeviceCaps {
        self.handle.caps()
    }

    pub fn last_error(&self) -> i32 {
        self.handle.last_error()
    }

    pub fn strerror(&self) -> String {
        self.handle.strerror()
    }

    pub fn information_about(&mut self) -> Result<String, Error> {
        ops::information_about(self.handle.as_mut())
    }

    pub fn set_property_bool(&mut self, property: Property, enable: bool) -> Result<(), Error> {
        ops::set_property_bool(self.handle.as_mut(), property, enable)
    }

    pub fn set_property_int(&mut self, property: Property, value: i32) -> Result<(), Error> {
        ops::set_property_int(self.handle.as_mut(), property, value)
    }

    pub fn supported_modulations(&mut self, mode: Mode) -> Result<Vec<ModulationType>, Error> {
        ops::supported_modulations(self.handle.as_mut(), mode)
    }

    pub fn supported_baud_rates(
        &mut self,
        mode: Mode,
        modulation_type: ModulationType,
    ) -> Result<Vec<BaudRate>, Error> {
        ops::supported_baud_rates(self.handle.as_mut(), mode, modulation_type)
    }

    pub fn initiator(&mut self) -> Result<InitiatorDevice<'_>, Error> {
        ensure_any_device_caps(self.caps(), initiator_view_caps(), "initiator")?;
        Ok(InitiatorDevice {
            device: self.handle.as_mut(),
        })
    }

    pub fn target(&mut self) -> Result<TargetDevice<'_>, Error> {
        ensure_any_device_caps(self.caps(), target_view_caps(), "target")?;
        Ok(TargetDevice {
            device: self.handle.as_mut(),
        })
    }

    pub fn pn53x(&mut self) -> Result<Pn53xDevice<'_>, Error> {
        ensure_any_device_caps(self.caps(), pn53x_view_caps(), "pn53x")?;
        Ok(Pn53xDevice {
            device: self.handle.as_mut(),
        })
    }

    #[doc(hidden)]
    pub fn into_backend(self) -> Box<dyn DeviceBackend> {
        self.handle
    }
}

pub struct InitiatorDevice<'a> {
    device: &'a mut dyn DeviceBackend,
}

impl<'a> InitiatorDevice<'a> {
    pub fn init(&mut self) -> Result<i32, Error> {
        ops::initiator_init(self.device)
    }

    pub fn init_secure_element(&mut self) -> Result<i32, Error> {
        ops::initiator_init_secure_element(self.device)
    }

    pub fn select_passive_target(
        &mut self,
        modulation: Modulation,
        init_data: Option<&[u8]>,
    ) -> Result<Option<Target>, Error> {
        ops::select_passive_target(self.device, modulation, init_data)
    }

    pub fn list_passive_targets(
        &mut self,
        modulation: Modulation,
        max_targets: usize,
    ) -> Result<Vec<Target>, Error> {
        ops::list_passive_targets(self.device, modulation, max_targets)
    }

    pub fn poll_target(
        &mut self,
        modulations: &[Modulation],
        poll_nr: u8,
        period: u8,
    ) -> Result<Option<Target>, Error> {
        ops::poll_target(self.device, modulations, poll_nr, period)
    }

    pub fn select_dep_target(
        &mut self,
        mode: DepMode,
        baud_rate: BaudRate,
        initiator: Option<&DepInfo>,
        timeout: i32,
    ) -> Result<Option<Target>, Error> {
        ops::select_dep_target(self.device, mode, baud_rate, initiator, timeout)
    }

    pub fn poll_dep_target(
        &mut self,
        mode: DepMode,
        baud_rate: BaudRate,
        initiator: Option<&DepInfo>,
        timeout: i32,
    ) -> Result<Option<Target>, Error> {
        ops::poll_dep_target(self.device, mode, baud_rate, initiator, timeout)
    }

    pub fn deselect_target(&mut self) -> Result<(), Error> {
        ops::deselect_target(self.device)
    }

    pub fn target_is_present(&mut self, target: Option<&Target>) -> Result<bool, Error> {
        ops::target_is_present(self.device, target)
    }

    pub fn transceive_bytes(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error> {
        ops::transceive_bytes(self.device, tx, rx, timeout)
    }

    pub fn transceive_bits(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        ops::transceive_bits(self.device, tx, tx_bits_len, tx_parity, rx, rx_parity)
    }

    pub fn transceive_bytes_timed(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
    ) -> Result<(usize, u32), Error> {
        ops::transceive_bytes_timed(self.device, tx, rx)
    }

    pub fn transceive_bits_timed(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), Error> {
        ops::transceive_bits_timed(self.device, tx, tx_bits_len, tx_parity, rx, rx_parity)
    }

    pub fn abort_command(&mut self) -> Result<(), Error> {
        ops::abort_command(self.device)
    }

    pub fn idle(&mut self) -> Result<(), Error> {
        ops::idle(self.device)
    }

    pub fn powerdown(&mut self) -> Result<(), Error> {
        ops::powerdown(self.device)
    }
}

pub struct TargetDevice<'a> {
    device: &'a mut dyn DeviceBackend,
}

impl<'a> TargetDevice<'a> {
    pub fn init(
        &mut self,
        target: &mut Target,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error> {
        ops::target_init(self.device, target, rx, timeout)
    }

    pub fn send_bytes(&mut self, tx: &[u8], timeout: i32) -> Result<usize, Error> {
        ops::target_send_bytes(self.device, tx, timeout)
    }

    pub fn receive_bytes(&mut self, rx: &mut [u8], timeout: i32) -> Result<usize, Error> {
        ops::target_receive_bytes(self.device, rx, timeout)
    }

    pub fn send_bits(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
    ) -> Result<usize, Error> {
        ops::target_send_bits(self.device, tx, tx_bits_len, tx_parity)
    }

    pub fn receive_bits(
        &mut self,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        ops::target_receive_bits(self.device, rx, rx_parity)
    }
}

pub struct Pn53xDevice<'a> {
    device: &'a mut dyn DeviceBackend,
}

impl<'a> Pn53xDevice<'a> {
    pub fn transceive(&mut self, tx: &[u8], rx: &mut [u8], timeout: i32) -> Result<usize, Error> {
        ops::pn53x_transceive(self.device, tx, rx, timeout)
    }

    pub fn read_register(&mut self, register: u16) -> Result<u8, Error> {
        ops::pn53x_read_register(self.device, register)
    }

    pub fn write_register(
        &mut self,
        register: u16,
        symbol_mask: u8,
        value: u8,
    ) -> Result<(), Error> {
        ops::pn53x_write_register(self.device, register, symbol_mask, value)
    }

    pub fn sam_configuration(&mut self, mode: u8, timeout: i32) -> Result<i32, Error> {
        ops::pn532_sam_configuration(self.device, mode, timeout)
    }
}

pub trait PropertyOps: InfoBackend + PropertyBackend {}
impl<T: InfoBackend + PropertyBackend + ?Sized> PropertyOps for T {}

pub trait InitiatorOps: DeviceMeta + PropertyBackend + InitiatorBackend {
    fn initiator_init(&mut self) -> Result<i32, Error> {
        ops::initiator_init(self)
    }

    fn initiator_init_secure_element(&mut self) -> Result<i32, Error> {
        ops::initiator_init_secure_element(self)
    }

    fn select_passive_target(
        &mut self,
        modulation: Modulation,
        init_data: Option<&[u8]>,
    ) -> Result<Option<Target>, Error> {
        ops::select_passive_target(self, modulation, init_data)
    }

    fn list_passive_targets(
        &mut self,
        modulation: Modulation,
        max_targets: usize,
    ) -> Result<Vec<Target>, Error> {
        ops::list_passive_targets(self, modulation, max_targets)
    }

    fn poll_target(
        &mut self,
        modulations: &[Modulation],
        poll_nr: u8,
        period: u8,
    ) -> Result<Option<Target>, Error> {
        ops::poll_target(self, modulations, poll_nr, period)
    }

    fn select_dep_target(
        &mut self,
        mode: DepMode,
        baud_rate: BaudRate,
        initiator: Option<&DepInfo>,
        timeout: i32,
    ) -> Result<Option<Target>, Error> {
        ops::select_dep_target(self, mode, baud_rate, initiator, timeout)
    }

    fn poll_dep_target(
        &mut self,
        mode: DepMode,
        baud_rate: BaudRate,
        initiator: Option<&DepInfo>,
        timeout: i32,
    ) -> Result<Option<Target>, Error> {
        ops::poll_dep_target(self, mode, baud_rate, initiator, timeout)
    }

    fn deselect_target(&mut self) -> Result<(), Error> {
        ops::deselect_target(self)
    }

    fn target_is_present(&mut self, target: Option<&Target>) -> Result<bool, Error> {
        ops::target_is_present(self, target)
    }

    fn transceive_bytes(&mut self, tx: &[u8], rx: &mut [u8], timeout: i32) -> Result<usize, Error> {
        ops::transceive_bytes(self, tx, rx, timeout)
    }

    fn transceive_bits(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        ops::transceive_bits(self, tx, tx_bits_len, tx_parity, rx, rx_parity)
    }

    fn transceive_bytes_timed(&mut self, tx: &[u8], rx: &mut [u8]) -> Result<(usize, u32), Error> {
        ops::transceive_bytes_timed(self, tx, rx)
    }

    fn transceive_bits_timed(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), Error> {
        ops::transceive_bits_timed(self, tx, tx_bits_len, tx_parity, rx, rx_parity)
    }

    fn abort_command(&mut self) -> Result<(), Error> {
        ops::abort_command(self)
    }

    fn idle(&mut self) -> Result<(), Error> {
        ops::idle(self)
    }

    fn powerdown(&mut self) -> Result<(), Error> {
        ops::powerdown(self)
    }
}

impl<T: DeviceMeta + PropertyBackend + InitiatorBackend + ?Sized> InitiatorOps for T {}

pub trait TargetOps: DeviceMeta + PropertyBackend + TargetBackend {
    fn target_init(
        &mut self,
        target: &mut Target,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error> {
        ops::target_init(self, target, rx, timeout)
    }

    fn target_send_bytes(&mut self, tx: &[u8], timeout: i32) -> Result<usize, Error> {
        ops::target_send_bytes(self, tx, timeout)
    }

    fn target_receive_bytes(&mut self, rx: &mut [u8], timeout: i32) -> Result<usize, Error> {
        ops::target_receive_bytes(self, rx, timeout)
    }

    fn target_send_bits(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
    ) -> Result<usize, Error> {
        ops::target_send_bits(self, tx, tx_bits_len, tx_parity)
    }

    fn target_receive_bits(
        &mut self,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        ops::target_receive_bits(self, rx, rx_parity)
    }
}

impl<T: DeviceMeta + PropertyBackend + TargetBackend + ?Sized> TargetOps for T {}

pub trait ChipDebugOps: DeviceMeta + Pn53xBackend {
    fn pn53x_transceive(&mut self, tx: &[u8], rx: &mut [u8], timeout: i32) -> Result<usize, Error> {
        ops::pn53x_transceive(self, tx, rx, timeout)
    }

    fn pn53x_read_register(&mut self, register: u16) -> Result<u8, Error> {
        ops::pn53x_read_register(self, register)
    }

    fn pn53x_write_register(
        &mut self,
        register: u16,
        symbol_mask: u8,
        value: u8,
    ) -> Result<(), Error> {
        ops::pn53x_write_register(self, register, symbol_mask, value)
    }

    fn pn532_sam_configuration(&mut self, mode: u8, timeout: i32) -> Result<i32, Error> {
        ops::pn532_sam_configuration(self, mode, timeout)
    }
}

impl<T: DeviceMeta + Pn53xBackend + ?Sized> ChipDebugOps for T {}

pub(crate) fn apply_bool_property_sequence<D: PropertyBackend + ?Sized>(
    device: &mut D,
    settings: &[(Property, bool)],
) -> Result<(), Error> {
    for (property, value) in settings {
        device.set_property_bool(*property, *value)?;
    }
    Ok(())
}

pub(crate) fn restore_property_bool<D: PropertyBackend + ?Sized>(
    device: &mut D,
    property: Property,
    previous: Option<bool>,
    temporary_value: bool,
) -> Result<(), Error> {
    if let Some(previous) = previous
        && previous != temporary_value
    {
        device.set_property_bool(property, previous)?;
    }
    Ok(())
}

pub(crate) fn validate_modulation<D: PropertyBackend + ?Sized>(
    device: &mut D,
    mode: Mode,
    modulation: Modulation,
) -> Result<(), Error> {
    let supported_modulations = device.supported_modulations(mode)?;
    if !supported_modulations.contains(&modulation.modulation_type) {
        return Err(Error::InvalidArgument("modulation not supported"));
    }

    let supported_baud_rates = device.supported_baud_rates(mode, modulation.modulation_type)?;
    if !supported_baud_rates.contains(&modulation.baud_rate) {
        return Err(Error::InvalidArgument("baud rate not supported"));
    }

    Ok(())
}

pub(crate) fn default_initiator_payload(modulation: Modulation) -> &'static [u8] {
    match modulation.modulation_type {
        ModulationType::Iso14443B => &[0x00],
        ModulationType::Iso14443Bi => &[0x01, 0x0b, 0x3f, 0x80],
        ModulationType::Felica => &[0x00, 0xff, 0xff, 0x01, 0x00],
        _ => &[],
    }
}

pub(crate) fn cascade_iso14443a_uid(uid: &[u8]) -> Vec<u8> {
    match uid.len() {
        4 => uid.to_vec(),
        7 => {
            let mut cascaded = Vec::with_capacity(8);
            cascaded.extend_from_slice(&[0x88, uid[0], uid[1], uid[2]]);
            cascaded.extend_from_slice(&uid[3..]);
            cascaded
        }
        10 => {
            let mut cascaded = Vec::with_capacity(12);
            cascaded.extend_from_slice(&[0x88, uid[0], uid[1], uid[2]]);
            cascaded.extend_from_slice(&[0x88, uid[3], uid[4], uid[5]]);
            cascaded.extend_from_slice(&uid[6..]);
            cascaded
        }
        _ => Vec::new(),
    }
}

pub(crate) fn modulation_requires_single_attempt(modulation: Modulation) -> bool {
    matches!(
        modulation.modulation_type,
        ModulationType::Felica
            | ModulationType::Jewel
            | ModulationType::Barcode
            | ModulationType::Iso14443Bi
            | ModulationType::Iso14443B2Sr
            | ModulationType::Iso14443B2Ct
    )
}

mod ops {
    use super::*;

    pub(super) fn information_about<D>(device: &mut D) -> Result<String, Error>
    where
        D: InfoBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::INFO,
            "device_get_information_about",
        )?;
        device.information_about()
    }

    pub(super) fn set_property_bool<D>(
        device: &mut D,
        property: Property,
        enable: bool,
    ) -> Result<(), Error>
    where
        D: PropertyBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::SET_PROPERTY_BOOL,
            "device_set_property_bool",
        )?;
        device.set_property_bool(property, enable)
    }

    pub(super) fn set_property_int<D>(
        device: &mut D,
        property: Property,
        value: i32,
    ) -> Result<(), Error>
    where
        D: PropertyBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::SET_PROPERTY_INT,
            "device_set_property_int",
        )?;
        device.set_property_int(property, value)
    }

    pub(super) fn supported_modulations<D>(
        device: &mut D,
        mode: Mode,
    ) -> Result<Vec<ModulationType>, Error>
    where
        D: PropertyBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::SUPPORTED_MODULATIONS,
            "get_supported_modulation",
        )?;
        device.supported_modulations(mode)
    }

    pub(super) fn supported_baud_rates<D>(
        device: &mut D,
        mode: Mode,
        modulation_type: ModulationType,
    ) -> Result<Vec<BaudRate>, Error>
    where
        D: PropertyBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::SUPPORTED_BAUD_RATES,
            "get_supported_baud_rate",
        )?;
        device.supported_baud_rates(mode, modulation_type)
    }

    pub(super) fn initiator_init<D>(device: &mut D) -> Result<i32, Error>
    where
        D: PropertyBackend + InitiatorBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::SET_PROPERTY_BOOL | DeviceCaps::INITIATOR_INIT,
            "initiator_init",
        )?;
        apply_bool_property_sequence(
            device,
            &[
                (Property::ActivateField, false),
                (Property::ActivateField, true),
                (Property::InfiniteSelect, true),
                (Property::AutoIso14443_4, true),
                (Property::ForceIso14443A, true),
                (Property::ForceSpeed106, true),
                (Property::AcceptInvalidFrames, false),
                (Property::AcceptMultipleFrames, false),
            ],
        )?;
        device.initiator_init_driver()
    }

    pub(super) fn initiator_init_secure_element<D>(device: &mut D) -> Result<i32, Error>
    where
        D: InitiatorBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::INITIATOR_INIT_SECURE_ELEMENT,
            "initiator_init_secure_element",
        )?;
        device.initiator_init_secure_element_driver()
    }

    pub(super) fn select_passive_target<D>(
        device: &mut D,
        nm: Modulation,
        init_data: Option<&[u8]>,
    ) -> Result<Option<Target>, Error>
    where
        D: PropertyBackend + InitiatorBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::SUPPORTED_MODULATIONS
                | DeviceCaps::SUPPORTED_BAUD_RATES
                | DeviceCaps::SELECT_PASSIVE_TARGET,
            "initiator_select_passive_target",
        )?;
        validate_modulation(device, Mode::Initiator, nm)?;

        let payload = if init_data.is_some_and(|value| !value.is_empty()) {
            if nm.modulation_type == ModulationType::Iso14443A {
                cascade_iso14443a_uid(init_data.expect("checked above"))
            } else {
                init_data.expect("checked above").to_vec()
            }
        } else {
            default_initiator_payload(nm).to_vec()
        };

        device.select_passive_target_driver(nm, &payload)
    }

    pub(super) fn list_passive_targets<D>(
        device: &mut D,
        nm: Modulation,
        max_targets: usize,
    ) -> Result<Vec<Target>, Error>
    where
        D: PropertyBackend + InitiatorBackend + ?Sized,
    {
        if max_targets == 0 {
            return Ok(Vec::new());
        }

        let mut required = DeviceCaps::SUPPORTED_MODULATIONS
            | DeviceCaps::SUPPORTED_BAUD_RATES
            | DeviceCaps::SELECT_PASSIVE_TARGET
            | DeviceCaps::SET_PROPERTY_BOOL;
        if max_targets > 1 && !modulation_requires_single_attempt(nm) {
            required |= DeviceCaps::DESELECT_TARGET;
        }
        ensure_device_caps(device.caps(), required, "list_passive_targets")?;

        let previous = device.property_bool_state(Property::InfiniteSelect);
        device.set_property_bool(Property::InfiniteSelect, false)?;

        let result = (|| {
            let mut targets = Vec::new();
            while let Some(target) = select_passive_target(device, nm, None)? {
                if targets.contains(&target) {
                    break;
                }

                targets.push(target);
                if targets.len() >= max_targets || modulation_requires_single_attempt(nm) {
                    break;
                }

                deselect_target(device)?;
            }
            Ok(targets)
        })();

        restore_property_bool(device, Property::InfiniteSelect, previous, false)?;
        result
    }

    pub(super) fn poll_target<D>(
        device: &mut D,
        modulations: &[Modulation],
        poll_nr: u8,
        period: u8,
    ) -> Result<Option<Target>, Error>
    where
        D: InitiatorBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::POLL_TARGET,
            "initiator_poll_target",
        )?;
        device.poll_target_driver(modulations, poll_nr, period)
    }

    pub(super) fn select_dep_target<D>(
        device: &mut D,
        ndm: DepMode,
        nbr: BaudRate,
        initiator: Option<&DepInfo>,
        timeout: i32,
    ) -> Result<Option<Target>, Error>
    where
        D: InitiatorBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::SELECT_DEP_TARGET,
            "initiator_select_dep_target",
        )?;
        device.select_dep_target_driver(ndm, nbr, initiator, timeout)
    }

    pub(super) fn poll_dep_target<D>(
        device: &mut D,
        ndm: DepMode,
        nbr: BaudRate,
        initiator: Option<&DepInfo>,
        timeout: i32,
    ) -> Result<Option<Target>, Error>
    where
        D: PropertyBackend + InitiatorBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::SET_PROPERTY_BOOL | DeviceCaps::SELECT_DEP_TARGET,
            "poll_dep_target",
        )?;
        let previous = device.property_bool_state(Property::InfiniteSelect);
        device.set_property_bool(Property::InfiniteSelect, true)?;

        let result = (|| {
            let mut remaining = timeout;
            while remaining > 0 {
                match select_dep_target(device, ndm, nbr, initiator, POLL_DEP_PERIOD_MS) {
                    Ok(Some(target)) => return Ok(Some(target)),
                    Ok(None) => remaining -= POLL_DEP_PERIOD_MS,
                    Err(error) if error.device_code() == Some(-6) => {
                        remaining -= POLL_DEP_PERIOD_MS;
                    }
                    Err(error) => return Err(error),
                }
            }
            Ok(None)
        })();

        restore_property_bool(device, Property::InfiniteSelect, previous, true)?;
        result
    }

    pub(super) fn deselect_target<D>(device: &mut D) -> Result<(), Error>
    where
        D: InitiatorBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::DESELECT_TARGET,
            "initiator_deselect_target",
        )?;
        device.deselect_target_driver()
    }

    pub(super) fn target_is_present<D>(
        device: &mut D,
        target: Option<&Target>,
    ) -> Result<bool, Error>
    where
        D: InitiatorBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::TARGET_IS_PRESENT,
            "initiator_target_is_present",
        )?;
        device.target_is_present_driver(target)
    }

    pub(super) fn transceive_bytes<D>(
        device: &mut D,
        tx: &[u8],
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error>
    where
        D: InitiatorBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::TRANSCEIVE_BYTES,
            "initiator_transceive_bytes",
        )?;
        device.transceive_bytes_driver(tx, rx, timeout)
    }

    pub(super) fn transceive_bits<D>(
        device: &mut D,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error>
    where
        D: InitiatorBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::TRANSCEIVE_BITS,
            "initiator_transceive_bits",
        )?;
        device.transceive_bits_driver(tx, tx_bits_len, tx_parity, rx, rx_parity)
    }

    pub(super) fn transceive_bytes_timed<D>(
        device: &mut D,
        tx: &[u8],
        rx: &mut [u8],
    ) -> Result<(usize, u32), Error>
    where
        D: InitiatorBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::TRANSCEIVE_BYTES_TIMED,
            "initiator_transceive_bytes_timed",
        )?;
        device.transceive_bytes_timed_driver(tx, rx)
    }

    pub(super) fn transceive_bits_timed<D>(
        device: &mut D,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), Error>
    where
        D: InitiatorBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::TRANSCEIVE_BITS_TIMED,
            "initiator_transceive_bits_timed",
        )?;
        device.transceive_bits_timed_driver(tx, tx_bits_len, tx_parity, rx, rx_parity)
    }

    pub(super) fn target_init<D>(
        device: &mut D,
        target: &mut Target,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error>
    where
        D: PropertyBackend + TargetBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::SET_PROPERTY_BOOL | DeviceCaps::TARGET_INIT,
            "target_init",
        )?;
        apply_bool_property_sequence(
            device,
            &[
                (Property::AcceptInvalidFrames, false),
                (Property::AcceptMultipleFrames, false),
                (Property::HandleCrc, true),
                (Property::HandleParity, true),
                (Property::AutoIso14443_4, true),
                (Property::EasyFraming, true),
                (Property::ActivateCrypto1, false),
                (Property::ActivateField, false),
            ],
        )?;
        device.target_init_driver(target, rx, timeout)
    }

    pub(super) fn target_send_bytes<D>(
        device: &mut D,
        tx: &[u8],
        timeout: i32,
    ) -> Result<usize, Error>
    where
        D: TargetBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::TARGET_SEND_BYTES,
            "target_send_bytes",
        )?;
        device.target_send_bytes_driver(tx, timeout)
    }

    pub(super) fn target_receive_bytes<D>(
        device: &mut D,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error>
    where
        D: TargetBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::TARGET_RECEIVE_BYTES,
            "target_receive_bytes",
        )?;
        device.target_receive_bytes_driver(rx, timeout)
    }

    pub(super) fn target_send_bits<D>(
        device: &mut D,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
    ) -> Result<usize, Error>
    where
        D: TargetBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::TARGET_SEND_BITS,
            "target_send_bits",
        )?;
        device.target_send_bits_driver(tx, tx_bits_len, tx_parity)
    }

    pub(super) fn target_receive_bits<D>(
        device: &mut D,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error>
    where
        D: TargetBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::TARGET_RECEIVE_BITS,
            "target_receive_bits",
        )?;
        device.target_receive_bits_driver(rx, rx_parity)
    }

    pub(super) fn abort_command<D>(device: &mut D) -> Result<(), Error>
    where
        D: InitiatorBackend + ?Sized,
    {
        ensure_device_caps(device.caps(), DeviceCaps::ABORT_COMMAND, "abort_command")?;
        device.abort_command_driver()
    }

    pub(super) fn idle<D>(device: &mut D) -> Result<(), Error>
    where
        D: InitiatorBackend + ?Sized,
    {
        ensure_device_caps(device.caps(), DeviceCaps::IDLE, "idle")?;
        device.idle_driver()
    }

    pub(super) fn powerdown<D>(device: &mut D) -> Result<(), Error>
    where
        D: InitiatorBackend + ?Sized,
    {
        ensure_device_caps(device.caps(), DeviceCaps::POWERDOWN, "powerdown")?;
        device.powerdown_driver()
    }

    pub(super) fn pn53x_transceive<D>(
        device: &mut D,
        tx: &[u8],
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error>
    where
        D: Pn53xBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::PN53X_TRANSCEIVE,
            "pn53x_transceive",
        )?;
        device.pn53x_transceive_driver(tx, rx, timeout)
    }

    pub(super) fn pn53x_read_register<D>(device: &mut D, register: u16) -> Result<u8, Error>
    where
        D: Pn53xBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::PN53X_READ_REGISTER,
            "pn53x_read_register",
        )?;
        device.pn53x_read_register_driver(register)
    }

    pub(super) fn pn53x_write_register<D>(
        device: &mut D,
        register: u16,
        symbol_mask: u8,
        value: u8,
    ) -> Result<(), Error>
    where
        D: Pn53xBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::PN53X_WRITE_REGISTER,
            "pn53x_write_register",
        )?;
        device.pn53x_write_register_driver(register, symbol_mask, value)
    }

    pub(super) fn pn532_sam_configuration<D>(
        device: &mut D,
        mode: u8,
        timeout: i32,
    ) -> Result<i32, Error>
    where
        D: Pn53xBackend + ?Sized,
    {
        ensure_device_caps(
            device.caps(),
            DeviceCaps::PN532_SAM_CONFIGURATION,
            "pn532_SAMConfiguration",
        )?;
        device.pn532_sam_configuration_driver(mode, timeout)
    }
}
