use std::any::Any;
use std::sync::Arc;

use crate::{
    BaudRate, BitFrame, ConnectionString, DepInfo, DepMode, Error, Mode, Modulation,
    ModulationType, OperationTimeout, PollIterations, PollPeriod, Property, Target,
};

pub(crate) const POLL_DEP_PERIOD_MS: i32 = 300;

fn missing_capability(operation: &'static str) -> Error {
    Error::MissingCapability(operation)
}

pub trait Logger: Send + Sync {
    fn log(&self, _priority: u8, _message: &str) {}
}

/// Authority to interrupt the currently running command without borrowing the
/// device backend that owns that command.
///
/// Implementations must be safe to invoke concurrently with any blocking I/O
/// operation performed by the associated device. The authority becomes
/// revoked when that device is finalized.
pub trait CommandAbort: Send + Sync {
    fn abort(&self) -> Result<(), Error>;
}

/// A shareable, operation-limited authority for interrupting a device command.
pub type CommandAbortHandle = Arc<dyn CommandAbort>;

pub trait DeviceMeta: Send + Any {
    fn name(&self) -> &str;
    fn connstring(&self) -> &ConnectionString;

    fn last_error(&self) -> i32 {
        0
    }

    fn strerror(&self) -> String {
        crate::device_error_message(self.last_error()).to_string()
    }

    fn missing_capability(&mut self, operation: &'static str) -> Error {
        missing_capability(operation)
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
    fn command_abort_handle(&self) -> Option<CommandAbortHandle> {
        None
    }

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
        _iterations: PollIterations,
        _period: PollPeriod,
    ) -> Result<Option<Target>, Error> {
        Err(Error::UnsupportedOperation("poll_target"))
    }

    fn select_dep_target_driver(
        &mut self,
        _ndm: DepMode,
        _nbr: BaudRate,
        _initiator: Option<&DepInfo>,
        _timeout: OperationTimeout,
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
        _timeout: OperationTimeout,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("transceive_bytes"))
    }

    fn transceive_bits_driver(
        &mut self,
        _tx: BitFrame<'_>,
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
        _tx: BitFrame<'_>,
        _rx: &mut [u8],
        _rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), Error> {
        Err(Error::UnsupportedOperation("transceive_bits_timed"))
    }

    fn abort_command_driver(&mut self) -> Result<(), Error> {
        self.command_abort_handle()
            .ok_or(Error::UnsupportedOperation("abort_command"))?
            .abort()
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
        _timeout: OperationTimeout,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("target_init"))
    }

    fn target_send_bytes_driver(
        &mut self,
        _tx: &[u8],
        _timeout: OperationTimeout,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("target_send_bytes"))
    }

    fn target_receive_bytes_driver(
        &mut self,
        _rx: &mut [u8],
        _timeout: OperationTimeout,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("target_receive_bytes"))
    }

    fn target_send_bits_driver(&mut self, _tx: BitFrame<'_>) -> Result<usize, Error> {
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
        _timeout: OperationTimeout,
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

    fn pn532_sam_configuration_driver(
        &mut self,
        _mode: u8,
        _timeout: OperationTimeout,
    ) -> Result<i32, Error> {
        Err(Error::UnsupportedOperation("pn532_SAMConfiguration"))
    }
}

#[doc(hidden)]
pub trait DeviceHandle:
    DeviceMeta + InfoBackend + PropertyBackend + InitiatorBackend + TargetBackend + Pn53xBackend
{
}

impl<T> DeviceHandle for T where
    T: DeviceMeta
        + InfoBackend
        + PropertyBackend
        + InitiatorBackend
        + TargetBackend
        + Pn53xBackend
        + ?Sized
{
}

pub struct Device {
    display_name: Option<String>,
    handle: Box<dyn DeviceHandle>,
}

