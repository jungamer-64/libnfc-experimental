//! External C drivers are admitted here and converted into the Rust driver contract.
//!
//! The C descriptor and legacy context/device layouts never become runtime
//! authority. Registration snapshots the callback table, scan/open receive a
//! temporary context projection, and the returned legacy device allocation is
//! owned and dereferenced only by `ExternalDevice`.

use crate::c_abi::types::{nfc_baud_rate, nfc_modulation_type, nfc_target};
use crate::c_boundary::raw::{
    bounded_strlen, c_string_ptr_to_string, copy_bytes_to_c_buffer, fixed_c_buffer_to_string,
};
use crate::c_boundary::status::{NFC_EDEVNOTSUPP, NFC_EOVFLOW};
use crate::domain_bridge::decode::{baud_rate_from_c, modulation_type_from_c, target_from_c};
use crate::domain_bridge::encode::{
    baud_rate_to_c, dep_info_to_c, dep_mode_to_c, mode_to_c, modulation_to_c, modulation_type_to_c,
    property_to_c, target_to_c, timeout_property_to_c,
};
use crate::lifecycle::{
    DEVICE_NAME_LENGTH, MAX_USER_DEFINED_DEVICES, NFC_DRIVER_NAME_MAX, nfc_context, nfc_device,
    nfc_driver, scan_type_enum,
};
use crate::release_allocated_ptr;
use libc::{c_char, c_int, c_uint, c_void};
use proximate_driver as rt;
use std::ffi::CString;
use std::ptr;
use std::sync::Arc;

const DEFAULT_SCAN_CAPACITY: usize = 4;
const MAX_SCAN_CAPACITY: usize = 256;
const MAX_CAPABILITY_ENTRIES: usize = 64;

pub(crate) struct DriverSnapshot {
    name: String,
    _c_name: CString,
    scan_type: rt::ScanType,
    callbacks: nfc_driver,
}

// C function addresses are immutable code capabilities. `name` and scan type
// are owned values, and the copied descriptor's name pointer refers to the
// snapshot-owned `CString` for the snapshot's entire lifetime.
unsafe impl Send for DriverSnapshot {}
unsafe impl Sync for DriverSnapshot {}

impl DriverSnapshot {
    #[cfg(test)]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Copies a valid C registration descriptor into Rust-owned state.
    ///
    /// # Safety
    ///
    /// `raw` must point to a readable `nfc_driver` descriptor and every
    /// non-null callback address must remain executable until registry clear.
    pub(crate) unsafe fn from_raw(raw: *const nfc_driver) -> Result<Self, c_int> {
        let Some(descriptor) = (unsafe { raw.as_ref() }) else {
            return Err(crate::c_boundary::status::NFC_EINVARG);
        };
        let (name, c_name) = if descriptor.name.is_null() {
            (String::new(), CString::new("").expect("empty C string"))
        } else {
            let length = bounded_strlen(descriptor.name, NFC_DRIVER_NAME_MAX);
            if length == NFC_DRIVER_NAME_MAX {
                return Err(crate::c_boundary::status::NFC_EINVARG);
            }
            let bytes = unsafe { std::slice::from_raw_parts(descriptor.name.cast::<u8>(), length) };
            let c_name = CString::new(bytes).map_err(|_| crate::c_boundary::status::NFC_EINVARG)?;
            (String::from_utf8_lossy(bytes).into_owned(), c_name)
        };
        let scan_type = match descriptor.scan_type {
            scan_type_enum::NOT_INTRUSIVE => rt::ScanType::NotIntrusive,
            scan_type_enum::INTRUSIVE => rt::ScanType::Intrusive,
            scan_type_enum::NOT_AVAILABLE => rt::ScanType::NotAvailable,
            _ => return Err(crate::c_boundary::status::NFC_EINVARG),
        };
        let mut callbacks = *descriptor;
        callbacks.name = c_name.as_ptr();
        Ok(Self {
            name,
            _c_name: c_name,
            scan_type,
            callbacks,
        })
    }
}

