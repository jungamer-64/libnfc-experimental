use super::external::ExternalDevice;
use super::rust_owned::RustDeviceState;
use super::*;

unsafe fn rust_device_state<'a>(device: *mut nfc_device) -> Option<&'a mut RustDeviceState> {
    let device = unsafe { optional_mut(device) }?;
    unsafe { optional_mut(device.driver_data as *mut RustDeviceState) }
}

pub(crate) fn borrowed_device(raw: *mut nfc_device) -> rt::Device {
    if is_rust_shim_device(raw) {
        return rt::Device::from_handle(Box::new(RustBorrowedDevice::new(raw)));
    }
    rt::Device::from_handle(Box::new(ExternalDevice::borrowed(raw)))
}

struct RustBorrowedDevice {
    raw: *mut nfc_device,
    name: String,
    connstring: rt::ConnectionString,
}

unsafe impl Send for RustBorrowedDevice {}

impl RustBorrowedDevice {
    fn new(raw: *mut nfc_device) -> Self {
        let name = unsafe { optional_ref(raw) }
            .map(|device| fixed_c_buffer_to_string(&device.name))
            .unwrap_or_default();
        let connstring_string = unsafe { optional_ref(raw) }
            .map(|device| fixed_c_buffer_to_string(&device.connstring))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unknown".to_string());
        let connstring = rt::ConnectionString::new(connstring_string)
            .unwrap_or_else(|_| rt::ConnectionString::new("unknown").expect("valid connstring"));
        Self {
            raw,
            name,
            connstring,
        }
    }

    fn with_handle<R>(
        &mut self,
        f: impl FnOnce(&mut dyn rt::DeviceHandle) -> Result<R, rt::Error>,
    ) -> Result<R, rt::Error> {
        let Some(state) = (unsafe { rust_device_state(self.raw) }) else {
            return Err(rt::Error::DriverNotFound("rust shim".to_string()));
        };
        let result = f(state.handle.as_mut());
        sync_property_mirrors(self.raw, state.handle.as_ref());
        result
    }

    fn normalize<T>(
        &mut self,
        required: rt::DeviceCaps,
        operation: &'static str,
        result: Result<T, rt::Error>,
    ) -> Result<T, rt::Error> {
        if !rt::DeviceMeta::caps(self).contains(required) {
            set_device_last_error(self.raw, NFC_EDEVNOTSUPP);
            return Err(missing_capability(operation));
        }

        match result {
            Ok(value) => {
                set_device_last_error(self.raw, 0);
                Ok(value)
            }
            Err(rt::Error::UnsupportedOperation(_)) => {
                set_device_last_error(self.raw, NFC_EDEVNOTSUPP);
                Err(missing_capability(operation))
            }
            Err(error @ rt::Error::DeviceOperationFailed { code, .. }) => {
                set_device_last_error(self.raw, code);
                Err(error)
            }
            Err(error) => Err(error),
        }
    }
}

impl rt::DeviceMeta for RustBorrowedDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn connstring(&self) -> &rt::ConnectionString {
        &self.connstring
    }

    fn caps(&self) -> rt::DeviceCaps {
        unsafe { rust_device_state(self.raw) }
            .map(|state| state.handle.caps())
            .unwrap_or(rt::DeviceCaps::NONE)
    }

    fn last_error(&self) -> i32 {
        unsafe { optional_ref(self.raw) }
            .map(|device| device.last_error)
            .unwrap_or(0)
    }

    fn strerror(&self) -> String {
        unsafe { rust_device_state(self.raw) }
            .map(|state| state.handle.strerror())
            .unwrap_or_else(|| rt::device_error_message(self.last_error()).to_string())
    }

    fn missing_capability(&mut self, operation: &'static str) -> rt::Error {
        set_device_last_error(self.raw, NFC_EDEVNOTSUPP);
        missing_capability(operation)
    }
}

impl rt::InfoBackend for RustBorrowedDevice {
    fn information_about(&mut self) -> Result<String, rt::Error> {
        let result = self.with_handle(|handle| handle.information_about());
        self.normalize(rt::DeviceCaps::INFO, "device_get_information_about", result)
    }
}

impl rt::PropertyBackend for RustBorrowedDevice {
    fn set_property_bool(&mut self, property: rt::Property, enable: bool) -> Result<(), rt::Error> {
        let result = self.with_handle(|handle| handle.set_property_bool(property, enable));
        self.normalize(
            rt::DeviceCaps::SET_PROPERTY_BOOL,
            "device_set_property_bool",
            result,
        )
    }

    fn set_property_int(&mut self, property: rt::Property, value: i32) -> Result<(), rt::Error> {
        let result = self.with_handle(|handle| handle.set_property_int(property, value));
        self.normalize(
            rt::DeviceCaps::SET_PROPERTY_INT,
            "device_set_property_int",
            result,
        )
    }