impl Device {
    pub(crate) fn new(handle: Box<dyn DeviceHandle>, display_name: Option<String>) -> Self {
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

    pub fn last_error(&self) -> i32 {
        self.handle.last_error()
    }

    /// Returns the authority used to interrupt a command running on this
    /// device, when the backend can provide concurrent abort semantics.
    pub fn command_abort_handle(&self) -> Option<CommandAbortHandle> {
        self.handle.command_abort_handle()
    }

    pub fn strerror(&self) -> String {
        self.handle.strerror()
    }

    pub fn info_ops(&mut self) -> Result<InfoOps<'_>, Error> {
        Ok(InfoOps {
            device: self.handle.as_mut(),
        })
    }

    /// Transfers an opened driver backend into the device lifecycle.
    ///
    /// This is the production ownership boundary used by driver and C-ABI
    /// integration crates. Callers surrender exclusive finalization authority
    /// to the returned [`Device`].
    #[doc(hidden)]
    pub fn from_backend(handle: Box<dyn DeviceHandle>) -> Self {
        Self::new(handle, None)
    }

    pub fn property_ops(&mut self) -> Result<PropertyOps<'_>, Error> {
        Ok(PropertyOps {
            device: self.handle.as_mut(),
        })
    }

    pub fn passive_scan_ops(&mut self) -> Result<PassiveScanOps<'_>, Error> {
        Ok(PassiveScanOps {
            device: self.handle.as_mut(),
        })
    }

    pub fn dep_ops(&mut self) -> Result<DepOps<'_>, Error> {
        Ok(DepOps {
            device: self.handle.as_mut(),
        })
    }

    pub fn session_ops(&mut self) -> Result<SessionOps<'_>, Error> {
        Ok(SessionOps {
            device: self.handle.as_mut(),
        })
    }

    pub fn initiator_io_ops(&mut self) -> Result<InitiatorIoOps<'_>, Error> {
        Ok(InitiatorIoOps {
            device: self.handle.as_mut(),
        })
    }

    pub fn target_io_ops(&mut self) -> Result<TargetIoOps<'_>, Error> {
        Ok(TargetIoOps {
            device: self.handle.as_mut(),
        })
    }

    pub fn pn53x_ops(&mut self) -> Result<Pn53xOps<'_>, Error> {
        Ok(Pn53xOps {
            device: self.handle.as_mut(),
        })
    }

    /// Transfers finalization authority from the Rust device wrapper to an
    /// owning external ABI wrapper.
    #[doc(hidden)]
    pub fn into_backend(self) -> Box<dyn DeviceHandle> {
        self.handle
    }
}

pub struct InfoOps<'a> {
    device: &'a mut dyn DeviceHandle,
}

impl<'a> InfoOps<'a> {
    pub fn information_about(&mut self) -> Result<String, Error> {
        ops::info::information_about(self.device)
    }
}

pub struct PropertyOps<'a> {
    device: &'a mut dyn DeviceHandle,
}

impl<'a> PropertyOps<'a> {
    pub fn set_property_bool(&mut self, property: Property, enable: bool) -> Result<(), Error> {
        ops::property::set_property_bool(self.device, property, enable)
    }

    pub fn set_property_int(&mut self, property: Property, value: i32) -> Result<(), Error> {
        ops::property::set_property_int(self.device, property, value)
    }

    pub fn supported_modulations(&mut self, mode: Mode) -> Result<Vec<ModulationType>, Error> {
        ops::property::supported_modulations(self.device, mode)
    }

    pub fn supported_baud_rates(
        &mut self,
        mode: Mode,
        modulation_type: ModulationType,
    ) -> Result<Vec<BaudRate>, Error> {
        ops::property::supported_baud_rates(self.device, mode, modulation_type)
    }
}

pub struct PassiveScanOps<'a> {
    device: &'a mut dyn DeviceHandle,
}

impl<'a> PassiveScanOps<'a> {
    pub fn init(&mut self) -> Result<i32, Error> {
        ops::initiator::init(self.device)
    }