pub(crate) struct ExternalDriver {
    snapshot: Arc<DriverSnapshot>,
}

impl ExternalDriver {
    pub(crate) fn new(snapshot: Arc<DriverSnapshot>) -> Self {
        Self { snapshot }
    }
}

#[repr(C)]
struct LegacyUserDefinedDevice {
    name: [c_char; DEVICE_NAME_LENGTH],
    connstring: [c_char; crate::c_boundary::NFC_BUFSIZE_CONNSTRING],
    optional: bool,
}

#[repr(C)]
struct LegacyContextView {
    allow_autoscan: bool,
    allow_intrusive_scan: bool,
    log_level: u32,
    user_defined_devices: [LegacyUserDefinedDevice; MAX_USER_DEFINED_DEVICES],
    user_defined_device_count: c_uint,
    runtime_data: *mut c_void,
}

impl LegacyContextView {
    fn from_runtime(context: &rt::Context) -> Self {
        let mut view: Self = unsafe { std::mem::zeroed() };
        view.allow_autoscan = context.config.allow_autoscan;
        view.allow_intrusive_scan = context.config.allow_intrusive_scan;
        view.log_level = context.config.log_level;
        for (index, configured) in context
            .config
            .user_defined_devices
            .iter()
            .take(MAX_USER_DEFINED_DEVICES)
            .enumerate()
        {
            let slot = &mut view.user_defined_devices[index];
            unsafe {
                copy_bytes_to_c_buffer(
                    slot.name.as_mut_ptr(),
                    slot.name.len(),
                    configured.name.as_bytes(),
                );
                copy_bytes_to_c_buffer(
                    slot.connstring.as_mut_ptr(),
                    slot.connstring.len(),
                    configured.connstring.as_str().as_bytes(),
                );
            }
            slot.optional = configured.optional;
            view.user_defined_device_count += 1;
        }
        view
    }

    fn as_opaque(&self) -> *const nfc_context {
        ptr::from_ref(self).cast()
    }
}

#[allow(non_snake_case)]
#[repr(C)]
struct LegacyDeviceView {
    context: *const nfc_context,
    driver: *const nfc_driver,
    driver_data: *mut c_void,
    chip_data: *mut c_void,
    command_abort: *mut c_void,
    name: [c_char; DEVICE_NAME_LENGTH],
    connstring: [c_char; crate::c_boundary::NFC_BUFSIZE_CONNSTRING],
    bCrc: bool,
    bPar: bool,
    bEasyFraming: bool,
    bInfiniteSelect: bool,
    bAutoIso14443_4: bool,
    btSupportByte: u8,
    last_error: c_int,
}

impl rt::Driver for ExternalDriver {
    fn name(&self) -> &str {
        &self.snapshot.name
    }

    fn scan_type(&self) -> rt::ScanType {
        self.snapshot.scan_type
    }

    fn scan(&self, context: &rt::Context) -> Result<rt::DriverScan, rt::Error> {
        let Some(scan) = self.snapshot.callbacks.scan else {
            return Err(missing_capability("scan"));
        };
        let context_view = LegacyContextView::from_runtime(context);
        let mut capacity = DEFAULT_SCAN_CAPACITY;
        loop {
            let mut buffer = vec![[0; crate::c_boundary::NFC_BUFSIZE_CONNSTRING]; capacity];
            let found = unsafe { scan(context_view.as_opaque(), buffer.as_mut_ptr(), capacity) };
            if found >= capacity && capacity < MAX_SCAN_CAPACITY {
                capacity = found.max(capacity * 2).min(MAX_SCAN_CAPACITY);
                continue;
            }
            let mut devices = Vec::new();
            for raw in buffer.iter().take(found.min(capacity)) {
                let value = fixed_c_buffer_to_string(raw);
                if value.is_empty() {
                    continue;
                }
                let connstring = rt::ConnectionString::new(value)?;
                devices.push(self.describe_discovered(connstring.as_str().to_string(), connstring));
            }
            return Ok(rt::DriverScan::Complete(devices));
        }
    }

