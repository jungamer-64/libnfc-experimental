use super::*;

pub(crate) struct ExternalDriver {
    raw: *const nfc_driver,
    name: String,
    scan_type: rt::ScanType,
    caps: rt::DriverCaps,
}

unsafe impl Send for ExternalDriver {}
unsafe impl Sync for ExternalDriver {}

impl ExternalDriver {
    pub(crate) fn new(raw: *const nfc_driver) -> Self {
        let name = unsafe { as_ref(raw) }
            .map(|driver| c_string_ptr_to_string(driver.name, NFC_DRIVER_NAME_MAX))
            .unwrap_or_default();
        let scan_type = unsafe { as_ref(raw) }
            .map(|driver| match driver.scan_type {
                scan_type_enum::NOT_INTRUSIVE => rt::ScanType::NotIntrusive,
                scan_type_enum::INTRUSIVE => rt::ScanType::Intrusive,
                scan_type_enum::NOT_AVAILABLE => rt::ScanType::NotAvailable,
            })
            .unwrap_or(rt::ScanType::NotAvailable);
        let caps = unsafe { as_ref(raw) }
            .map(driver_caps_from_raw)
            .unwrap_or(rt::DriverCaps::NONE);
        Self {
            raw,
            name,
            scan_type,
            caps,
        }
    }
}

impl rt::Driver for ExternalDriver {
    fn name(&self) -> &str {
        &self.name
    }

    fn scan_type(&self) -> rt::ScanType {
        self.scan_type
    }

    fn caps(&self) -> rt::DriverCaps {
        self.caps
    }

    fn scan(&self, context: &rt::Context) -> Result<Vec<rt::ConnectionString>, rt::Error> {
        if !self.caps.contains(rt::DriverCaps::SCAN) {
            return Err(missing_capability("scan"));
        }

        let Some(driver) = (unsafe { as_ref(self.raw) }) else {
            return Ok(Vec::new());
        };
        let Some(scan) = driver.scan else {
            return Err(missing_capability("scan"));
        };

        let mut capacity = DEFAULT_SCAN_CAPACITY;
        loop {
            let mut raw_context = unsafe { std::mem::zeroed::<nfc_context>() };
            write_context_to_c(context, ptr::addr_of_mut!(raw_context));

            let mut buffer = vec![[0 as c_char; NFC_BUFSIZE_CONNSTRING]; capacity];
            let found = unsafe { scan(ptr::addr_of!(raw_context), buffer.as_mut_ptr(), capacity) };
            let mut devices = Vec::new();
            for connstring in buffer.iter().take(found.min(capacity)) {
                let value = fixed_c_buffer_to_string(connstring);
                if value.is_empty() {
                    continue;
                }
                devices.push(rt::ConnectionString::new(value)?);
            }

            if devices.len() < capacity || capacity >= MAX_SCAN_CAPACITY {
                return Ok(devices);
            }
            capacity = (capacity * 2).min(MAX_SCAN_CAPACITY);
        }
    }

    fn open(
        &self,
        context: &rt::Context,
        connstring: &rt::ConnectionString,
    ) -> Result<Box<dyn rt::DeviceBackend>, rt::Error> {
        if !self.caps.contains(rt::DriverCaps::OPEN) {
            return Err(missing_capability("open"));
        }

        let Some(driver) = (unsafe { as_ref(self.raw) }) else {
            return Err(rt::Error::DriverOpenFailed(connstring.as_str().to_string()));
        };
        let Some(open) = driver.open else {
            return Err(missing_capability("open"));
        };

        let mut raw_context = unsafe { std::mem::zeroed::<nfc_context>() };
        write_context_to_c(context, ptr::addr_of_mut!(raw_context));

        let connstring_c = CString::new(connstring.as_str())
            .map_err(|_| rt::Error::InvalidEncoding("connstring"))?;
        let raw_device = unsafe { open(ptr::addr_of!(raw_context), connstring_c.as_ptr()) };
        if raw_device.is_null() {
            return Err(rt::Error::DriverOpenFailed(connstring.as_str().to_string()));
        }

        Ok(Box::new(ExternalDevice::owned(raw_device)))
    }
}