    pub fn select_passive_target(
        &mut self,
        modulation: Modulation,
        init_data: Option<&[u8]>,
    ) -> Result<Option<Target>, Error> {
        ops::initiator::select_passive_target(self.device, modulation, init_data)
    }

    pub fn list_passive_targets(
        &mut self,
        modulation: Modulation,
        max_targets: usize,
    ) -> Result<Vec<Target>, Error> {
        ops::initiator::list_passive_targets(self.device, modulation, max_targets)
    }

    pub fn poll_target(
        &mut self,
        modulations: &[Modulation],
        iterations: PollIterations,
        period: PollPeriod,
    ) -> Result<Option<Target>, Error> {
        ops::initiator::poll_target(self.device, modulations, iterations, period)
    }
}

pub struct DepOps<'a> {
    device: &'a mut dyn DeviceHandle,
}

impl<'a> DepOps<'a> {
    pub fn init_secure_element(&mut self) -> Result<i32, Error> {
        ops::initiator::init_secure_element(self.device)
    }

    pub fn select_dep_target(
        &mut self,
        mode: DepMode,
        baud_rate: BaudRate,
        initiator: Option<&DepInfo>,
        timeout: OperationTimeout,
    ) -> Result<Option<Target>, Error> {
        ops::initiator::select_dep_target(self.device, mode, baud_rate, initiator, timeout)
    }

    pub fn poll_dep_target(
        &mut self,
        mode: DepMode,
        baud_rate: BaudRate,
        initiator: Option<&DepInfo>,
        timeout: OperationTimeout,
    ) -> Result<Option<Target>, Error> {
        ops::initiator::poll_dep_target(self.device, mode, baud_rate, initiator, timeout)
    }
}

pub struct SessionOps<'a> {
    device: &'a mut dyn DeviceHandle,
}

impl<'a> SessionOps<'a> {
    pub fn deselect_target(&mut self) -> Result<(), Error> {
        ops::initiator::deselect_target(self.device)
    }

    pub fn target_is_present(&mut self, target: Option<&Target>) -> Result<bool, Error> {
        ops::initiator::target_is_present(self.device, target)
    }

    pub fn abort_command(&mut self) -> Result<(), Error> {
        ops::initiator::abort_command(self.device)
    }

    pub fn idle(&mut self) -> Result<(), Error> {
        ops::initiator::idle(self.device)
    }

    pub fn powerdown(&mut self) -> Result<(), Error> {
        ops::initiator::powerdown(self.device)
    }
}

pub struct InitiatorIoOps<'a> {
    device: &'a mut dyn DeviceHandle,
}

impl<'a> InitiatorIoOps<'a> {
    pub fn transceive_bytes(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: OperationTimeout,
    ) -> Result<usize, Error> {
        ops::initiator::transceive_bytes(self.device, tx, rx, timeout)
    }

    pub fn transceive_bits(
        &mut self,
        tx: BitFrame<'_>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        ops::initiator::transceive_bits(self.device, tx, rx, rx_parity)
    }

    pub fn transceive_bytes_timed(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
    ) -> Result<(usize, u32), Error> {
        ops::initiator::transceive_bytes_timed(self.device, tx, rx)
    }

    pub fn transceive_bits_timed(
        &mut self,
        tx: BitFrame<'_>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), Error> {
        ops::initiator::transceive_bits_timed(self.device, tx, rx, rx_parity)
    }
}

pub struct TargetIoOps<'a> {
    device: &'a mut dyn DeviceHandle,
}

impl<'a> TargetIoOps<'a> {
    pub fn init(
        &mut self,
        target: &mut Target,
        rx: &mut [u8],
        timeout: OperationTimeout,
    ) -> Result<usize, Error> {
        ops::target::init(self.device, target, rx, timeout)
    }

    pub fn send_bytes(&mut self, tx: &[u8], timeout: OperationTimeout) -> Result<usize, Error> {
        ops::target::send_bytes(self.device, tx, timeout)
    }