    fn open(
        &self,
        context: &rt::Context,
        connstring: &rt::ConnectionString,
    ) -> Result<Box<dyn rt::DeviceHandle>, rt::Error> {
        let Some(open) = self.snapshot.callbacks.open else {
            return Err(missing_capability("open"));
        };
        let context_view = LegacyContextView::from_runtime(context);
        let connstring_c = CString::new(connstring.as_str())
            .map_err(|_| rt::Error::InvalidEncoding("connstring"))?;
        let raw = unsafe { open(context_view.as_opaque(), connstring_c.as_ptr()) };
        if raw.is_null() {
            return Err(rt::Error::DriverOpenFailed(connstring.as_str().to_string()));
        }
        Ok(Box::new(ExternalDevice::new(
            Arc::clone(&self.snapshot),
            raw,
        )))
    }
}

struct ExternalAbort {
    raw: usize,
    callback: unsafe extern "C" fn(*mut nfc_device) -> c_int,
}

impl rt::CommandAbort for ExternalAbort {
    fn abort(&self) -> Result<(), rt::Error> {
        let status = unsafe { (self.callback)(self.raw as *mut nfc_device) };
        ExternalDevice::status_to_result("abort_command", status).map(|_| ())
    }
}

struct ExternalDevice {
    snapshot: Arc<DriverSnapshot>,
    raw: *mut nfc_device,
    name: String,
    connstring: rt::ConnectionString,
}

unsafe impl Send for ExternalDevice {}

impl ExternalDevice {
    fn new(snapshot: Arc<DriverSnapshot>, raw: *mut nfc_device) -> Self {
        let view = unsafe { &mut *raw.cast::<LegacyDeviceView>() };
        let name = fixed_c_buffer_to_string(&view.name);
        let connstring = fixed_c_buffer_to_string(&view.connstring);
        view.context = ptr::null();
        view.driver = ptr::from_ref(&snapshot.callbacks);
        let connstring = rt::ConnectionString::new(if connstring.is_empty() {
            "unknown".to_string()
        } else {
            connstring
        })
        .unwrap_or_else(|_| rt::ConnectionString::new("unknown").expect("valid fallback"));
        Self {
            snapshot,
            raw,
            name,
            connstring,
        }
    }

    fn view(&self) -> &LegacyDeviceView {
        unsafe { &*self.raw.cast::<LegacyDeviceView>() }
    }