pub(super) struct ExternalDevice {
    raw: *mut nfc_device,
    name: String,
    connstring: rt::ConnectionString,
    caps: rt::DeviceCaps,
    owned: bool,
}

unsafe impl Send for ExternalDevice {}

impl ExternalDevice {
    pub(super) fn borrowed(raw: *mut nfc_device) -> Self {
        Self::new(raw, false)
    }

    fn owned(raw: *mut nfc_device) -> Self {
        Self::new(raw, true)
    }

    fn new(raw: *mut nfc_device, owned: bool) -> Self {
        let name = unsafe { as_ref(raw) }
            .map(|device| fixed_c_buffer_to_string(&device.name))
            .unwrap_or_default();
        let connstring_string = unsafe { as_ref(raw) }
            .map(|device| fixed_c_buffer_to_string(&device.connstring))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unknown".to_string());
        let connstring = rt::ConnectionString::new(connstring_string)
            .unwrap_or_else(|_| rt::ConnectionString::new("unknown").expect("valid connstring"));
        let caps = unsafe { as_ref(raw) }
            .and_then(|device| unsafe { as_ref(device.driver) })
            .map(device_caps_from_raw)
            .unwrap_or(rt::DeviceCaps::NONE);
        Self {
            raw,
            name,
            connstring,
            caps,
            owned,
        }
    }

    fn set_last_error_raw(&mut self, value: c_int) {
        set_device_last_error(self.raw, value);
    }

    fn driver_ref(&self) -> Option<&nfc_driver> {
        let device = unsafe { as_ref(self.raw) }?;
        unsafe { as_ref(device.driver) }
    }

    fn status_to_result(operation: &'static str, status: c_int) -> Result<c_int, rt::Error> {
        if status < 0 {
            Err(rt::Error::DeviceOperationFailed {
                operation,
                code: status,
            })
        } else {
            Ok(status)
        }
    }

    fn normalize<T>(
        &mut self,
        required: rt::DeviceCaps,
        operation: &'static str,
        result: Result<T, rt::Error>,
    ) -> Result<T, rt::Error> {
        if !self.caps.contains(required) {
            self.set_last_error_raw(NFC_EDEVNOTSUPP);
            return Err(missing_capability(operation));
        }

        match result {
            Ok(value) => {
                self.set_last_error_raw(0);
                Ok(value)
            }
            Err(rt::Error::UnsupportedOperation(_)) => {
                self.set_last_error_raw(NFC_EDEVNOTSUPP);
                Err(missing_capability(operation))
            }
            Err(error @ rt::Error::DeviceOperationFailed { code, .. }) => {
                self.set_last_error_raw(code);
                Err(error)
            }
            Err(error) => Err(error),
        }
    }
}

impl Drop for ExternalDevice {
    fn drop(&mut self) {
        if self.owned && !self.raw.is_null() {
            unsafe { crate::core::bridge_close_device(self.raw) };
        }
    }
}

impl rt::DeviceMeta for ExternalDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn connstring(&self) -> &rt::ConnectionString {
        &self.connstring
    }

    fn caps(&self) -> rt::DeviceCaps {
        self.caps
    }

    fn last_error(&self) -> i32 {
        unsafe { as_ref(self.raw) }
            .map(|device| device.last_error)
            .unwrap_or(0)
    }

    fn strerror(&self) -> String {
        let Some(driver) = self.driver_ref() else {
            return rt::device_error_message(self.last_error()).to_string();
        };
        let Some(callback) = driver.strerror else {
            return rt::device_error_message(self.last_error()).to_string();
        };
        let value = unsafe { callback(self.raw.cast_const()) };
        if value.is_null() {
            rt::device_error_message(self.last_error()).to_string()
        } else {
            c_string_ptr_to_string(value, bounded_strlen(value, 256))
        }
    }
}