    pub fn receive_bytes(
        &mut self,
        rx: &mut [u8],
        timeout: OperationTimeout,
    ) -> Result<usize, Error> {
        ops::target::receive_bytes(self.device, rx, timeout)
    }

    pub fn send_bits(&mut self, tx: BitFrame<'_>) -> Result<usize, Error> {
        ops::target::send_bits(self.device, tx)
    }

    pub fn receive_bits(
        &mut self,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        ops::target::receive_bits(self.device, rx, rx_parity)
    }
}

pub struct Pn53xOps<'a> {
    device: &'a mut dyn DeviceHandle,
}

impl<'a> Pn53xOps<'a> {
    pub fn transceive(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: OperationTimeout,
    ) -> Result<usize, Error> {
        ops::pn53x::transceive(self.device, tx, rx, timeout)
    }

    pub fn read_register(&mut self, register: u16) -> Result<u8, Error> {
        ops::pn53x::read_register(self.device, register)
    }

    pub fn write_register(
        &mut self,
        register: u16,
        symbol_mask: u8,
        value: u8,
    ) -> Result<(), Error> {
        ops::pn53x::write_register(self.device, register, symbol_mask, value)
    }

    pub fn sam_configuration(&mut self, mode: u8, timeout: OperationTimeout) -> Result<i32, Error> {
        ops::pn53x::sam_configuration(self.device, mode, timeout)
    }
}

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
    if !supported_modulations.contains(&modulation.modulation_type()) {
        return Err(Error::InvalidArgument("modulation not supported"));
    }

    let supported_baud_rates = device.supported_baud_rates(mode, modulation.modulation_type())?;
    if !supported_baud_rates.contains(&modulation.baud_rate()) {
        return Err(Error::InvalidArgument("baud rate not supported"));
    }

    Ok(())
}