    fn view_mut(&mut self) -> &mut LegacyDeviceView {
        unsafe { &mut *self.raw.cast::<LegacyDeviceView>() }
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

    fn bounded_count_to_result(
        operation: &'static str,
        status: c_int,
        capacity: usize,
    ) -> Result<usize, rt::Error> {
        let count = Self::status_to_result(operation, status)? as usize;
        if count > capacity {
            Err(rt::Error::DeviceOperationFailed {
                operation,
                code: NFC_EOVFLOW,
            })
        } else {
            Ok(count)
        }
    }

    fn normalize<T>(
        &mut self,
        operation: &'static str,
        result: Result<T, rt::Error>,
    ) -> Result<T, rt::Error> {
        match result {
            Ok(value) => {
                self.view_mut().last_error = 0;
                Ok(value)
            }
            Err(rt::Error::UnsupportedOperation(_)) => {
                self.view_mut().last_error = NFC_EDEVNOTSUPP;
                Err(missing_capability(operation))
            }
            Err(error @ rt::Error::DeviceOperationFailed { code, .. }) => {
                self.view_mut().last_error = code;
                Err(error)
            }
            Err(error) => Err(error),
        }
    }

    fn simple_status(
        &mut self,
        operation: &'static str,
        callback: Option<unsafe extern "C" fn(*mut nfc_device) -> c_int>,
    ) -> Result<c_int, rt::Error> {
        let result = callback
            .ok_or(rt::Error::UnsupportedOperation(operation))
            .and_then(|callback| Self::status_to_result(operation, unsafe { callback(self.raw) }));
        self.normalize(operation, result)
    }
}

impl Drop for ExternalDevice {
    fn drop(&mut self) {
        let raw = std::mem::replace(&mut self.raw, ptr::null_mut());
        if !raw.is_null()
            && let Some(close) = self.snapshot.callbacks.close
        {
            unsafe { close(raw) };
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

    fn last_error(&self) -> i32 {
        self.view().last_error
    }

    fn strerror(&self) -> String {
        let Some(callback) = self.snapshot.callbacks.strerror else {
            return rt::device_error_message(self.last_error()).to_string();
        };
        let raw = unsafe { callback(self.raw.cast_const()) };
        if raw.is_null() {
            rt::device_error_message(self.last_error()).to_string()
        } else {
            c_string_ptr_to_string(raw, bounded_strlen(raw, 256))
        }
    }

    fn missing_capability(&mut self, operation: &'static str) -> rt::Error {
        self.view_mut().last_error = NFC_EDEVNOTSUPP;
        missing_capability(operation)
    }
}

impl rt::InfoBackend for ExternalDevice {
    fn information_about(&mut self) -> Result<String, rt::Error> {
        let result = (|| {
            let callback = self.snapshot.callbacks.device_get_information_about.ok_or(
                rt::Error::UnsupportedOperation("device_get_information_about"),
            )?;
            let mut buffer = ptr::null_mut();
            Self::status_to_result("device_get_information_about", unsafe {
                callback(self.raw, ptr::addr_of_mut!(buffer))
            })?;
            let value = if buffer.is_null() {
                String::new()
            } else {
                c_string_ptr_to_string(buffer, bounded_strlen(buffer, 4096))
            };
            unsafe { release_allocated_ptr(buffer.cast()) };
            Ok(value)
        })();
        self.normalize("device_get_information_about", result)
    }
}

impl rt::PropertyBackend for ExternalDevice {
    fn set_property_bool(&mut self, property: rt::Property, enable: bool) -> Result<(), rt::Error> {
        let result = (|| {
            let callback = self
                .snapshot
                .callbacks
                .device_set_property_bool
                .ok_or(rt::Error::UnsupportedOperation("device_set_property_bool"))?;
            Self::status_to_result("device_set_property_bool", unsafe {
                callback(self.raw, property_to_c(property), enable)
            })?;
            Ok(())
        })();
        self.normalize("device_set_property_bool", result)
    }

    fn set_timeout(
        &mut self,
        property: rt::TimeoutProperty,
        timeout: rt::OperationTimeout,
    ) -> Result<(), rt::Error> {
        let result = (|| {
            let callback = self
                .snapshot
                .callbacks
                .device_set_property_int
                .ok_or(rt::Error::UnsupportedOperation("device_set_property_int"))?;
            Self::status_to_result("device_set_property_int", unsafe {
                callback(
                    self.raw,
                    timeout_property_to_c(property),
                    timeout.configured_millis()?,
                )
            })?;
            Ok(())
        })();
        self.normalize("device_set_property_int", result)
    }

    fn supported_modulations(
        &mut self,
        mode: rt::Mode,
    ) -> Result<Vec<rt::ModulationType>, rt::Error> {
        let result = (|| {
            let callback = self
                .snapshot
                .callbacks
                .get_supported_modulation
                .ok_or(rt::Error::UnsupportedOperation("get_supported_modulation"))?;
            let mut values = ptr::null();
            Self::status_to_result("get_supported_modulation", unsafe {
                callback(self.raw, mode_to_c(mode), ptr::addr_of_mut!(values))
            })?;
            decode_modulation_array(values)
        })();
        self.normalize("get_supported_modulation", result)
    }

    fn supported_baud_rates(
        &mut self,
        mode: rt::Mode,
        modulation_type: rt::ModulationType,
    ) -> Result<Vec<rt::BaudRate>, rt::Error> {
        let result = (|| {
            let callback = self
                .snapshot
                .callbacks
                .get_supported_baud_rate
                .ok_or(rt::Error::UnsupportedOperation("get_supported_baud_rate"))?;
            let mut values = ptr::null();
            Self::status_to_result("get_supported_baud_rate", unsafe {
                callback(
                    self.raw,
                    mode_to_c(mode),
                    modulation_type_to_c(modulation_type),
                    ptr::addr_of_mut!(values),
                )
            })?;
            decode_baud_rate_array(values)
        })();
        self.normalize("get_supported_baud_rate", result)
    }

    fn property_bool_state(&self, property: rt::Property) -> Option<bool> {
        let view = self.view();
        Some(match property {
            rt::Property::HandleCrc => view.bCrc,
            rt::Property::HandleParity => view.bPar,
            rt::Property::EasyFraming => view.bEasyFraming,
            rt::Property::InfiniteSelect => view.bInfiniteSelect,
            rt::Property::AutoIso14443_4 => view.bAutoIso14443_4,
            _ => return None,
        })
    }
}

impl rt::InitiatorBackend for ExternalDevice {
    fn command_abort_handle(&self) -> Option<rt::CommandAbortHandle> {
        self.snapshot.callbacks.abort_command.map(|callback| {
            Arc::new(ExternalAbort {
                raw: self.raw as usize,
                callback,
            }) as rt::CommandAbortHandle
        })
    }

    fn initiator_init_driver(&mut self) -> Result<i32, rt::Error> {
        self.simple_status("initiator_init", self.snapshot.callbacks.initiator_init)
    }

    fn initiator_init_secure_element_driver(&mut self) -> Result<i32, rt::Error> {
        self.simple_status(
            "initiator_init_secure_element",
            self.snapshot.callbacks.initiator_init_secure_element,
        )
    }

    fn select_passive_target_driver(
        &mut self,
        modulation: rt::Modulation,
        init_data: &[u8],
    ) -> Result<Option<rt::Target>, rt::Error> {
        let result = (|| {
            let callback = self
                .snapshot
                .callbacks
                .initiator_select_passive_target
                .ok_or(rt::Error::UnsupportedOperation(
                    "initiator_select_passive_target",
                ))?;
            let mut target = unsafe { std::mem::zeroed::<nfc_target>() };
            let status = Self::status_to_result("initiator_select_passive_target", unsafe {
                callback(
                    self.raw,
                    modulation_to_c(modulation),
                    bytes_ptr(init_data),
                    init_data.len(),
                    ptr::addr_of_mut!(target),
                )
            })?;
            (status != 0)
                .then(|| target_from_c(ptr::addr_of!(target)))
                .transpose()
        })();
        self.normalize("initiator_select_passive_target", result)
    }

    fn poll_target_driver(
        &mut self,
        modulations: &[rt::Modulation],
        iterations: rt::PollIterations,
        period: rt::PollPeriod,
    ) -> Result<Option<rt::Target>, rt::Error> {
        let result = (|| {
            let callback = self
                .snapshot
                .callbacks
                .initiator_poll_target
                .ok_or(rt::Error::UnsupportedOperation("initiator_poll_target"))?;
            let raw_modulations: Vec<_> =
                modulations.iter().copied().map(modulation_to_c).collect();
            let mut target = unsafe { std::mem::zeroed::<nfc_target>() };
            let status = Self::status_to_result("initiator_poll_target", unsafe {
                callback(
                    self.raw,
                    raw_modulations.as_ptr(),
                    raw_modulations.len(),
                    iterations.to_libnfc(),
                    period.get(),
                    ptr::addr_of_mut!(target),
                )
            })?;
            (status != 0)
                .then(|| target_from_c(ptr::addr_of!(target)))
                .transpose()
        })();
        self.normalize("initiator_poll_target", result)
    }

    fn select_dep_target_driver(
        &mut self,
        mode: rt::DepMode,
        baud_rate: rt::BaudRate,
        initiator: Option<&rt::DepInfo>,
        timeout: rt::OperationTimeout,
    ) -> Result<Option<rt::Target>, rt::Error> {
        let result = (|| {
            let callback = self.snapshot.callbacks.initiator_select_dep_target.ok_or(
                rt::Error::UnsupportedOperation("initiator_select_dep_target"),
            )?;
            let raw_initiator = initiator.map(dep_info_to_c);
            let mut target = unsafe { std::mem::zeroed::<nfc_target>() };
            let status = Self::status_to_result("initiator_select_dep_target", unsafe {
                callback(
                    self.raw,
                    dep_mode_to_c(mode),
                    baud_rate_to_c(baud_rate),
                    raw_initiator.as_ref().map_or(ptr::null(), ptr::from_ref),
                    ptr::addr_of_mut!(target),
                    timeout.to_libnfc_millis(),
                )
            })?;
            (status != 0)
                .then(|| target_from_c(ptr::addr_of!(target)))
                .transpose()
        })();
        self.normalize("initiator_select_dep_target", result)
    }

    fn deselect_target_driver(&mut self) -> Result<(), rt::Error> {
        self.simple_status(
            "initiator_deselect_target",
            self.snapshot.callbacks.initiator_deselect_target,
        )
        .map(|_| ())
    }

    fn target_is_present_driver(&mut self, target: Option<&rt::Target>) -> Result<bool, rt::Error> {
        let result = (|| {
            let callback = self.snapshot.callbacks.initiator_target_is_present.ok_or(
                rt::Error::UnsupportedOperation("initiator_target_is_present"),
            )?;
            let raw_target = target.map(target_to_c);
            Self::status_to_result("initiator_target_is_present", unsafe {
                callback(
                    self.raw,
                    raw_target.as_ref().map_or(ptr::null(), ptr::from_ref),
                )
            })?;
            Ok(true)
        })();
        self.normalize("initiator_target_is_present", result)
    }

    fn transceive_bytes_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: rt::OperationTimeout,
    ) -> Result<usize, rt::Error> {
        let result = (|| {
            let callback = self.snapshot.callbacks.initiator_transceive_bytes.ok_or(
                rt::Error::UnsupportedOperation("initiator_transceive_bytes"),
            )?;
            Self::bounded_count_to_result(
                "initiator_transceive_bytes",
                unsafe {
                    callback(
                        self.raw,
                        bytes_ptr(tx),
                        tx.len(),
                        bytes_mut_ptr(rx),
                        rx.len(),
                        timeout.to_libnfc_millis(),
                    )
                },
                rx.len(),
            )
        })();
        self.normalize("initiator_transceive_bytes", result)
    }

    fn transceive_bits_driver(
        &mut self,
        tx: rt::BitFrame<'_>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, rt::Error> {
        let result = (|| {
            let callback = self
                .snapshot
                .callbacks
                .initiator_transceive_bits
                .ok_or(rt::Error::UnsupportedOperation("initiator_transceive_bits"))?;
            Self::bounded_count_to_result(
                "initiator_transceive_bits",
                unsafe {
                    callback(
                        self.raw,
                        bytes_ptr(tx.bytes()),
                        tx.bit_len(),
                        optional_bytes_ptr(tx.parity()),
                        bytes_mut_ptr(rx),
                        optional_bytes_mut_ptr(rx_parity),
                    )
                },
                rx.len().saturating_mul(8),
            )
        })();
        self.normalize("initiator_transceive_bits", result)
    }

    fn transceive_bytes_timed_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        max_cycles: rt::TimerCycles,
    ) -> Result<(usize, rt::TimerCycles), rt::Error> {
        let result = (|| {
            let callback = self
                .snapshot
                .callbacks
                .initiator_transceive_bytes_timed
                .ok_or(rt::Error::UnsupportedOperation(
                    "initiator_transceive_bytes_timed",
                ))?;
            let mut cycles = max_cycles.get();
            let count = Self::bounded_count_to_result(
                "initiator_transceive_bytes_timed",
                unsafe {
                    callback(
                        self.raw,
                        bytes_ptr(tx),
                        tx.len(),
                        bytes_mut_ptr(rx),
                        rx.len(),
                        ptr::addr_of_mut!(cycles),
                    )
                },
                rx.len(),
            )?;
            Ok((count, rt::TimerCycles::new(cycles)))
        })();
        self.normalize("initiator_transceive_bytes_timed", result)
    }

