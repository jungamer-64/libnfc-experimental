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

pub trait Logger: Send + Sync {
    fn log(&self, _priority: u8, _message: &str) {}
}

pub trait OpenedDevice: Send + Any {
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

    fn information_about(&mut self) -> Result<String, Error> {
        Err(Error::UnsupportedOperation("information_about"))
    }

    fn set_property_bool(&mut self, property: Property, enable: bool) -> Result<(), Error>;

    fn set_property_int(&mut self, property: Property, value: i32) -> Result<(), Error>;

    fn supported_modulations(&mut self, mode: Mode) -> Result<Vec<ModulationType>, Error>;

    fn supported_baud_rates(
        &mut self,
        mode: Mode,
        modulation_type: ModulationType,
    ) -> Result<Vec<BaudRate>, Error>;

    #[doc(hidden)]
    fn property_bool_state(&self, _property: Property) -> Option<bool> {
        None
    }

    #[doc(hidden)]
    fn initiator_init_driver(&mut self) -> Result<i32, Error> {
        Err(Error::UnsupportedOperation("initiator_init"))
    }

    #[doc(hidden)]
    fn initiator_init_secure_element_driver(&mut self) -> Result<i32, Error> {
        Err(Error::UnsupportedOperation("initiator_init_secure_element"))
    }

    #[doc(hidden)]
    fn select_passive_target_driver(
        &mut self,
        _nm: Modulation,
        _init_data: &[u8],
    ) -> Result<Option<Target>, Error> {
        Err(Error::UnsupportedOperation("select_passive_target"))
    }

    #[doc(hidden)]
    fn poll_target_driver(
        &mut self,
        _modulations: &[Modulation],
        _poll_nr: u8,
        _period: u8,
    ) -> Result<Option<Target>, Error> {
        Err(Error::UnsupportedOperation("poll_target"))
    }

    #[doc(hidden)]
    fn select_dep_target_driver(
        &mut self,
        _ndm: DepMode,
        _nbr: BaudRate,
        _initiator: Option<&DepInfo>,
        _timeout: i32,
    ) -> Result<Option<Target>, Error> {
        Err(Error::UnsupportedOperation("select_dep_target"))
    }

    #[doc(hidden)]
    fn deselect_target_driver(&mut self) -> Result<(), Error> {
        Err(Error::UnsupportedOperation("deselect_target"))
    }

    #[doc(hidden)]
    fn target_is_present_driver(&mut self, _target: Option<&Target>) -> Result<bool, Error> {
        Err(Error::UnsupportedOperation("target_is_present"))
    }