    fn supported_modulations(
        &mut self,
        mode: rt::Mode,
    ) -> Result<Vec<rt::ModulationType>, rt::Error> {
        let result = self.with_handle(|handle| handle.supported_modulations(mode));
        self.normalize(
            rt::DeviceCaps::SUPPORTED_MODULATIONS,
            "get_supported_modulation",
            result,
        )
    }

    fn supported_baud_rates(
        &mut self,
        mode: rt::Mode,
        modulation_type: rt::ModulationType,
    ) -> Result<Vec<rt::BaudRate>, rt::Error> {
        let result = self.with_handle(|handle| handle.supported_baud_rates(mode, modulation_type));
        self.normalize(
            rt::DeviceCaps::SUPPORTED_BAUD_RATES,
            "get_supported_baud_rate",
            result,
        )
    }

    fn property_bool_state(&self, property: rt::Property) -> Option<bool> {
        let device = unsafe { optional_ref(self.raw) }?;
        Some(match property {
            rt::Property::HandleCrc => device.bCrc,
            rt::Property::HandleParity => device.bPar,
            rt::Property::EasyFraming => device.bEasyFraming,
            rt::Property::InfiniteSelect => device.bInfiniteSelect,
            rt::Property::AutoIso14443_4 => device.bAutoIso14443_4,
            _ => return None,
        })
    }
}

impl rt::InitiatorBackend for RustBorrowedDevice {
    fn initiator_init_driver(&mut self) -> Result<i32, rt::Error> {
        let result = self.with_handle(|handle| handle.initiator_init_driver());
        self.normalize(rt::DeviceCaps::INITIATOR_INIT, "initiator_init", result)
    }

    fn initiator_init_secure_element_driver(&mut self) -> Result<i32, rt::Error> {
        let result = self.with_handle(|handle| handle.initiator_init_secure_element_driver());
        self.normalize(
            rt::DeviceCaps::INITIATOR_INIT_SECURE_ELEMENT,
            "initiator_init_secure_element",
            result,
        )
    }

    fn select_passive_target_driver(
        &mut self,
        nm: rt::Modulation,
        init_data: &[u8],
    ) -> Result<Option<rt::Target>, rt::Error> {
        let result = self.with_handle(|handle| handle.select_passive_target_driver(nm, init_data));
        self.normalize(
            rt::DeviceCaps::SELECT_PASSIVE_TARGET,
            "initiator_select_passive_target",
            result,
        )
    }

    fn poll_target_driver(
        &mut self,
        modulations: &[rt::Modulation],
        poll_nr: u8,
        period: u8,
    ) -> Result<Option<rt::Target>, rt::Error> {
        let result =
            self.with_handle(|handle| handle.poll_target_driver(modulations, poll_nr, period));
        self.normalize(rt::DeviceCaps::POLL_TARGET, "initiator_poll_target", result)
    }

    fn select_dep_target_driver(
        &mut self,
        ndm: rt::DepMode,
        nbr: rt::BaudRate,
        initiator: Option<&rt::DepInfo>,
        timeout: i32,
    ) -> Result<Option<rt::Target>, rt::Error> {
        let result = self
            .with_handle(|handle| handle.select_dep_target_driver(ndm, nbr, initiator, timeout));
        self.normalize(
            rt::DeviceCaps::SELECT_DEP_TARGET,
            "initiator_select_dep_target",
            result,
        )
    }

    fn deselect_target_driver(&mut self) -> Result<(), rt::Error> {
        let result = self.with_handle(|handle| handle.deselect_target_driver());
        self.normalize(
            rt::DeviceCaps::DESELECT_TARGET,
            "initiator_deselect_target",
            result,
        )
    }

    fn target_is_present_driver(&mut self, target: Option<&rt::Target>) -> Result<bool, rt::Error> {
        let result = self.with_handle(|handle| handle.target_is_present_driver(target));
        self.normalize(
            rt::DeviceCaps::TARGET_IS_PRESENT,
            "initiator_target_is_present",
            result,
        )
    }

    fn transceive_bytes_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, rt::Error> {
        let result = self.with_handle(|handle| handle.transceive_bytes_driver(tx, rx, timeout));
        self.normalize(
            rt::DeviceCaps::TRANSCEIVE_BYTES,
            "initiator_transceive_bytes",
            result,
        )
    }