    fn transceive_bits_timed_driver(
        &mut self,
        tx: rt::BitFrame<'_>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
        max_cycles: rt::TimerCycles,
    ) -> Result<(usize, rt::TimerCycles), rt::Error> {
        let result = (|| {
            let callback = self
                .snapshot
                .callbacks
                .initiator_transceive_bits_timed
                .ok_or(rt::Error::UnsupportedOperation(
                    "initiator_transceive_bits_timed",
                ))?;
            let mut cycles = max_cycles.get();
            let count = Self::bounded_count_to_result(
                "initiator_transceive_bits_timed",
                unsafe {
                    callback(
                        self.raw,
                        bytes_ptr(tx.bytes()),
                        tx.bit_len(),
                        optional_bytes_ptr(tx.parity()),
                        bytes_mut_ptr(rx),
                        optional_bytes_mut_ptr(rx_parity),
                        ptr::addr_of_mut!(cycles),
                    )
                },
                rx.len().saturating_mul(8),
            )?;
            Ok((count, rt::TimerCycles::new(cycles)))
        })();
        self.normalize("initiator_transceive_bits_timed", result)
    }

    fn abort_command_driver(&mut self) -> Result<(), rt::Error> {
        self.simple_status("abort_command", self.snapshot.callbacks.abort_command)
            .map(|_| ())
    }