    #[doc(hidden)]
    fn target_init_driver(
        &mut self,
        _target: &mut Target,
        _rx: &mut [u8],
        _timeout: i32,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("target_init"))
    }

    #[doc(hidden)]
    fn transceive_bytes_driver(
        &mut self,
        _tx: &[u8],
        _rx: &mut [u8],
        _timeout: i32,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("transceive_bytes"))
    }

    #[doc(hidden)]
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

    #[doc(hidden)]
    fn transceive_bytes_timed_driver(
        &mut self,
        _tx: &[u8],
        _rx: &mut [u8],
    ) -> Result<(usize, u32), Error> {
        Err(Error::UnsupportedOperation("transceive_bytes_timed"))
    }

    #[doc(hidden)]
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

    #[doc(hidden)]
    fn target_send_bytes_driver(&mut self, _tx: &[u8], _timeout: i32) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("target_send_bytes"))
    }

    #[doc(hidden)]
    fn target_receive_bytes_driver(
        &mut self,
        _rx: &mut [u8],
        _timeout: i32,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("target_receive_bytes"))
    }

    #[doc(hidden)]
    fn target_send_bits_driver(
        &mut self,
        _tx: &[u8],
        _tx_bits_len: usize,
        _tx_parity: Option<&[u8]>,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("target_send_bits"))
    }

    #[doc(hidden)]
    fn target_receive_bits_driver(
        &mut self,
        _rx: &mut [u8],
        _rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("target_receive_bits"))
    }

    #[doc(hidden)]
    fn abort_command_driver(&mut self) -> Result<(), Error> {
        Err(Error::UnsupportedOperation("abort_command"))
    }

    #[doc(hidden)]
    fn idle_driver(&mut self) -> Result<(), Error> {
        Err(Error::UnsupportedOperation("idle"))
    }

    #[doc(hidden)]
    fn powerdown_driver(&mut self) -> Result<(), Error> {
        Err(Error::UnsupportedOperation("powerdown"))
    }

    #[doc(hidden)]
    fn pn53x_transceive_driver(
        &mut self,
        _tx: &[u8],
        _rx: &mut [u8],
        _timeout: i32,
    ) -> Result<usize, Error> {
        Err(Error::UnsupportedOperation("pn53x_transceive"))
    }

    #[doc(hidden)]
    fn pn53x_read_register_driver(&mut self, _register: u16) -> Result<u8, Error> {
        Err(Error::UnsupportedOperation("pn53x_read_register"))
    }

    #[doc(hidden)]
    fn pn53x_write_register_driver(
        &mut self,
        _register: u16,
        _symbol_mask: u8,
        _value: u8,
    ) -> Result<(), Error> {
        Err(Error::UnsupportedOperation("pn53x_write_register"))
    }

    #[doc(hidden)]
    fn pn532_sam_configuration_driver(&mut self, _mode: u8, _timeout: i32) -> Result<i32, Error> {
        Err(Error::UnsupportedOperation("pn532_SAMConfiguration"))
    }

    #[doc(hidden)]
    fn into_native_payload(self: Box<Self>) -> Option<Box<dyn Any + Send>> {
        None
    }

    fn initiator_init(&mut self) -> Result<i32, Error> {
        ensure_device_caps(
            self.caps(),
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
        ensure_device_caps(
            self.caps(),
            DeviceCaps::INITIATOR_INIT_SECURE_ELEMENT,
            "initiator_init_secure_element",
        )?;
        self.initiator_init_secure_element_driver()
    }

    fn select_passive_target(
        &mut self,
        nm: Modulation,
        init_data: Option<&[u8]>,
    ) -> Result<Option<Target>, Error> {
        ensure_device_caps(
            self.caps(),
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
        ensure_device_caps(self.caps(), required, "list_passive_targets")?;

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
        ensure_device_caps(
            self.caps(),
            DeviceCaps::POLL_TARGET,
            "initiator_poll_target",
        )?;
        self.poll_target_driver(modulations, poll_nr, period)
    }

    fn select_dep_target(
        &mut self,
        ndm: DepMode,
        nbr: BaudRate,
        initiator: Option<&DepInfo>,
        timeout: i32,
    ) -> Result<Option<Target>, Error> {
        ensure_device_caps(
            self.caps(),
            DeviceCaps::SELECT_DEP_TARGET,
            "initiator_select_dep_target",
        )?;
        self.select_dep_target_driver(ndm, nbr, initiator, timeout)
    }

    fn poll_dep_target(
        &mut self,
        ndm: DepMode,
        nbr: BaudRate,
        initiator: Option<&DepInfo>,
        timeout: i32,
    ) -> Result<Option<Target>, Error> {
        ensure_device_caps(
            self.caps(),
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

    fn deselect_target(&mut self) -> Result<(), Error> {
        ensure_device_caps(
            self.caps(),
            DeviceCaps::DESELECT_TARGET,
            "initiator_deselect_target",
        )?;
        self.deselect_target_driver()
    }

    fn target_is_present(&mut self, target: Option<&Target>) -> Result<bool, Error> {
        ensure_device_caps(
            self.caps(),
            DeviceCaps::TARGET_IS_PRESENT,
            "initiator_target_is_present",
        )?;
        self.target_is_present_driver(target)
    }

    fn transceive_bytes(&mut self, tx: &[u8], rx: &mut [u8], timeout: i32) -> Result<usize, Error> {
        ensure_device_caps(
            self.caps(),
            DeviceCaps::TRANSCEIVE_BYTES,
            "initiator_transceive_bytes",
        )?;
        self.transceive_bytes_driver(tx, rx, timeout)
    }

    fn transceive_bits(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        ensure_device_caps(
            self.caps(),
            DeviceCaps::TRANSCEIVE_BITS,
            "initiator_transceive_bits",
        )?;
        self.transceive_bits_driver(tx, tx_bits_len, tx_parity, rx, rx_parity)
    }

    fn transceive_bytes_timed(&mut self, tx: &[u8], rx: &mut [u8]) -> Result<(usize, u32), Error> {
        ensure_device_caps(
            self.caps(),
            DeviceCaps::TRANSCEIVE_BYTES_TIMED,
            "initiator_transceive_bytes_timed",
        )?;
        self.transceive_bytes_timed_driver(tx, rx)
    }

    fn transceive_bits_timed(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), Error> {
        ensure_device_caps(
            self.caps(),
            DeviceCaps::TRANSCEIVE_BITS_TIMED,
            "initiator_transceive_bits_timed",
        )?;
        self.transceive_bits_timed_driver(tx, tx_bits_len, tx_parity, rx, rx_parity)
    }

    fn target_init(
        &mut self,
        target: &mut Target,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error> {
        ensure_device_caps(
            self.caps(),
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

    fn target_send_bytes(&mut self, tx: &[u8], timeout: i32) -> Result<usize, Error> {
        ensure_device_caps(
            self.caps(),
            DeviceCaps::TARGET_SEND_BYTES,
            "target_send_bytes",
        )?;
        self.target_send_bytes_driver(tx, timeout)
    }

    fn target_receive_bytes(&mut self, rx: &mut [u8], timeout: i32) -> Result<usize, Error> {
        ensure_device_caps(
            self.caps(),
            DeviceCaps::TARGET_RECEIVE_BYTES,
            "target_receive_bytes",
        )?;
        self.target_receive_bytes_driver(rx, timeout)
    }

    fn target_send_bits(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
    ) -> Result<usize, Error> {
        ensure_device_caps(
            self.caps(),
            DeviceCaps::TARGET_SEND_BITS,
            "target_send_bits",
        )?;
        self.target_send_bits_driver(tx, tx_bits_len, tx_parity)
    }

    fn target_receive_bits(
        &mut self,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        ensure_device_caps(
            self.caps(),
            DeviceCaps::TARGET_RECEIVE_BITS,
            "target_receive_bits",
        )?;
        self.target_receive_bits_driver(rx, rx_parity)
    }

    fn abort_command(&mut self) -> Result<(), Error> {
        ensure_device_caps(self.caps(), DeviceCaps::ABORT_COMMAND, "abort_command")?;
        self.abort_command_driver()
    }

    fn idle(&mut self) -> Result<(), Error> {
        ensure_device_caps(self.caps(), DeviceCaps::IDLE, "idle")?;
        self.idle_driver()
    }

    fn powerdown(&mut self) -> Result<(), Error> {
        ensure_device_caps(self.caps(), DeviceCaps::POWERDOWN, "powerdown")?;
        self.powerdown_driver()
    }

    #[doc(hidden)]
    fn pn53x_transceive(&mut self, tx: &[u8], rx: &mut [u8], timeout: i32) -> Result<usize, Error> {
        ensure_device_caps(
            self.caps(),
            DeviceCaps::PN53X_TRANSCEIVE,
            "pn53x_transceive",
        )?;
        self.pn53x_transceive_driver(tx, rx, timeout)
    }

    #[doc(hidden)]
    fn pn53x_read_register(&mut self, register: u16) -> Result<u8, Error> {
        ensure_device_caps(
            self.caps(),
            DeviceCaps::PN53X_READ_REGISTER,
            "pn53x_read_register",
        )?;
        self.pn53x_read_register_driver(register)
    }

    #[doc(hidden)]
    fn pn53x_write_register(
        &mut self,
        register: u16,
        symbol_mask: u8,
        value: u8,
    ) -> Result<(), Error> {
        ensure_device_caps(
            self.caps(),
            DeviceCaps::PN53X_WRITE_REGISTER,
            "pn53x_write_register",
        )?;
        self.pn53x_write_register_driver(register, symbol_mask, value)
    }

    #[doc(hidden)]
    fn pn532_sam_configuration(&mut self, mode: u8, timeout: i32) -> Result<i32, Error> {
        ensure_device_caps(
            self.caps(),
            DeviceCaps::PN532_SAM_CONFIGURATION,
            "pn532_SAMConfiguration",
        )?;
        self.pn532_sam_configuration_driver(mode, timeout)
    }
}

pub struct NamedOpenedDevice {
    name: String,
    inner: Box<dyn OpenedDevice>,
}

impl NamedOpenedDevice {
    pub(crate) fn new(name: String, inner: Box<dyn OpenedDevice>) -> Self {
        Self { name, inner }
    }
}

impl OpenedDevice for NamedOpenedDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn connstring(&self) -> &ConnectionString {
        self.inner.connstring()
    }

    fn caps(&self) -> DeviceCaps {
        self.inner.caps()
    }

    fn last_error(&self) -> i32 {
        self.inner.last_error()
    }

    fn strerror(&self) -> String {
        self.inner.strerror()
    }

    fn information_about(&mut self) -> Result<String, Error> {
        self.inner.information_about()
    }

    fn set_property_bool(&mut self, property: Property, enable: bool) -> Result<(), Error> {
        self.inner.set_property_bool(property, enable)
    }

    fn set_property_int(&mut self, property: Property, value: i32) -> Result<(), Error> {
        self.inner.set_property_int(property, value)
    }

    fn supported_modulations(&mut self, mode: Mode) -> Result<Vec<ModulationType>, Error> {
        self.inner.supported_modulations(mode)
    }

    fn supported_baud_rates(
        &mut self,
        mode: Mode,
        modulation_type: ModulationType,
    ) -> Result<Vec<BaudRate>, Error> {
        self.inner.supported_baud_rates(mode, modulation_type)
    }

    fn property_bool_state(&self, property: Property) -> Option<bool> {
        self.inner.property_bool_state(property)
    }

    fn initiator_init_driver(&mut self) -> Result<i32, Error> {
        self.inner.initiator_init_driver()
    }

    fn initiator_init_secure_element_driver(&mut self) -> Result<i32, Error> {
        self.inner.initiator_init_secure_element_driver()
    }

    fn select_passive_target_driver(
        &mut self,
        nm: Modulation,
        init_data: &[u8],
    ) -> Result<Option<Target>, Error> {
        self.inner.select_passive_target_driver(nm, init_data)
    }

    fn poll_target_driver(
        &mut self,
        modulations: &[Modulation],
        poll_nr: u8,
        period: u8,
    ) -> Result<Option<Target>, Error> {
        self.inner.poll_target_driver(modulations, poll_nr, period)
    }

    fn select_dep_target_driver(
        &mut self,
        ndm: DepMode,
        nbr: BaudRate,
        initiator: Option<&DepInfo>,
        timeout: i32,
    ) -> Result<Option<Target>, Error> {
        self.inner
            .select_dep_target_driver(ndm, nbr, initiator, timeout)
    }

    fn deselect_target_driver(&mut self) -> Result<(), Error> {
        self.inner.deselect_target_driver()
    }

    fn target_is_present_driver(&mut self, target: Option<&Target>) -> Result<bool, Error> {
        self.inner.target_is_present_driver(target)
    }

    fn target_init_driver(
        &mut self,
        target: &mut Target,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error> {
        self.inner.target_init_driver(target, rx, timeout)
    }

    fn transceive_bytes_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error> {
        self.inner.transceive_bytes_driver(tx, rx, timeout)
    }

    fn transceive_bits_driver(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        self.inner
            .transceive_bits_driver(tx, tx_bits_len, tx_parity, rx, rx_parity)
    }

    fn transceive_bytes_timed_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
    ) -> Result<(usize, u32), Error> {
        self.inner.transceive_bytes_timed_driver(tx, rx)
    }

    fn transceive_bits_timed_driver(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), Error> {
        self.inner
            .transceive_bits_timed_driver(tx, tx_bits_len, tx_parity, rx, rx_parity)
    }

    fn target_send_bytes_driver(&mut self, tx: &[u8], timeout: i32) -> Result<usize, Error> {
        self.inner.target_send_bytes_driver(tx, timeout)
    }

    fn target_receive_bytes_driver(&mut self, rx: &mut [u8], timeout: i32) -> Result<usize, Error> {
        self.inner.target_receive_bytes_driver(rx, timeout)
    }

    fn target_send_bits_driver(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
    ) -> Result<usize, Error> {
        self.inner
            .target_send_bits_driver(tx, tx_bits_len, tx_parity)
    }

    fn target_receive_bits_driver(
        &mut self,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        self.inner.target_receive_bits_driver(rx, rx_parity)
    }

    fn abort_command_driver(&mut self) -> Result<(), Error> {
        self.inner.abort_command_driver()
    }

    fn idle_driver(&mut self) -> Result<(), Error> {
        self.inner.idle_driver()
    }

    fn powerdown_driver(&mut self) -> Result<(), Error> {
        self.inner.powerdown_driver()
    }

    fn into_native_payload(self: Box<Self>) -> Option<Box<dyn Any + Send>> {
        self.inner.into_native_payload()
    }
}

pub struct Device {
    handle: Box<dyn OpenedDevice>,
}

impl Device {
    pub(crate) fn new(handle: Box<dyn OpenedDevice>) -> Self {
        Self { handle }
    }

    pub fn name(&self) -> &str {
        self.handle.name()
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

    pub fn handle(&self) -> &dyn OpenedDevice {
        self.handle.as_ref()
    }

    pub fn handle_mut(&mut self) -> &mut dyn OpenedDevice {
        self.handle.as_mut()
    }

    #[doc(hidden)]
    pub fn into_handle(self) -> Box<dyn OpenedDevice> {
        self.handle
    }

    pub fn information_about(&mut self) -> Result<String, Error> {
        ensure_device_caps(
            self.caps(),
            DeviceCaps::INFO,
            "device_get_information_about",
        )?;
        self.handle.information_about()
    }

    pub fn set_property_bool(&mut self, property: Property, enable: bool) -> Result<(), Error> {
        ensure_device_caps(
            self.caps(),
            DeviceCaps::SET_PROPERTY_BOOL,
            "device_set_property_bool",
        )?;
        self.handle.set_property_bool(property, enable)
    }

    pub fn set_property_int(&mut self, property: Property, value: i32) -> Result<(), Error> {
        ensure_device_caps(
            self.caps(),
            DeviceCaps::SET_PROPERTY_INT,
            "device_set_property_int",
        )?;
        self.handle.set_property_int(property, value)
    }

    pub fn supported_modulations(&mut self, mode: Mode) -> Result<Vec<ModulationType>, Error> {
        ensure_device_caps(
            self.caps(),
            DeviceCaps::SUPPORTED_MODULATIONS,
            "get_supported_modulation",
        )?;
        self.handle.supported_modulations(mode)
    }

    pub fn supported_baud_rates(
        &mut self,
        mode: Mode,
        modulation_type: ModulationType,
    ) -> Result<Vec<BaudRate>, Error> {
        ensure_device_caps(
            self.caps(),
            DeviceCaps::SUPPORTED_BAUD_RATES,
            "get_supported_baud_rate",
        )?;
        self.handle.supported_baud_rates(mode, modulation_type)
    }

    pub fn initiator_init(&mut self) -> Result<i32, Error> {
        self.handle.initiator_init()
    }

    pub fn initiator_init_secure_element(&mut self) -> Result<i32, Error> {
        self.handle.initiator_init_secure_element()
    }

    pub fn select_passive_target(
        &mut self,
        modulation: Modulation,
        init_data: Option<&[u8]>,
    ) -> Result<Option<Target>, Error> {
        self.handle.select_passive_target(modulation, init_data)
    }

    pub fn list_passive_targets(
        &mut self,
        modulation: Modulation,
        max_targets: usize,
    ) -> Result<Vec<Target>, Error> {
        self.handle.list_passive_targets(modulation, max_targets)
    }

    pub fn poll_target(
        &mut self,
        modulations: &[Modulation],
        poll_nr: u8,
        period: u8,
    ) -> Result<Option<Target>, Error> {
        self.handle.poll_target(modulations, poll_nr, period)
    }

    pub fn select_dep_target(
        &mut self,
        mode: DepMode,
        baud_rate: BaudRate,
        initiator: Option<&DepInfo>,
        timeout: i32,
    ) -> Result<Option<Target>, Error> {
        self.handle
            .select_dep_target(mode, baud_rate, initiator, timeout)
    }

    pub fn poll_dep_target(
        &mut self,
        mode: DepMode,
        baud_rate: BaudRate,
        initiator: Option<&DepInfo>,
        timeout: i32,
    ) -> Result<Option<Target>, Error> {
        self.handle
            .poll_dep_target(mode, baud_rate, initiator, timeout)
    }

    pub fn deselect_target(&mut self) -> Result<(), Error> {
        self.handle.deselect_target()
    }

    pub fn target_is_present(&mut self, target: Option<&Target>) -> Result<bool, Error> {
        self.handle.target_is_present(target)
    }

    pub fn transceive_bytes(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error> {
        self.handle.transceive_bytes(tx, rx, timeout)
    }

    pub fn transceive_bits(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        self.handle
            .transceive_bits(tx, tx_bits_len, tx_parity, rx, rx_parity)
    }

    pub fn transceive_bytes_timed(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
    ) -> Result<(usize, u32), Error> {
        self.handle.transceive_bytes_timed(tx, rx)
    }

    pub fn transceive_bits_timed(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), Error> {
        self.handle
            .transceive_bits_timed(tx, tx_bits_len, tx_parity, rx, rx_parity)
    }

    pub fn target_init(
        &mut self,
        target: &mut Target,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error> {
        self.handle.target_init(target, rx, timeout)
    }

    pub fn target_send_bytes(&mut self, tx: &[u8], timeout: i32) -> Result<usize, Error> {
        self.handle.target_send_bytes(tx, timeout)
    }

    pub fn target_receive_bytes(&mut self, rx: &mut [u8], timeout: i32) -> Result<usize, Error> {
        self.handle.target_receive_bytes(rx, timeout)
    }

    pub fn target_send_bits(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
    ) -> Result<usize, Error> {
        self.handle.target_send_bits(tx, tx_bits_len, tx_parity)
    }

    pub fn target_receive_bits(
        &mut self,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        self.handle.target_receive_bits(rx, rx_parity)
    }

    pub fn abort_command(&mut self) -> Result<(), Error> {
        self.handle.abort_command()
    }

    pub fn idle(&mut self) -> Result<(), Error> {
        self.handle.idle()
    }

    pub fn powerdown(&mut self) -> Result<(), Error> {
        self.handle.powerdown()
    }
}

pub(crate) fn apply_bool_property_sequence<D: OpenedDevice + ?Sized>(
    device: &mut D,
    settings: &[(Property, bool)],
) -> Result<(), Error> {
    for (property, value) in settings {
        device.set_property_bool(*property, *value)?;
    }
    Ok(())
}

pub(crate) fn restore_property_bool<D: OpenedDevice + ?Sized>(
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

pub(crate) fn validate_modulation<D: OpenedDevice + ?Sized>(
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

pub trait PropertyOps: OpenedDevice {}
impl<T: OpenedDevice + ?Sized> PropertyOps for T {}

pub trait InitiatorOps: OpenedDevice {}
impl<T: OpenedDevice + ?Sized> InitiatorOps for T {}

pub trait TargetOps: OpenedDevice {}
impl<T: OpenedDevice + ?Sized> TargetOps for T {}

pub trait ChipDebugOps: OpenedDevice {}
impl<T: OpenedDevice + ?Sized> ChipDebugOps for T {}