    fn transceive_bits_driver(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, rt::Error> {
        let result = self.with_handle(|handle| {
            handle.transceive_bits_driver(tx, tx_bits_len, tx_parity, rx, rx_parity)
        });
        self.normalize(
            rt::DeviceCaps::TRANSCEIVE_BITS,
            "initiator_transceive_bits",
            result,
        )
    }

    fn transceive_bytes_timed_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
    ) -> Result<(usize, u32), rt::Error> {
        let result = self.with_handle(|handle| handle.transceive_bytes_timed_driver(tx, rx));
        self.normalize(
            rt::DeviceCaps::TRANSCEIVE_BYTES_TIMED,
            "initiator_transceive_bytes_timed",
            result,
        )
    }

    fn transceive_bits_timed_driver(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), rt::Error> {
        let result = self.with_handle(|handle| {
            handle.transceive_bits_timed_driver(tx, tx_bits_len, tx_parity, rx, rx_parity)
        });
        self.normalize(
            rt::DeviceCaps::TRANSCEIVE_BITS_TIMED,
            "initiator_transceive_bits_timed",
            result,
        )
    }

    fn abort_command_driver(&mut self) -> Result<(), rt::Error> {
        let result = self.with_handle(|handle| handle.abort_command_driver());
        self.normalize(rt::DeviceCaps::ABORT_COMMAND, "abort_command", result)
    }

    fn idle_driver(&mut self) -> Result<(), rt::Error> {
        let result = self.with_handle(|handle| handle.idle_driver());
        self.normalize(rt::DeviceCaps::IDLE, "idle", result)
    }

    fn powerdown_driver(&mut self) -> Result<(), rt::Error> {
        let result = self.with_handle(|handle| handle.powerdown_driver());
        self.normalize(rt::DeviceCaps::POWERDOWN, "powerdown", result)
    }
}

impl rt::TargetBackend for RustBorrowedDevice {
    fn target_init_driver(
        &mut self,
        target: &mut rt::Target,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, rt::Error> {
        let result = self.with_handle(|handle| handle.target_init_driver(target, rx, timeout));
        self.normalize(rt::DeviceCaps::TARGET_INIT, "target_init", result)
    }

    fn target_send_bytes_driver(&mut self, tx: &[u8], timeout: i32) -> Result<usize, rt::Error> {
        let result = self.with_handle(|handle| handle.target_send_bytes_driver(tx, timeout));
        self.normalize(
            rt::DeviceCaps::TARGET_SEND_BYTES,
            "target_send_bytes",
            result,
        )
    }

    fn target_receive_bytes_driver(
        &mut self,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, rt::Error> {
        let result = self.with_handle(|handle| handle.target_receive_bytes_driver(rx, timeout));
        self.normalize(
            rt::DeviceCaps::TARGET_RECEIVE_BYTES,
            "target_receive_bytes",
            result,
        )
    }

    fn target_send_bits_driver(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
    ) -> Result<usize, rt::Error> {
        let result =
            self.with_handle(|handle| handle.target_send_bits_driver(tx, tx_bits_len, tx_parity));
        self.normalize(rt::DeviceCaps::TARGET_SEND_BITS, "target_send_bits", result)
    }

    fn target_receive_bits_driver(
        &mut self,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, rt::Error> {
        let result = self.with_handle(|handle| handle.target_receive_bits_driver(rx, rx_parity));
        self.normalize(
            rt::DeviceCaps::TARGET_RECEIVE_BITS,
            "target_receive_bits",
            result,
        )
    }
}

impl rt::Pn53xBackend for RustBorrowedDevice {
    fn pn53x_transceive_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, rt::Error> {
        let result = self.with_handle(|handle| handle.pn53x_transceive_driver(tx, rx, timeout));
        self.normalize(rt::DeviceCaps::PN53X_TRANSCEIVE, "pn53x_transceive", result)
    }

    fn pn53x_read_register_driver(&mut self, register: u16) -> Result<u8, rt::Error> {
        let result = self.with_handle(|handle| handle.pn53x_read_register_driver(register));
        self.normalize(
            rt::DeviceCaps::PN53X_READ_REGISTER,
            "pn53x_read_register",
            result,
        )
    }

    fn pn53x_write_register_driver(
        &mut self,
        register: u16,
        symbol_mask: u8,
        value: u8,
    ) -> Result<(), rt::Error> {
        let result = self
            .with_handle(|handle| handle.pn53x_write_register_driver(register, symbol_mask, value));
        self.normalize(
            rt::DeviceCaps::PN53X_WRITE_REGISTER,
            "pn53x_write_register",
            result,
        )
    }

    fn pn532_sam_configuration_driver(&mut self, mode: u8, timeout: i32) -> Result<i32, rt::Error> {
        let result =
            self.with_handle(|handle| handle.pn532_sam_configuration_driver(mode, timeout));
        self.normalize(
            rt::DeviceCaps::PN532_SAM_CONFIGURATION,
            "pn532_SAMConfiguration",
            result,
        )
    }
}