    fn idle_driver(&mut self) -> Result<(), rt::Error> {
        self.simple_status("idle", self.snapshot.callbacks.idle)
            .map(|_| ())
    }

    fn powerdown_driver(&mut self) -> Result<(), rt::Error> {
        self.simple_status("powerdown", self.snapshot.callbacks.powerdown)
            .map(|_| ())
    }
}

impl rt::TargetBackend for ExternalDevice {
    fn target_init_driver(
        &mut self,
        target: &mut rt::Target,
        rx: &mut [u8],
        timeout: rt::OperationTimeout,
    ) -> Result<usize, rt::Error> {
        let result = (|| {
            let callback = self
                .snapshot
                .callbacks
                .target_init
                .ok_or(rt::Error::UnsupportedOperation("target_init"))?;
            let mut raw_target = target_to_c(target);
            let count = Self::bounded_count_to_result(
                "target_init",
                unsafe {
                    callback(
                        self.raw,
                        ptr::addr_of_mut!(raw_target),
                        bytes_mut_ptr(rx),
                        rx.len(),
                        timeout.to_libnfc_millis(),
                    )
                },
                rx.len(),
            )?;
            *target = target_from_c(ptr::addr_of!(raw_target))?;
            Ok(count)
        })();
        self.normalize("target_init", result)
    }