impl rt::InfoBackend for ExternalDevice {
    fn information_about(&mut self) -> Result<String, rt::Error> {
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation(
                    "device_get_information_about",
                ));
            };
            let Some(callback) = driver.device_get_information_about else {
                return Err(rt::Error::UnsupportedOperation(
                    "device_get_information_about",
                ));
            };

            let mut buffer = ptr::null_mut();
            Self::status_to_result("device_get_information_about", unsafe {
                callback(self.raw, ptr::addr_of_mut!(buffer))
            })?;
            let value = c_string_ptr_to_string(buffer, bounded_strlen(buffer, 4096));
            unsafe { release_allocated_ptr(buffer.cast()) };
            Ok(value)
        })();
        self.normalize(rt::DeviceCaps::INFO, "device_get_information_about", result)
    }
}

impl rt::PropertyBackend for ExternalDevice {
    fn set_property_bool(&mut self, property: rt::Property, enable: bool) -> Result<(), rt::Error> {
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation("device_set_property_bool"));
            };
            let Some(callback) = driver.device_set_property_bool else {
                return Err(rt::Error::UnsupportedOperation("device_set_property_bool"));
            };
            Self::status_to_result("device_set_property_bool", unsafe {
                callback(self.raw, property_to_c(property), enable)
            })?;
            Ok(())
        })();
        self.normalize(
            rt::DeviceCaps::SET_PROPERTY_BOOL,
            "device_set_property_bool",
            result,
        )
    }

    fn set_property_int(&mut self, property: rt::Property, value: i32) -> Result<(), rt::Error> {
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation("device_set_property_int"));
            };
            let Some(callback) = driver.device_set_property_int else {
                return Err(rt::Error::UnsupportedOperation("device_set_property_int"));
            };
            Self::status_to_result("device_set_property_int", unsafe {
                callback(self.raw, property_to_c(property), value)
            })?;
            Ok(())
        })();
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
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation("get_supported_modulation"));
            };
            let Some(callback) = driver.get_supported_modulation else {
                return Err(rt::Error::UnsupportedOperation("get_supported_modulation"));
            };

            let mut supported = ptr::null();
            Self::status_to_result("get_supported_modulation", unsafe {
                callback(self.raw, mode_to_c(mode), ptr::addr_of_mut!(supported))
            })?;

            let mut values = Vec::new();
            let mut index = 0usize;
            while !supported.is_null() {
                let value = unsafe { supported.add(index).read() };
                if matches!(value, nfc_modulation_type::NMT_UNDEFINED) {
                    break;
                }
                values.push(modulation_type_from_c(value));
                index += 1;
            }
            Ok(values)
        })();
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
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation("get_supported_baud_rate"));
            };
            let Some(callback) = driver.get_supported_baud_rate else {
                return Err(rt::Error::UnsupportedOperation("get_supported_baud_rate"));
            };

            let mut supported = ptr::null();
            Self::status_to_result("get_supported_baud_rate", unsafe {
                callback(
                    self.raw,
                    mode_to_c(mode),
                    modulation_type_to_c(modulation_type),
                    ptr::addr_of_mut!(supported),
                )
            })?;

            let mut values = Vec::new();
            let mut index = 0usize;
            while !supported.is_null() {
                let value = unsafe { supported.add(index).read() };
                if matches!(value, nfc_baud_rate::NBR_UNDEFINED) {
                    break;
                }
                values.push(baud_rate_from_c(value));
                index += 1;
            }
            Ok(values)
        })();
        self.normalize(
            rt::DeviceCaps::SUPPORTED_BAUD_RATES,
            "get_supported_baud_rate",
            result,
        )
    }

    fn property_bool_state(&self, property: rt::Property) -> Option<bool> {
        let device = unsafe { as_ref(self.raw) }?;
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

impl rt::InitiatorBackend for ExternalDevice {
    fn initiator_init_driver(&mut self) -> Result<i32, rt::Error> {
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation("initiator_init"));
            };
            let Some(callback) = driver.initiator_init else {
                return Err(rt::Error::UnsupportedOperation("initiator_init"));
            };
            Self::status_to_result("initiator_init", unsafe { callback(self.raw) })
        })();
        self.normalize(rt::DeviceCaps::INITIATOR_INIT, "initiator_init", result)
    }

    fn initiator_init_secure_element_driver(&mut self) -> Result<i32, rt::Error> {
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation(
                    "initiator_init_secure_element",
                ));
            };
            let Some(callback) = driver.initiator_init_secure_element else {
                return Err(rt::Error::UnsupportedOperation(
                    "initiator_init_secure_element",
                ));
            };
            Self::status_to_result("initiator_init_secure_element", unsafe {
                callback(self.raw)
            })
        })();
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
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation(
                    "initiator_select_passive_target",
                ));
            };
            let Some(callback) = driver.initiator_select_passive_target else {
                return Err(rt::Error::UnsupportedOperation(
                    "initiator_select_passive_target",
                ));
            };
            let mut target = unsafe { std::mem::zeroed::<nfc_target>() };
            let status = Self::status_to_result("initiator_select_passive_target", unsafe {
                callback(
                    self.raw,
                    modulation_to_c(nm),
                    if init_data.is_empty() {
                        ptr::null()
                    } else {
                        init_data.as_ptr()
                    },
                    init_data.len(),
                    ptr::addr_of_mut!(target),
                )
            })?;
            if status == 0 {
                return Ok(None);
            }
            Ok(Some(target_from_c(ptr::addr_of!(target))))
        })();
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
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation("initiator_poll_target"));
            };
            let Some(callback) = driver.initiator_poll_target else {
                return Err(rt::Error::UnsupportedOperation("initiator_poll_target"));
            };
            let raw_modulations: Vec<nfc_modulation> =
                modulations.iter().copied().map(modulation_to_c).collect();
            let mut target = unsafe { std::mem::zeroed::<nfc_target>() };
            let status = Self::status_to_result("initiator_poll_target", unsafe {
                callback(
                    self.raw,
                    raw_modulations.as_ptr(),
                    raw_modulations.len(),
                    poll_nr,
                    period,
                    ptr::addr_of_mut!(target),
                )
            })?;
            if status == 0 {
                return Ok(None);
            }
            Ok(Some(target_from_c(ptr::addr_of!(target))))
        })();
        self.normalize(rt::DeviceCaps::POLL_TARGET, "initiator_poll_target", result)
    }

    fn select_dep_target_driver(
        &mut self,
        ndm: rt::DepMode,
        nbr: rt::BaudRate,
        initiator: Option<&rt::DepInfo>,
        timeout: i32,
    ) -> Result<Option<rt::Target>, rt::Error> {
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation(
                    "initiator_select_dep_target",
                ));
            };
            let Some(callback) = driver.initiator_select_dep_target else {
                return Err(rt::Error::UnsupportedOperation(
                    "initiator_select_dep_target",
                ));
            };
            let raw_initiator = initiator.map(dep_info_to_c);
            let mut target = unsafe { std::mem::zeroed::<nfc_target>() };
            let status = Self::status_to_result("initiator_select_dep_target", unsafe {
                callback(
                    self.raw,
                    dep_mode_to_c(ndm),
                    baud_rate_to_c(nbr),
                    raw_initiator
                        .as_ref()
                        .map_or(ptr::null(), |value| ptr::addr_of!(*value)),
                    ptr::addr_of_mut!(target),
                    timeout,
                )
            })?;
            if status == 0 {
                return Ok(None);
            }
            Ok(Some(target_from_c(ptr::addr_of!(target))))
        })();
        self.normalize(
            rt::DeviceCaps::SELECT_DEP_TARGET,
            "initiator_select_dep_target",
            result,
        )
    }

    fn deselect_target_driver(&mut self) -> Result<(), rt::Error> {
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation("initiator_deselect_target"));
            };
            let Some(callback) = driver.initiator_deselect_target else {
                return Err(rt::Error::UnsupportedOperation("initiator_deselect_target"));
            };
            Self::status_to_result("initiator_deselect_target", unsafe { callback(self.raw) })?;
            Ok(())
        })();
        self.normalize(
            rt::DeviceCaps::DESELECT_TARGET,
            "initiator_deselect_target",
            result,
        )
    }

    fn target_is_present_driver(&mut self, target: Option<&rt::Target>) -> Result<bool, rt::Error> {
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation(
                    "initiator_target_is_present",
                ));
            };
            let Some(callback) = driver.initiator_target_is_present else {
                return Err(rt::Error::UnsupportedOperation(
                    "initiator_target_is_present",
                ));
            };
            let raw_target = target.map(target_to_c);
            let status = Self::status_to_result("initiator_target_is_present", unsafe {
                callback(
                    self.raw,
                    raw_target
                        .as_ref()
                        .map_or(ptr::null(), |value| ptr::addr_of!(*value)),
                )
            })?;
            Ok(status > 0)
        })();
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
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation(
                    "initiator_transceive_bytes",
                ));
            };
            let Some(callback) = driver.initiator_transceive_bytes else {
                return Err(rt::Error::UnsupportedOperation(
                    "initiator_transceive_bytes",
                ));
            };
            let count = Self::status_to_result("initiator_transceive_bytes", unsafe {
                callback(
                    self.raw,
                    bytes_ptr(tx),
                    tx.len(),
                    bytes_mut_ptr(rx),
                    rx.len(),
                    timeout,
                )
            })?;
            Ok(count as usize)
        })();
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
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation("initiator_transceive_bits"));
            };
            let Some(callback) = driver.initiator_transceive_bits else {
                return Err(rt::Error::UnsupportedOperation("initiator_transceive_bits"));
            };
            let count = Self::status_to_result("initiator_transceive_bits", unsafe {
                callback(
                    self.raw,
                    bytes_ptr(tx),
                    tx_bits_len,
                    optional_bytes_ptr(tx_parity),
                    bytes_mut_ptr(rx),
                    optional_bytes_mut_ptr(rx_parity),
                )
            })?;
            Ok(count as usize)
        })();
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
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation(
                    "initiator_transceive_bytes_timed",
                ));
            };
            let Some(callback) = driver.initiator_transceive_bytes_timed else {
                return Err(rt::Error::UnsupportedOperation(
                    "initiator_transceive_bytes_timed",
                ));
            };
            let mut cycles = 0u32;
            let count = Self::status_to_result("initiator_transceive_bytes_timed", unsafe {
                callback(
                    self.raw,
                    bytes_ptr(tx),
                    tx.len(),
                    bytes_mut_ptr(rx),
                    rx.len(),
                    ptr::addr_of_mut!(cycles),
                )
            })?;
            Ok((count as usize, cycles))
        })();
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
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation(
                    "initiator_transceive_bits_timed",
                ));
            };
            let Some(callback) = driver.initiator_transceive_bits_timed else {
                return Err(rt::Error::UnsupportedOperation(
                    "initiator_transceive_bits_timed",
                ));
            };
            let mut cycles = 0u32;
            let count = Self::status_to_result("initiator_transceive_bits_timed", unsafe {
                callback(
                    self.raw,
                    bytes_ptr(tx),
                    tx_bits_len,
                    optional_bytes_ptr(tx_parity),
                    bytes_mut_ptr(rx),
                    optional_bytes_mut_ptr(rx_parity),
                    ptr::addr_of_mut!(cycles),
                )
            })?;
            Ok((count as usize, cycles))
        })();
        self.normalize(
            rt::DeviceCaps::TRANSCEIVE_BITS_TIMED,
            "initiator_transceive_bits_timed",
            result,
        )
    }

    fn abort_command_driver(&mut self) -> Result<(), rt::Error> {
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation("abort_command"));
            };
            let Some(callback) = driver.abort_command else {
                return Err(rt::Error::UnsupportedOperation("abort_command"));
            };
            Self::status_to_result("abort_command", unsafe { callback(self.raw) })?;
            Ok(())
        })();
        self.normalize(rt::DeviceCaps::ABORT_COMMAND, "abort_command", result)
    }

    fn idle_driver(&mut self) -> Result<(), rt::Error> {
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation("idle"));
            };
            let Some(callback) = driver.idle else {
                return Err(rt::Error::UnsupportedOperation("idle"));
            };
            Self::status_to_result("idle", unsafe { callback(self.raw) })?;
            Ok(())
        })();
        self.normalize(rt::DeviceCaps::IDLE, "idle", result)
    }

    fn powerdown_driver(&mut self) -> Result<(), rt::Error> {
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation("powerdown"));
            };
            let Some(callback) = driver.powerdown else {
                return Err(rt::Error::UnsupportedOperation("powerdown"));
            };
            Self::status_to_result("powerdown", unsafe { callback(self.raw) })?;
            Ok(())
        })();
        self.normalize(rt::DeviceCaps::POWERDOWN, "powerdown", result)
    }
}