pub(crate) fn default_initiator_payload(modulation: Modulation) -> &'static [u8] {
    match modulation.modulation_type() {
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
        modulation.modulation_type(),
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

    pub(super) mod info {
        use super::*;

        pub(crate) fn information_about<D>(device: &mut D) -> Result<String, Error>
        where
            D: InfoBackend + ?Sized,
        {
            device.information_about()
        }
    }

    pub(super) mod property {
        use super::*;

        pub(crate) fn set_property_bool<D>(
            device: &mut D,
            property: Property,
            enable: bool,
        ) -> Result<(), Error>
        where
            D: PropertyBackend + ?Sized,
        {
            device.set_property_bool(property, enable)
        }

        pub(crate) fn set_property_int<D>(
            device: &mut D,
            property: Property,
            value: i32,
        ) -> Result<(), Error>
        where
            D: PropertyBackend + ?Sized,
        {
            device.set_property_int(property, value)
        }

        pub(crate) fn supported_modulations<D>(
            device: &mut D,
            mode: Mode,
        ) -> Result<Vec<ModulationType>, Error>
        where
            D: PropertyBackend + ?Sized,
        {
            device.supported_modulations(mode)
        }

        pub(crate) fn supported_baud_rates<D>(
            device: &mut D,
            mode: Mode,
            modulation_type: ModulationType,
        ) -> Result<Vec<BaudRate>, Error>
        where
            D: PropertyBackend + ?Sized,
        {
            device.supported_baud_rates(mode, modulation_type)
        }
    }

    pub(super) mod initiator {
        use super::*;

        pub(crate) fn init<D>(device: &mut D) -> Result<i32, Error>
        where
            D: PropertyBackend + InitiatorBackend + ?Sized,
        {
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

        pub(crate) fn init_secure_element<D>(device: &mut D) -> Result<i32, Error>
        where
            D: InitiatorBackend + ?Sized,
        {
            device.initiator_init_secure_element_driver()
        }

        pub(crate) fn select_passive_target<D>(
            device: &mut D,
            nm: Modulation,
            init_data: Option<&[u8]>,
        ) -> Result<Option<Target>, Error>
        where
            D: PropertyBackend + InitiatorBackend + ?Sized,
        {
            validate_modulation(device, Mode::Initiator, nm)?;

            let payload = if init_data.is_some_and(|value| !value.is_empty()) {
                if nm.modulation_type() == ModulationType::Iso14443A {
                    cascade_iso14443a_uid(init_data.expect("checked above"))
                } else {
                    init_data.expect("checked above").to_vec()
                }
            } else {
                default_initiator_payload(nm).to_vec()
            };

            device.select_passive_target_driver(nm, &payload)
        }

        pub(crate) fn list_passive_targets<D>(
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

        pub(crate) fn poll_target<D>(
            device: &mut D,
            modulations: &[Modulation],
            iterations: PollIterations,
            period: PollPeriod,
        ) -> Result<Option<Target>, Error>
        where
            D: InitiatorBackend + ?Sized,
        {
            device.poll_target_driver(modulations, iterations, period)
        }

        pub(crate) fn select_dep_target<D>(
            device: &mut D,
            ndm: DepMode,
            nbr: BaudRate,
            initiator: Option<&DepInfo>,
            timeout: OperationTimeout,
        ) -> Result<Option<Target>, Error>
        where
            D: InitiatorBackend + ?Sized,
        {
            device.select_dep_target_driver(ndm, nbr, initiator, timeout)
        }

        pub(crate) fn poll_dep_target<D>(
            device: &mut D,
            ndm: DepMode,
            nbr: BaudRate,
            initiator: Option<&DepInfo>,
            timeout: OperationTimeout,
        ) -> Result<Option<Target>, Error>
        where
            D: PropertyBackend + InitiatorBackend + ?Sized,
        {
            let previous = device.property_bool_state(Property::InfiniteSelect);
            device.set_property_bool(Property::InfiniteSelect, true)?;

            let result = (|| {
                let mut remaining = timeout.finite_millis().unwrap_or(0);
                while remaining > 0 {
                    match select_dep_target(
                        device,
                        ndm,
                        nbr,
                        initiator,
                        OperationTimeout::try_milliseconds(POLL_DEP_PERIOD_MS as u32)
                            .expect("poll period is a valid finite timeout"),
                    ) {
                        Ok(Some(target)) => return Ok(Some(target)),
                        Ok(None) => remaining = remaining.saturating_sub(POLL_DEP_PERIOD_MS as u32),
                        Err(error) if error.device_code() == Some(-6) => {
                            remaining = remaining.saturating_sub(POLL_DEP_PERIOD_MS as u32);
                        }
                        Err(error) => return Err(error),
                    }
                }
                Ok(None)
            })();

            restore_property_bool(device, Property::InfiniteSelect, previous, true)?;
            result
        }

        pub(crate) fn deselect_target<D>(device: &mut D) -> Result<(), Error>
        where
            D: InitiatorBackend + ?Sized,
        {
            device.deselect_target_driver()
        }

        pub(crate) fn target_is_present<D>(
            device: &mut D,
            target: Option<&Target>,
        ) -> Result<bool, Error>
        where
            D: InitiatorBackend + ?Sized,
        {
            device.target_is_present_driver(target)
        }

        pub(crate) fn transceive_bytes<D>(
            device: &mut D,
            tx: &[u8],
            rx: &mut [u8],
            timeout: OperationTimeout,
        ) -> Result<usize, Error>
        where
            D: InitiatorBackend + ?Sized,
        {
            device.transceive_bytes_driver(tx, rx, timeout)
        }

        pub(crate) fn transceive_bits<D>(
            device: &mut D,
            tx: BitFrame<'_>,
            rx: &mut [u8],
            rx_parity: Option<&mut [u8]>,
        ) -> Result<usize, Error>
        where
            D: InitiatorBackend + ?Sized,
        {
            device.transceive_bits_driver(tx, rx, rx_parity)
        }

        pub(crate) fn transceive_bytes_timed<D>(
            device: &mut D,
            tx: &[u8],
            rx: &mut [u8],
        ) -> Result<(usize, u32), Error>
        where
            D: InitiatorBackend + ?Sized,
        {
            device.transceive_bytes_timed_driver(tx, rx)
        }

        pub(crate) fn transceive_bits_timed<D>(
            device: &mut D,
            tx: BitFrame<'_>,
            rx: &mut [u8],
            rx_parity: Option<&mut [u8]>,
        ) -> Result<(usize, u32), Error>
        where
            D: InitiatorBackend + ?Sized,
        {
            device.transceive_bits_timed_driver(tx, rx, rx_parity)
        }

        pub(crate) fn abort_command<D>(device: &mut D) -> Result<(), Error>
        where
            D: InitiatorBackend + ?Sized,
        {
            device.abort_command_driver()
        }

        pub(crate) fn idle<D>(device: &mut D) -> Result<(), Error>
        where
            D: InitiatorBackend + ?Sized,
        {
            device.idle_driver()
        }

        pub(crate) fn powerdown<D>(device: &mut D) -> Result<(), Error>
        where
            D: InitiatorBackend + ?Sized,
        {
            device.powerdown_driver()
        }
    }

    pub(super) mod target {
        use super::*;

        pub(crate) fn init<D>(
            device: &mut D,
            target: &mut Target,
            rx: &mut [u8],
            timeout: OperationTimeout,
        ) -> Result<usize, Error>
        where
            D: PropertyBackend + TargetBackend + ?Sized,
        {
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

        pub(crate) fn send_bytes<D>(
            device: &mut D,
            tx: &[u8],
            timeout: OperationTimeout,
        ) -> Result<usize, Error>
        where
            D: TargetBackend + ?Sized,
        {
            device.target_send_bytes_driver(tx, timeout)
        }

        pub(crate) fn receive_bytes<D>(
            device: &mut D,
            rx: &mut [u8],
            timeout: OperationTimeout,
        ) -> Result<usize, Error>
        where
            D: TargetBackend + ?Sized,
        {
            device.target_receive_bytes_driver(rx, timeout)
        }

        pub(crate) fn send_bits<D>(device: &mut D, tx: BitFrame<'_>) -> Result<usize, Error>
        where
            D: TargetBackend + ?Sized,
        {
            device.target_send_bits_driver(tx)
        }

        pub(crate) fn receive_bits<D>(
            device: &mut D,
            rx: &mut [u8],
            rx_parity: Option<&mut [u8]>,
        ) -> Result<usize, Error>
        where
            D: TargetBackend + ?Sized,
        {
            device.target_receive_bits_driver(rx, rx_parity)
        }
    }

    pub(super) mod pn53x {
        use super::*;

        pub(crate) fn transceive<D>(
            device: &mut D,
            tx: &[u8],
            rx: &mut [u8],
            timeout: OperationTimeout,
        ) -> Result<usize, Error>
        where
            D: Pn53xBackend + ?Sized,
        {
            device.pn53x_transceive_driver(tx, rx, timeout)
        }

        pub(crate) fn read_register<D>(device: &mut D, register: u16) -> Result<u8, Error>
        where
            D: Pn53xBackend + ?Sized,
        {
            device.pn53x_read_register_driver(register)
        }

        pub(crate) fn write_register<D>(
            device: &mut D,
            register: u16,
            symbol_mask: u8,
            value: u8,
        ) -> Result<(), Error>
        where
            D: Pn53xBackend + ?Sized,
        {
            device.pn53x_write_register_driver(register, symbol_mask, value)
        }

        pub(crate) fn sam_configuration<D>(
            device: &mut D,
            mode: u8,
            timeout: OperationTimeout,
        ) -> Result<i32, Error>
        where
            D: Pn53xBackend + ?Sized,
        {
            device.pn532_sam_configuration_driver(mode, timeout)
        }
    }
}