    fn target_send_bytes_driver(
        &mut self,
        tx: &[u8],
        timeout: rt::OperationTimeout,
    ) -> Result<usize, rt::Error> {
        let result = (|| {
            let callback = self
                .snapshot
                .callbacks
                .target_send_bytes
                .ok_or(rt::Error::UnsupportedOperation("target_send_bytes"))?;
            Self::bounded_count_to_result(
                "target_send_bytes",
                unsafe {
                    callback(
                        self.raw,
                        bytes_ptr(tx),
                        tx.len(),
                        timeout.to_libnfc_millis(),
                    )
                },
                tx.len(),
            )
        })();
        self.normalize("target_send_bytes", result)
    }

    fn target_receive_bytes_driver(
        &mut self,
        rx: &mut [u8],
        timeout: rt::OperationTimeout,
    ) -> Result<usize, rt::Error> {
        let result = (|| {
            let callback = self
                .snapshot
                .callbacks
                .target_receive_bytes
                .ok_or(rt::Error::UnsupportedOperation("target_receive_bytes"))?;
            Self::bounded_count_to_result(
                "target_receive_bytes",
                unsafe {
                    callback(
                        self.raw,
                        bytes_mut_ptr(rx),
                        rx.len(),
                        timeout.to_libnfc_millis(),
                    )
                },
                rx.len(),
            )
        })();
        self.normalize("target_receive_bytes", result)
    }