impl rt::TargetBackend for ExternalDevice {
    fn target_init_driver(
        &mut self,
        target: &mut rt::Target,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, rt::Error> {
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation("target_init"));
            };
            let Some(callback) = driver.target_init else {
                return Err(rt::Error::UnsupportedOperation("target_init"));
            };
            let mut raw_target = target_to_c(target);
            let count = Self::status_to_result("target_init", unsafe {
                callback(
                    self.raw,
                    ptr::addr_of_mut!(raw_target),
                    bytes_mut_ptr(rx),
                    rx.len(),
                    timeout,
                )
            })?;
            *target = target_from_c(ptr::addr_of!(raw_target));
            Ok(count as usize)
        })();
        self.normalize(rt::DeviceCaps::TARGET_INIT, "target_init", result)
    }

    fn target_send_bytes_driver(&mut self, tx: &[u8], timeout: i32) -> Result<usize, rt::Error> {
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation("target_send_bytes"));
            };
            let Some(callback) = driver.target_send_bytes else {
                return Err(rt::Error::UnsupportedOperation("target_send_bytes"));
            };
            let count = Self::status_to_result("target_send_bytes", unsafe {
                callback(self.raw, bytes_ptr(tx), tx.len(), timeout)
            })?;
            Ok(count as usize)
        })();
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
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation("target_receive_bytes"));
            };
            let Some(callback) = driver.target_receive_bytes else {
                return Err(rt::Error::UnsupportedOperation("target_receive_bytes"));
            };
            let count = Self::status_to_result("target_receive_bytes", unsafe {
                callback(self.raw, bytes_mut_ptr(rx), rx.len(), timeout)
            })?;
            Ok(count as usize)
        })();
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
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation("target_send_bits"));
            };
            let Some(callback) = driver.target_send_bits else {
                return Err(rt::Error::UnsupportedOperation("target_send_bits"));
            };
            let count = Self::status_to_result("target_send_bits", unsafe {
                callback(
                    self.raw,
                    bytes_ptr(tx),
                    tx_bits_len,
                    optional_bytes_ptr(tx_parity),
                )
            })?;
            Ok(count as usize)
        })();
        self.normalize(rt::DeviceCaps::TARGET_SEND_BITS, "target_send_bits", result)
    }

    fn target_receive_bits_driver(
        &mut self,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, rt::Error> {
        let result = (|| {
            let Some(driver) = self.driver_ref() else {
                return Err(rt::Error::UnsupportedOperation("target_receive_bits"));
            };
            let Some(callback) = driver.target_receive_bits else {
                return Err(rt::Error::UnsupportedOperation("target_receive_bits"));
            };
            let count = Self::status_to_result("target_receive_bits", unsafe {
                callback(
                    self.raw,
                    bytes_mut_ptr(rx),
                    rx.len(),
                    optional_bytes_mut_ptr(rx_parity),
                )
            })?;
            Ok(count as usize)
        })();
        self.normalize(
            rt::DeviceCaps::TARGET_RECEIVE_BITS,
            "target_receive_bits",
            result,
        )
    }
}

impl rt::Pn53xBackend for ExternalDevice {}
