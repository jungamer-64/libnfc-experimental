use std::any::Any;

use super::device::{
    OpenedDevice, POLL_DEP_PERIOD_MS, apply_bool_property_sequence, cascade_iso14443a_uid,
    default_initiator_payload, modulation_requires_single_attempt, restore_property_bool,
    validate_modulation,
};
use super::driver::Driver;
use super::{
    BaudRate, ConnectionString, Context, DepInfo, DepMode, DeviceCaps, DriverCaps, Error, Mode,
    Modulation, ModulationType, Property, ScanType, Target, device_error_message,
};

const DEFAULT_SCAN_CAPACITY: usize = 4;
const MAX_SCAN_CAPACITY: usize = 256;

#[doc(hidden)]
pub trait DriverBackend: Send + Sync {
    fn name(&self) -> &str;
    fn scan_type(&self) -> ScanType;
    fn caps(&self) -> DriverCaps {
        let mut caps = DriverCaps::OPEN;
        if self.scan_type() != ScanType::NotAvailable {
            caps |= DriverCaps::SCAN;
        }
        caps
    }
    fn scan_with_capacity(
        &self,
        context: &Context,
        capacity: usize,
    ) -> Result<Vec<ConnectionString>, Error>;
    fn open(
        &self,
        context: &Context,
        connstring: &ConnectionString,
    ) -> Result<Box<dyn DeviceBackend>, Error>;
}

#[doc(hidden)]
pub trait DeviceBackend: Send + Any {
    fn name(&self) -> &str;
    fn connstring(&self) -> &ConnectionString;
    fn caps(&self) -> DeviceCaps {
        DeviceCaps::NONE
    }
    fn last_error(&self) -> i32;
    fn set_last_error(&mut self, value: i32);

    fn unsupported_error_code(&self) -> i32 {
        -3
    }

    fn strerror_backend(&self) -> Option<String> {
        None
    }

    fn information_about_backend(&mut self) -> Result<String, Error> {
        Err(Error::UnsupportedOperation("device_get_information_about"))
    }

    fn set_property_bool_backend(
        &mut self,
        _property: Property,
        _enable: bool,
    ) -> Result<(), Error> {
        Err(Error::UnsupportedOperation("device_set_property_bool"))
    }

    fn set_property_int_backend(&mut self, _property: Property, _value: i32) -> Result<(), Error> {
        Err(Error::UnsupportedOperation("device_set_property_int"))
    }

    fn supported_modulations_backend(&mut self, _mode: Mode) -> Result<Vec<ModulationType>, Error> {
        Err(Error::UnsupportedOperation("get_supported_modulation"))
    }

    fn supported_baud_rates_backend(
        &mut self,
        _mode: Mode,
        _modulation_type: ModulationType,
    ) -> Result<Vec<BaudRate>, Error> {
        Err(Error::UnsupportedOperation("get_supported_baud_rate"))
    }

    fn property_bool_state(&self, _property: Property) -> Option<bool> {
        None
    }

    fn initiator_init_backend(&mut self) -> Result<i32, Error> {
        Err(Error::UnsupportedOperation("initiator_init"))
    }

    fn initiator_init_secure_element_backend(&mut self) -> Result<i32, Error> {
        Err(Error::UnsupportedOperation("initiator_init_secure_element"))
    }

    fn select_passive_target_backend(
        &mut self,
        _nm: Modulation,
        _init_data: &[u8],
    ) -> Result<Option<Target>, Error> {
        Err(Error::UnsupportedOperation(
            "initiator_select_passive_target",
        ))
    }

    fn poll_target_backend(
        &mut self,
        _modulations: &[Modulation],
        _poll_nr: u8,
        _period: u8,
    ) -> Result<Option<Target>, Error> {
        Err(Error::UnsupportedOperation("initiator_poll_target"))
    }

    fn select_dep_target_backend(
        &mut self,
        _ndm: DepMode,
        _nbr: BaudRate,
        _initiator: Option<&DepInfo>,
        _timeout: i32,
    ) -> Result<Option<Target>, Error> {
        Err(Error::UnsupportedOperation("initiator_select_dep_target"))
    }

    fn deselect_target_backend(&mut self) -> Result<(), Error> {
        Err(Error::UnsupportedOperation("initiator_deselect_target"))
    }

