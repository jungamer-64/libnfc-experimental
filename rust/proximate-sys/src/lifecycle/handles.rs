use super::{nfc_context, nfc_device};
use crate::c_abi::types::{nfc_baud_rate, nfc_modulation_type};
use crate::c_boundary::status::NFC_ESOFT;
use proximate_driver as rt;
use std::ffi::CString;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI32, Ordering};

type BaudRateQuery = (rt::Mode, rt::ModulationType);
type CachedBaudRates = (BaudRateQuery, Box<[nfc_baud_rate]>);

pub(crate) struct AbiContext {
    runtime: rt::Context,
}

impl AbiContext {
    pub(crate) fn new(runtime: rt::Context) -> Self {
        Self { runtime }
    }

    pub(crate) fn runtime(&self) -> &rt::Context {
        &self.runtime
    }
}

struct DeviceState {
    device: rt::Device,
    modulation_cache: Vec<(rt::Mode, Box<[nfc_modulation_type]>)>,
    baud_rate_cache: Vec<CachedBaudRates>,
}

pub(crate) struct AbiDevice {
    abort: Option<rt::CommandAbortHandle>,
    state: Mutex<DeviceState>,
    name: CString,
    connstring: CString,
    last_error: AtomicI32,
}

impl AbiDevice {
    pub(crate) fn new(device: rt::Device) -> Self {
        let name = c_string_prefix(device.name());
        let connstring = c_string_prefix(device.connstring().as_str());
        let abort = device.command_abort_handle();
        Self {
            abort,
            state: Mutex::new(DeviceState {
                device,
                modulation_cache: Vec::new(),
                baud_rate_cache: Vec::new(),
            }),
            name,
            connstring,
            last_error: AtomicI32::new(0),
        }
    }

    pub(crate) fn name_ptr(&self) -> *const libc::c_char {
        self.name.as_ptr()
    }

    pub(crate) fn connstring_ptr(&self) -> *const libc::c_char {
        self.connstring.as_ptr()
    }

    pub(crate) fn last_error(&self) -> i32 {
        self.last_error.load(Ordering::Acquire)
    }

    pub(crate) fn set_last_error(&self, value: i32) {
        self.last_error.store(value, Ordering::Release);
    }

    pub(crate) fn with_device<R>(
        &self,
        operation: impl FnOnce(&mut rt::Device) -> Result<R, rt::Error>,
    ) -> Result<R, rt::Error> {
        let mut state = self.state.lock().map_err(|_| self.poisoned())?;
        let _panic_state = PanicState(&self.last_error);
        let result = operation(&mut state.device);
        if result.is_ok() {
            self.set_last_error(0);
        }
        result
    }

    pub(crate) fn abort(&self) -> Result<(), rt::Error> {
        let result = self
            .abort
            .as_ref()
            .ok_or(rt::Error::UnsupportedOperation("abort_command"))?
            .abort();
        if result.is_ok() {
            self.set_last_error(0);
        }
        result
    }

    pub(crate) fn cache_modulations(
        &self,
        mode: rt::Mode,
        values: Box<[nfc_modulation_type]>,
    ) -> Result<*const nfc_modulation_type, rt::Error> {
        let mut state = self.state.lock().map_err(|_| self.poisoned())?;
        if let Some((_, cached)) = state
            .modulation_cache
            .iter()
            .find(|(cached_mode, _)| *cached_mode == mode)
        {
            return Ok(cached.as_ptr());
        }
        let pointer = values.as_ptr();
        state.modulation_cache.push((mode, values));
        Ok(pointer)
    }

    pub(crate) fn cache_baud_rates(
        &self,
        mode: rt::Mode,
        modulation_type: rt::ModulationType,
        values: Box<[nfc_baud_rate]>,
    ) -> Result<*const nfc_baud_rate, rt::Error> {
        let mut state = self.state.lock().map_err(|_| self.poisoned())?;
        if let Some((_, cached)) =
            state
                .baud_rate_cache
                .iter()
                .find(|((cached_mode, cached_type), _)| {
                    *cached_mode == mode && *cached_type == modulation_type
                })
        {
            return Ok(cached.as_ptr());
        }
        let pointer = values.as_ptr();
        state
            .baud_rate_cache
            .push(((mode, modulation_type), values));
        Ok(pointer)
    }

    fn poisoned(&self) -> rt::Error {
        self.set_last_error(NFC_ESOFT);
        rt::Error::DeviceOperationFailed {
            operation: "device mutex poisoned",
            code: NFC_ESOFT,
        }
    }
}

struct PanicState<'a>(&'a AtomicI32);

impl Drop for PanicState<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.store(NFC_ESOFT, Ordering::Release);
        }
    }
}

fn c_string_prefix(value: &str) -> CString {
    let prefix = value
        .as_bytes()
        .split(|byte| *byte == 0)
        .next()
        .unwrap_or(&[]);
    CString::new(prefix).expect("NUL bytes were removed")
}

/// Interprets a valid C ABI context handle as its private allocation.
///
/// # Safety
///
/// Non-null pointers must have been returned by this crate and must remain
/// live for the duration of the returned borrow.
pub(crate) unsafe fn context_ref<'a>(raw: *const nfc_context) -> Option<&'a AbiContext> {
    unsafe { raw.cast::<AbiContext>().as_ref() }
}

/// Interprets a valid C ABI device handle as its private allocation.
///
/// # Safety
///
/// Non-null pointers must have been returned by this crate and must remain
/// live for the duration of the returned borrow.
pub(crate) unsafe fn device_ref<'a>(raw: *const nfc_device) -> Option<&'a AbiDevice> {
    unsafe { raw.cast::<AbiDevice>().as_ref() }
}

pub(crate) fn context_into_raw(context: AbiContext) -> *mut nfc_context {
    Box::into_raw(Box::new(context)).cast()
}

pub(crate) fn device_into_raw(device: AbiDevice) -> *mut nfc_device {
    Box::into_raw(Box::new(device)).cast()
}

/// Reclaims the unique context allocation represented by an opaque handle.
///
/// # Safety
///
/// `raw` must be null or a live handle returned by `context_into_raw`, and it
/// must not be reclaimed more than once.
pub(crate) unsafe fn drop_context(raw: *mut nfc_context) {
    if !raw.is_null() {
        unsafe { drop(Box::from_raw(raw.cast::<AbiContext>())) };
    }
}

/// Reclaims the unique device allocation represented by an opaque handle.
///
/// # Safety
///
/// `raw` must be null or a live handle returned by `device_into_raw`, and it
/// must not be reclaimed more than once or concurrently with an operation.
pub(crate) unsafe fn drop_device(raw: *mut nfc_device) {
    if !raw.is_null() {
        unsafe { drop(Box::from_raw(raw.cast::<AbiDevice>())) };
    }
}