    fn target_send_bits_driver(&mut self, tx: rt::BitFrame<'_>) -> Result<usize, rt::Error> {
        let result = (|| {
            let callback = self
                .snapshot
                .callbacks
                .target_send_bits
                .ok_or(rt::Error::UnsupportedOperation("target_send_bits"))?;
            Self::bounded_count_to_result(
                "target_send_bits",
                unsafe {
                    callback(
                        self.raw,
                        bytes_ptr(tx.bytes()),
                        tx.bit_len(),
                        optional_bytes_ptr(tx.parity()),
                    )
                },
                tx.bit_len(),
            )
        })();
        self.normalize("target_send_bits", result)
    }

    fn target_receive_bits_driver(
        &mut self,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, rt::Error> {
        let result = (|| {
            let callback = self
                .snapshot
                .callbacks
                .target_receive_bits
                .ok_or(rt::Error::UnsupportedOperation("target_receive_bits"))?;
            Self::bounded_count_to_result(
                "target_receive_bits",
                unsafe {
                    callback(
                        self.raw,
                        bytes_mut_ptr(rx),
                        rx.len(),
                        optional_bytes_mut_ptr(rx_parity),
                    )
                },
                rx.len().saturating_mul(8),
            )
        })();
        self.normalize("target_receive_bits", result)
    }
}

impl rt::Pn53xBackend for ExternalDevice {}

fn missing_capability(operation: &'static str) -> rt::Error {
    rt::Error::MissingCapability(operation)
}

fn decode_modulation_array(
    raw: *const nfc_modulation_type,
) -> Result<Vec<rt::ModulationType>, rt::Error> {
    if raw.is_null() {
        return Ok(Vec::new());
    }
    let mut decoded = Vec::new();
    for index in 0..MAX_CAPABILITY_ENTRIES {
        let value = unsafe { raw.add(index).read() };
        if value == nfc_modulation_type::NMT_UNDEFINED {
            return Ok(decoded);
        }
        decoded.push(modulation_type_from_c(value)?);
    }
    Err(rt::Error::InvalidEncoding(
        "unterminated modulation capability array",
    ))
}

fn decode_baud_rate_array(raw: *const nfc_baud_rate) -> Result<Vec<rt::BaudRate>, rt::Error> {
    if raw.is_null() {
        return Ok(Vec::new());
    }
    let mut decoded = Vec::new();
    for index in 0..MAX_CAPABILITY_ENTRIES {
        let value = unsafe { raw.add(index).read() };
        if value == nfc_baud_rate::NBR_UNDEFINED {
            return Ok(decoded);
        }
        decoded.push(baud_rate_from_c(value)?);
    }
    Err(rt::Error::InvalidEncoding(
        "unterminated baud-rate capability array",
    ))
}

fn bytes_ptr(bytes: &[u8]) -> *const u8 {
    if bytes.is_empty() {
        ptr::null()
    } else {
        bytes.as_ptr()
    }
}

fn bytes_mut_ptr(bytes: &mut [u8]) -> *mut u8 {
    if bytes.is_empty() {
        ptr::null_mut()
    } else {
        bytes.as_mut_ptr()
    }
}

fn optional_bytes_ptr(bytes: Option<&[u8]>) -> *const u8 {
    bytes.map_or(ptr::null(), bytes_ptr)
}

fn optional_bytes_mut_ptr(bytes: Option<&mut [u8]>) -> *mut u8 {
    bytes.map_or(ptr::null_mut(), bytes_mut_ptr)
}