    fn target_is_present_backend(&mut self, _target: Option<&Target>) -> Result<bool, Error> {
        Err(Error::UnsupportedOperation("initiator_target_is_present"))
    }

    fn target_init_backend(
        &mut self,
        _target: &mut Target,
        _rx: &mut [u8],
        _timeout: i32,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("target_init"))
    }

    fn transceive_bytes_backend(
        &mut self,
        _tx: &[u8],
        _rx: &mut [u8],
        _timeout: i32,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("initiator_transceive_bytes"))
    }

    fn transceive_bits_backend(
        &mut self,
        _tx: &[u8],
        _tx_bits_len: usize,
        _tx_parity: Option<&[u8]>,
        _rx: &mut [u8],
        _rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("initiator_transceive_bits"))
    }

    fn transceive_bytes_timed_backend(
        &mut self,
        _tx: &[u8],
        _rx: &mut [u8],
    ) -> Result<(usize, u32), Error> {
        Err(Error::UnsupportedOperation(
            "initiator_transceive_bytes_timed",
        ))
    }

    fn transceive_bits_timed_backend(
        &mut self,
        _tx: &[u8],
        _tx_bits_len: usize,
        _tx_parity: Option<&[u8]>,
        _rx: &mut [u8],
        _rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), Error> {
        Err(Error::UnsupportedOperation(
            "initiator_transceive_bits_timed",
        ))
    }

    fn target_send_bytes_backend(&mut self, _tx: &[u8], _timeout: i32) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("target_send_bytes"))
    }

    fn target_receive_bytes_backend(
        &mut self,
        _rx: &mut [u8],
        _timeout: i32,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("target_receive_bytes"))
    }

    fn target_send_bits_backend(
        &mut self,
        _tx: &[u8],
        _tx_bits_len: usize,
        _tx_parity: Option<&[u8]>,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("target_send_bits"))
    }

    fn target_receive_bits_backend(
        &mut self,
        _rx: &mut [u8],
        _rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("target_receive_bits"))
    }

    fn abort_command_backend(&mut self) -> Result<(), Error> {
        Err(Error::UnsupportedOperation("abort_command"))
    }

    fn idle_backend(&mut self) -> Result<(), Error> {
        Err(Error::UnsupportedOperation("idle"))
    }

    fn powerdown_backend(&mut self) -> Result<(), Error> {
        Err(Error::UnsupportedOperation("powerdown"))
    }

    fn pn53x_transceive_backend(
        &mut self,
        _tx: &[u8],
        _rx: &mut [u8],
        _timeout: i32,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("pn53x_transceive"))
    }

    fn pn53x_read_register_backend(&mut self, _register: u16) -> Result<u8, Error> {
        Err(Error::UnsupportedOperation("pn53x_read_register"))
    }

    fn pn53x_write_register_backend(
        &mut self,
        _register: u16,
        _symbol_mask: u8,
        _value: u8,
    ) -> Result<(), Error> {
        Err(Error::UnsupportedOperation("pn53x_write_register"))
    }

    fn pn532_sam_configuration_backend(&mut self, _mode: u8, _timeout: i32) -> Result<i32, Error> {
        Err(Error::UnsupportedOperation("pn532_SAMConfiguration"))
    }

    fn into_native_payload(self: Box<Self>) -> Option<Box<dyn Any + Send>> {
        None
    }
}

fn missing_capability(operation: &'static str) -> Error {
    Error::MissingCapability(operation)
}

#[doc(hidden)]
pub fn wrap_driver_backend(backend: Box<dyn DriverBackend>) -> Box<dyn Driver> {
    Box::new(BackendDriver::new(backend))
}

#[doc(hidden)]
pub fn wrap_device_backend(backend: Box<dyn DeviceBackend>) -> Box<dyn OpenedDevice> {
    Box::new(BackendOpenedDevice::new(backend))
}

struct BackendDriver {
    backend: Box<dyn DriverBackend>,
    name: String,
    scan_type: ScanType,
    caps: DriverCaps,
}

impl BackendDriver {
    fn new(backend: Box<dyn DriverBackend>) -> Self {
        let name = backend.name().to_string();
        let scan_type = backend.scan_type();
        let caps = backend.caps();
        Self {
            backend,
            name,
            scan_type,
            caps,
        }
    }
}

impl Driver for BackendDriver {
    fn name(&self) -> &str {
        &self.name
    }

    fn scan_type(&self) -> ScanType {
        self.scan_type
    }

    fn caps(&self) -> DriverCaps {
        self.caps
    }

    fn scan(&self, context: &Context) -> Result<Vec<ConnectionString>, Error> {
        if !self.caps.contains(DriverCaps::SCAN) {
            return Err(missing_capability("scan"));
        }
        let mut capacity = DEFAULT_SCAN_CAPACITY;
        loop {
            let devices = self.backend.scan_with_capacity(context, capacity)?;
            if devices.len() < capacity || capacity >= MAX_SCAN_CAPACITY {
                return Ok(devices);
            }
            capacity = (capacity * 2).min(MAX_SCAN_CAPACITY);
        }
    }

    fn open(
        &self,
        context: &Context,
        connstring: &ConnectionString,
    ) -> Result<Box<dyn OpenedDevice>, Error> {
        if !self.caps.contains(DriverCaps::OPEN) {
            return Err(missing_capability("open"));
        }
        Ok(wrap_device_backend(self.backend.open(context, connstring)?))
    }
}

struct BackendOpenedDevice {
    backend: Box<dyn DeviceBackend>,
}

impl BackendOpenedDevice {
    fn new(backend: Box<dyn DeviceBackend>) -> Self {
        Self { backend }
    }

    fn missing_capability<T>(&mut self, operation: &'static str) -> Result<T, Error> {
        self.backend
            .set_last_error(self.backend.unsupported_error_code());
        Err(missing_capability(operation))
    }

    fn require_caps(&mut self, required: DeviceCaps, operation: &'static str) -> Result<(), Error> {
        if self.backend.caps().contains(required) {
            Ok(())
        } else {
            self.missing_capability(operation)
        }
    }

    fn normalize_result<T>(
        &mut self,
        required: DeviceCaps,
        operation: &'static str,
        f: impl FnOnce(&mut dyn DeviceBackend) -> Result<T, Error>,
    ) -> Result<T, Error> {
        self.require_caps(required, operation)?;
        let result = f(self.backend.as_mut());
        match result {
            Ok(value) => {
                self.backend.set_last_error(0);
                Ok(value)
            }
            Err(Error::UnsupportedOperation(_)) => self.missing_capability(operation),
            Err(error @ Error::DeviceOperationFailed { code, .. }) => {
                self.backend.set_last_error(code);
                Err(error)
            }
            Err(error) => Err(error),
        }
    }
}

impl OpenedDevice for BackendOpenedDevice {
    fn name(&self) -> &str {
        self.backend.name()
    }

    fn connstring(&self) -> &ConnectionString {
        self.backend.connstring()
    }

    fn caps(&self) -> DeviceCaps {
        self.backend.caps()
    }

    fn last_error(&self) -> i32 {
        self.backend.last_error()
    }

    fn strerror(&self) -> String {
        self.backend
            .strerror_backend()
            .unwrap_or_else(|| device_error_message(self.last_error()).to_string())
    }

    fn information_about(&mut self) -> Result<String, Error> {
        self.normalize_result(
            DeviceCaps::INFO,
            "device_get_information_about",
            |backend| backend.information_about_backend(),
        )
    }

    fn set_property_bool(&mut self, property: Property, enable: bool) -> Result<(), Error> {
        self.normalize_result(
            DeviceCaps::SET_PROPERTY_BOOL,
            "device_set_property_bool",
            |backend| backend.set_property_bool_backend(property, enable),
        )
    }

    fn set_property_int(&mut self, property: Property, value: i32) -> Result<(), Error> {
        self.normalize_result(
            DeviceCaps::SET_PROPERTY_INT,
            "device_set_property_int",
            |backend| backend.set_property_int_backend(property, value),
        )
    }

    fn supported_modulations(&mut self, mode: Mode) -> Result<Vec<ModulationType>, Error> {
        self.normalize_result(
            DeviceCaps::SUPPORTED_MODULATIONS,
            "get_supported_modulation",
            |backend| backend.supported_modulations_backend(mode),
        )
    }

    fn supported_baud_rates(
        &mut self,
        mode: Mode,
        modulation_type: ModulationType,
    ) -> Result<Vec<BaudRate>, Error> {
        self.normalize_result(
            DeviceCaps::SUPPORTED_BAUD_RATES,
            "get_supported_baud_rate",
            |backend| backend.supported_baud_rates_backend(mode, modulation_type),
        )
    }

    fn initiator_init(&mut self) -> Result<i32, Error> {
        self.require_caps(
            DeviceCaps::SET_PROPERTY_BOOL | DeviceCaps::INITIATOR_INIT,
            "initiator_init",
        )?;
        apply_bool_property_sequence(
            self,
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
        self.initiator_init_driver()
    }

    fn initiator_init_secure_element(&mut self) -> Result<i32, Error> {
        self.initiator_init_secure_element_driver()
    }

    fn select_passive_target(
        &mut self,
        nm: Modulation,
        init_data: Option<&[u8]>,
    ) -> Result<Option<Target>, Error> {
        self.require_caps(
            DeviceCaps::SUPPORTED_MODULATIONS
                | DeviceCaps::SUPPORTED_BAUD_RATES
                | DeviceCaps::SELECT_PASSIVE_TARGET,
            "initiator_select_passive_target",
        )?;
        validate_modulation(self, Mode::Initiator, nm)?;

        let payload = if init_data.is_some_and(|value| !value.is_empty()) {
            if nm.modulation_type == ModulationType::Iso14443A {
                cascade_iso14443a_uid(init_data.expect("checked above"))
            } else {
                init_data.expect("checked above").to_vec()
            }
        } else {
            default_initiator_payload(nm).to_vec()
        };

        self.select_passive_target_driver(nm, &payload)
    }

    fn list_passive_targets(
        &mut self,
        nm: Modulation,
        max_targets: usize,
    ) -> Result<Vec<Target>, Error> {
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
        self.require_caps(required, "list_passive_targets")?;

        let previous = self.property_bool_state(Property::InfiniteSelect);
        self.set_property_bool(Property::InfiniteSelect, false)?;

        let result = (|| {
            let mut targets = Vec::new();
            while let Some(target) = self.select_passive_target(nm, None)? {
                if targets.contains(&target) {
                    break;
                }

                targets.push(target);
                if targets.len() >= max_targets || modulation_requires_single_attempt(nm) {
                    break;
                }

                self.deselect_target()?;
            }
            Ok(targets)
        })();

        restore_property_bool(self, Property::InfiniteSelect, previous, false)?;
        result
    }

    fn poll_target(
        &mut self,
        modulations: &[Modulation],
        poll_nr: u8,
        period: u8,
    ) -> Result<Option<Target>, Error> {
        self.poll_target_driver(modulations, poll_nr, period)
    }

    fn select_dep_target(
        &mut self,
        ndm: DepMode,
        nbr: BaudRate,
        initiator: Option<&DepInfo>,
        timeout: i32,
    ) -> Result<Option<Target>, Error> {
        self.select_dep_target_driver(ndm, nbr, initiator, timeout)
    }

    fn poll_dep_target(
        &mut self,
        ndm: DepMode,
        nbr: BaudRate,
        initiator: Option<&DepInfo>,
        timeout: i32,
    ) -> Result<Option<Target>, Error> {
        self.require_caps(
            DeviceCaps::SET_PROPERTY_BOOL | DeviceCaps::SELECT_DEP_TARGET,
            "poll_dep_target",
        )?;
        let previous = self.property_bool_state(Property::InfiniteSelect);
        self.set_property_bool(Property::InfiniteSelect, true)?;

        let result = (|| {
            let mut remaining = timeout;
            while remaining > 0 {
                match self.select_dep_target(ndm, nbr, initiator, POLL_DEP_PERIOD_MS) {
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

        restore_property_bool(self, Property::InfiniteSelect, previous, true)?;
        result
    }

    fn property_bool_state(&self, property: Property) -> Option<bool> {
        self.backend.property_bool_state(property)
    }

    fn initiator_init_driver(&mut self) -> Result<i32, Error> {
        self.normalize_result(DeviceCaps::INITIATOR_INIT, "initiator_init", |backend| {
            backend.initiator_init_backend()
        })
    }

    fn initiator_init_secure_element_driver(&mut self) -> Result<i32, Error> {
        self.normalize_result(
            DeviceCaps::INITIATOR_INIT_SECURE_ELEMENT,
            "initiator_init_secure_element",
            |backend| backend.initiator_init_secure_element_backend(),
        )
    }

    fn select_passive_target_driver(
        &mut self,
        nm: Modulation,
        init_data: &[u8],
    ) -> Result<Option<Target>, Error> {
        self.normalize_result(
            DeviceCaps::SELECT_PASSIVE_TARGET,
            "initiator_select_passive_target",
            |backend| backend.select_passive_target_backend(nm, init_data),
        )
    }

    fn poll_target_driver(
        &mut self,
        modulations: &[Modulation],
        poll_nr: u8,
        period: u8,
    ) -> Result<Option<Target>, Error> {
        self.normalize_result(
            DeviceCaps::POLL_TARGET,
            "initiator_poll_target",
            |backend| backend.poll_target_backend(modulations, poll_nr, period),
        )
    }

    fn select_dep_target_driver(
        &mut self,
        ndm: DepMode,
        nbr: BaudRate,
        initiator: Option<&DepInfo>,
        timeout: i32,
    ) -> Result<Option<Target>, Error> {
        self.normalize_result(
            DeviceCaps::SELECT_DEP_TARGET,
            "initiator_select_dep_target",
            |backend| backend.select_dep_target_backend(ndm, nbr, initiator, timeout),
        )
    }

    fn deselect_target_driver(&mut self) -> Result<(), Error> {
        self.normalize_result(
            DeviceCaps::DESELECT_TARGET,
            "initiator_deselect_target",
            |backend| backend.deselect_target_backend(),
        )
    }

    fn deselect_target(&mut self) -> Result<(), Error> {
        self.deselect_target_driver()
    }

    fn target_is_present_driver(&mut self, target: Option<&Target>) -> Result<bool, Error> {
        self.normalize_result(
            DeviceCaps::TARGET_IS_PRESENT,
            "initiator_target_is_present",
            |backend| backend.target_is_present_backend(target),
        )
    }

    fn target_is_present(&mut self, target: Option<&Target>) -> Result<bool, Error> {
        self.target_is_present_driver(target)
    }

    fn target_init_driver(
        &mut self,
        target: &mut Target,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error> {
        self.normalize_result(DeviceCaps::TARGET_INIT, "target_init", |backend| {
            backend.target_init_backend(target, rx, timeout)
        })
    }

    fn target_init(
        &mut self,
        target: &mut Target,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error> {
        self.require_caps(
            DeviceCaps::SET_PROPERTY_BOOL | DeviceCaps::TARGET_INIT,
            "target_init",
        )?;
        apply_bool_property_sequence(
            self,
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
        self.target_init_driver(target, rx, timeout)
    }

    fn transceive_bytes_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error> {
        self.normalize_result(
            DeviceCaps::TRANSCEIVE_BYTES,
            "initiator_transceive_bytes",
            |backend| backend.transceive_bytes_backend(tx, rx, timeout),
        )
    }

    fn transceive_bytes(&mut self, tx: &[u8], rx: &mut [u8], timeout: i32) -> Result<usize, Error> {
        self.transceive_bytes_driver(tx, rx, timeout)
    }

    fn transceive_bits_driver(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        self.normalize_result(
            DeviceCaps::TRANSCEIVE_BITS,
            "initiator_transceive_bits",
            |backend| backend.transceive_bits_backend(tx, tx_bits_len, tx_parity, rx, rx_parity),
        )
    }

    fn transceive_bits(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        self.transceive_bits_driver(tx, tx_bits_len, tx_parity, rx, rx_parity)
    }

    fn transceive_bytes_timed_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
    ) -> Result<(usize, u32), Error> {
        self.normalize_result(
            DeviceCaps::TRANSCEIVE_BYTES_TIMED,
            "initiator_transceive_bytes_timed",
            |backend| backend.transceive_bytes_timed_backend(tx, rx),
        )
    }

    fn transceive_bytes_timed(&mut self, tx: &[u8], rx: &mut [u8]) -> Result<(usize, u32), Error> {
        self.transceive_bytes_timed_driver(tx, rx)
    }

    fn transceive_bits_timed_driver(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), Error> {
        self.normalize_result(
            DeviceCaps::TRANSCEIVE_BITS_TIMED,
            "initiator_transceive_bits_timed",
            |backend| {
                backend.transceive_bits_timed_backend(tx, tx_bits_len, tx_parity, rx, rx_parity)
            },
        )
    }

    fn transceive_bits_timed(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), Error> {
        self.transceive_bits_timed_driver(tx, tx_bits_len, tx_parity, rx, rx_parity)
    }

    fn target_send_bytes_driver(&mut self, tx: &[u8], timeout: i32) -> Result<usize, Error> {
        self.normalize_result(
            DeviceCaps::TARGET_SEND_BYTES,
            "target_send_bytes",
            |backend| backend.target_send_bytes_backend(tx, timeout),
        )
    }

    fn target_send_bytes(&mut self, tx: &[u8], timeout: i32) -> Result<usize, Error> {
        self.target_send_bytes_driver(tx, timeout)
    }

    fn target_receive_bytes_driver(&mut self, rx: &mut [u8], timeout: i32) -> Result<usize, Error> {
        self.normalize_result(
            DeviceCaps::TARGET_RECEIVE_BYTES,
            "target_receive_bytes",
            |backend| backend.target_receive_bytes_backend(rx, timeout),
        )
    }

    fn target_receive_bytes(&mut self, rx: &mut [u8], timeout: i32) -> Result<usize, Error> {
        self.target_receive_bytes_driver(rx, timeout)
    }

    fn target_send_bits_driver(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
    ) -> Result<usize, Error> {
        self.normalize_result(
            DeviceCaps::TARGET_SEND_BITS,
            "target_send_bits",
            |backend| backend.target_send_bits_backend(tx, tx_bits_len, tx_parity),
        )
    }

    fn target_send_bits(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
    ) -> Result<usize, Error> {
        self.target_send_bits_driver(tx, tx_bits_len, tx_parity)
    }

    fn target_receive_bits_driver(
        &mut self,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        self.normalize_result(
            DeviceCaps::TARGET_RECEIVE_BITS,
            "target_receive_bits",
            |backend| backend.target_receive_bits_backend(rx, rx_parity),
        )
    }

    fn target_receive_bits(
        &mut self,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        self.target_receive_bits_driver(rx, rx_parity)
    }

    fn abort_command_driver(&mut self) -> Result<(), Error> {
        self.normalize_result(DeviceCaps::ABORT_COMMAND, "abort_command", |backend| {
            backend.abort_command_backend()
        })
    }

    fn abort_command(&mut self) -> Result<(), Error> {
        self.abort_command_driver()
    }

    fn idle_driver(&mut self) -> Result<(), Error> {
        self.normalize_result(DeviceCaps::IDLE, "idle", |backend| backend.idle_backend())
    }

    fn idle(&mut self) -> Result<(), Error> {
        self.idle_driver()
    }

    fn powerdown_driver(&mut self) -> Result<(), Error> {
        self.normalize_result(DeviceCaps::POWERDOWN, "powerdown", |backend| {
            backend.powerdown_backend()
        })
    }

    fn powerdown(&mut self) -> Result<(), Error> {
        self.powerdown_driver()
    }

    fn pn53x_transceive_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error> {
        self.normalize_result(
            DeviceCaps::PN53X_TRANSCEIVE,
            "pn53x_transceive",
            |backend| backend.pn53x_transceive_backend(tx, rx, timeout),
        )
    }

    fn pn53x_read_register_driver(&mut self, register: u16) -> Result<u8, Error> {
        self.normalize_result(
            DeviceCaps::PN53X_READ_REGISTER,
            "pn53x_read_register",
            |backend| backend.pn53x_read_register_backend(register),
        )
    }

    fn pn53x_write_register_driver(
        &mut self,
        register: u16,
        symbol_mask: u8,
        value: u8,
    ) -> Result<(), Error> {
        self.normalize_result(
            DeviceCaps::PN53X_WRITE_REGISTER,
            "pn53x_write_register",
            |backend| backend.pn53x_write_register_backend(register, symbol_mask, value),
        )
    }

    fn pn532_sam_configuration_driver(&mut self, mode: u8, timeout: i32) -> Result<i32, Error> {
        self.normalize_result(
            DeviceCaps::PN532_SAM_CONFIGURATION,
            "pn532_SAMConfiguration",
            |backend| backend.pn532_sam_configuration_backend(mode, timeout),
        )
    }

    fn into_native_payload(self: Box<Self>) -> Option<Box<dyn Any + Send>> {
        self.backend.into_native_payload()
    }
}
