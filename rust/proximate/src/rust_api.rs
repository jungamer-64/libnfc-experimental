use crate::{NFC_BUFSIZE_CONNSTRING, nfc_build_connstring, nfc_parse_connstring};
#[cfg(feature = "secure")]
use crate::{
    NFC_SECURE_ERROR_INVALID, NFC_SECURE_ERROR_OVERFLOW, NFC_SECURE_ERROR_RANGE,
    NFC_SECURE_ERROR_ZERO_SIZE, nfc_safe_memcpy, nfc_safe_memmove, nfc_secure_zero,
};
use std::ffi::{CStr, CString};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidArgument(&'static str),
    InvalidEncoding(&'static str),
    BufferTooSmall { needed: usize, available: usize },
    InvalidConnectionString(String),
    DriverNotFound(String),
    DriverOpenFailed(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecureError {
    Invalid,
    Overflow,
    Range,
    ZeroSize,
    Internal(i32),
}

impl SecureError {
    #[cfg(feature = "secure")]
    fn from_code(code: i32) -> Self {
        match code {
            NFC_SECURE_ERROR_INVALID => Self::Invalid,
            NFC_SECURE_ERROR_OVERFLOW => Self::Overflow,
            NFC_SECURE_ERROR_RANGE => Self::Range,
            NFC_SECURE_ERROR_ZERO_SIZE => Self::ZeroSize,
            other => Self::Internal(other),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionString(String);

impl ConnectionString {
    pub fn new(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        validate_connstring(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ConnectionString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedConnectionString {
    pub match_depth: i32,
    pub param1: Option<String>,
    pub param2: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserDefinedDevice {
    pub name: String,
    pub connstring: ConnectionString,
    pub optional: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextConfig {
    pub allow_autoscan: bool,
    pub allow_intrusive_scan: bool,
    pub log_level: u32,
    pub user_defined_devices: Vec<UserDefinedDevice>,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            allow_autoscan: true,
            allow_intrusive_scan: false,
            log_level: if cfg!(libnfc_debug) { 3 } else { 1 },
            user_defined_devices: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Context {
    pub config: ContextConfig,
}

impl Context {
    pub fn new() -> Self {
        Self {
            config: ContextConfig::default(),
        }
    }

    pub fn with_config(config: ContextConfig) -> Self {
        Self { config }
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanType {
    NotIntrusive,
    Intrusive,
    NotAvailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Property {
    TimeoutCommand,
    TimeoutAtr,
    TimeoutCom,
    HandleCrc,
    HandleParity,
    ActivateField,
    ActivateCrypto1,
    InfiniteSelect,
    AcceptInvalidFrames,
    AcceptMultipleFrames,
    AutoIso14443_4,
    EasyFraming,
    ForceIso14443A,
    ForceIso14443B,
    ForceSpeed106,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DepMode {
    Undefined,
    Passive,
    Active,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BaudRate {
    Undefined,
    Br106,
    Br212,
    Br424,
    Br847,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModulationType {
    Undefined,
    Iso14443A,
    Jewel,
    Iso14443B,
    Iso14443Bi,
    Iso14443B2Sr,
    Iso14443B2Ct,
    Felica,
    Dep,
    Barcode,
    Iso14443BiClass,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Target,
    Initiator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Modulation {
    pub modulation_type: ModulationType,
    pub baud_rate: BaudRate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Target {
    pub modulation: Modulation,
}

pub trait Logger: Send + Sync {
    fn log(&self, _priority: u8, _message: &str) {}
}

pub trait OpenedDevice: Send {
    fn name(&self) -> &str;
    fn connstring(&self) -> &ConnectionString;
}

pub trait Driver: Send + Sync {
    fn name(&self) -> &str;
    fn scan_type(&self) -> ScanType;
    fn scan(&self, context: &Context) -> Result<Vec<ConnectionString>, Error>;
    fn open(
        &self,
        context: &Context,
        connstring: &ConnectionString,
    ) -> Result<Box<dyn OpenedDevice>, Error>;
}

pub struct Device {
    pub name: String,
    pub connstring: ConnectionString,
    pub last_error: i32,
    handle: Box<dyn OpenedDevice>,
}

impl Device {
    pub fn handle(&self) -> &dyn OpenedDevice {
        self.handle.as_ref()
    }
}

#[derive(Default)]
pub struct DriverRegistry {
    drivers: Vec<Box<dyn Driver>>,
}

impl DriverRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_driver(&mut self, driver: Box<dyn Driver>) {
        self.drivers.push(driver);
    }

    pub fn is_empty(&self) -> bool {
        self.drivers.is_empty()
    }

    pub fn list_devices(&self, context: &Context) -> Result<Vec<ConnectionString>, Error> {
        let mut devices = Vec::new();
        for driver in &self.drivers {
            let mut scanned = driver.scan(context)?;
            devices.append(&mut scanned);
        }
        Ok(devices)
    }

    pub fn open(
        &self,
        context: &Context,
        connstring: Option<&ConnectionString>,
    ) -> Result<Device, Error> {
        let requested = if let Some(connstring) = connstring {
            connstring.clone()
        } else {
            self.list_devices(context)?
                .into_iter()
                .next()
                .ok_or_else(|| Error::DriverNotFound("no device available".to_string()))?
        };

        let driver = self
            .drivers
            .iter()
            .find(|driver| driver_matches_connstring(driver.as_ref(), &requested))
            .ok_or_else(|| Error::DriverNotFound(requested.as_str().to_string()))?;

        let handle = driver.open(context, &requested)?;
        let name = handle.name().to_string();
        let connstring = handle.connstring().clone();

        Ok(Device {
            name,
            connstring,
            last_error: 0,
            handle,
        })
    }
}

pub fn parse_connstring(connstring: &str, prefix: &str, param_name: &str) -> Result<String, Error> {
    let connstring = cstring(connstring, "connstring")?;
    let prefix = cstring(prefix, "prefix")?;
    let param_name = cstring(param_name, "param_name")?;
    let mut buffer = vec![0i8; NFC_BUFSIZE_CONNSTRING];
    let rc = unsafe {
        nfc_parse_connstring(
            connstring.as_ptr(),
            prefix.as_ptr(),
            param_name.as_ptr(),
            buffer.as_mut_ptr(),
            buffer.len(),
        )
    };
    if rc != 0 {
        return Err(Error::InvalidConnectionString(cstring_from_buf(&buffer)));
    }

    Ok(cstring_from_buf(&buffer))
}

pub fn build_connstring(
    driver_name: &str,
    param_name: &str,
    param_value: &str,
) -> Result<ConnectionString, Error> {
    let driver_name = cstring(driver_name, "driver_name")?;
    let param_name = cstring(param_name, "param_name")?;
    let param_value = cstring(param_value, "param_value")?;
    let mut buffer = vec![0i8; NFC_BUFSIZE_CONNSTRING];
    let rc = unsafe {
        nfc_build_connstring(
            buffer.as_mut_ptr(),
            buffer.len(),
            driver_name.as_ptr(),
            param_name.as_ptr(),
            param_value.as_ptr(),
        )
    };
    if rc != 0 {
        return Err(Error::BufferTooSmall {
            needed: driver_name.as_bytes().len()
                + param_name.as_bytes().len()
                + param_value.as_bytes().len()
                + 3,
            available: buffer.len(),
        });
    }

    ConnectionString::new(cstring_from_buf(&buffer))
}

pub fn decode_connstring(
    connstring: &ConnectionString,
    driver_name: &str,
    bus_name: &str,
) -> Result<DecodedConnectionString, Error> {
    let connstring = cstring(connstring.as_str(), "connstring")?;
    let driver_name = cstring(driver_name, "driver_name")?;
    let bus_name = cstring(bus_name, "bus_name")?;
    let mut param1 = std::ptr::null_mut();
    let mut param2 = std::ptr::null_mut();
    let match_depth = unsafe {
        crate::connstring_decode(
            connstring.as_ptr(),
            driver_name.as_ptr(),
            bus_name.as_ptr(),
            &mut param1,
            &mut param2,
        )
    };
    let decoded = DecodedConnectionString {
        match_depth,
        param1: owned_c_string(param1),
        param2: owned_c_string(param2),
    };
    Ok(decoded)
}

#[cfg(feature = "secure")]
pub fn safe_memcpy(dst: &mut [u8], src: &[u8]) -> Result<(), SecureError> {
    let rc = unsafe {
        nfc_safe_memcpy(
            dst.as_mut_ptr().cast(),
            dst.len(),
            src.as_ptr().cast(),
            src.len(),
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(SecureError::from_code(rc))
    }
}

#[cfg(feature = "secure")]
pub fn safe_memmove(dst: &mut [u8], src: &[u8]) -> Result<(), SecureError> {
    let rc = unsafe {
        nfc_safe_memmove(
            dst.as_mut_ptr().cast(),
            dst.len(),
            src.as_ptr().cast(),
            src.len(),
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(SecureError::from_code(rc))
    }
}

#[cfg(feature = "secure")]
pub fn secure_zero(bytes: &mut [u8]) -> Result<(), SecureError> {
    let rc = unsafe { nfc_secure_zero(bytes.as_mut_ptr().cast(), bytes.len()) };
    if rc == 0 {
        Ok(())
    } else {
        Err(SecureError::from_code(rc))
    }
}

fn validate_connstring(value: &str) -> Result<(), Error> {
    if value.is_empty() {
        return Err(Error::InvalidConnectionString(
            "connection string cannot be empty".to_string(),
        ));
    }
    if value.len() >= NFC_BUFSIZE_CONNSTRING {
        return Err(Error::BufferTooSmall {
            needed: value.len() + 1,
            available: NFC_BUFSIZE_CONNSTRING,
        });
    }
    if value.as_bytes().iter().any(|byte| byte.is_ascii_control()) {
        return Err(Error::InvalidConnectionString(
            "connection string contains control characters".to_string(),
        ));
    }
    Ok(())
}

fn driver_matches_connstring(driver: &dyn Driver, connstring: &ConnectionString) -> bool {
    let name = driver.name();
    connstring.as_str().starts_with(name)
        || (connstring.as_str().starts_with("usb") && name.ends_with("_usb"))
}

fn cstring(value: &str, context: &'static str) -> Result<CString, Error> {
    CString::new(value).map_err(|_| Error::InvalidEncoding(context))
}

fn cstring_from_buf(buffer: &[i8]) -> String {
    let ptr = buffer.as_ptr();
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

fn owned_c_string(ptr: *mut i8) -> Option<String> {
    if ptr.is_null() {
        return None;
    }

    let value = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    unsafe { crate::nfc_rs_free(ptr.cast()) };
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeDevice {
        name: String,
        connstring: ConnectionString,
    }

    impl OpenedDevice for FakeDevice {
        fn name(&self) -> &str {
            &self.name
        }

        fn connstring(&self) -> &ConnectionString {
            &self.connstring
        }
    }

    struct FakeDriver {
        name: &'static str,
        devices: Vec<ConnectionString>,
    }

    impl Driver for FakeDriver {
        fn name(&self) -> &str {
            self.name
        }

        fn scan_type(&self) -> ScanType {
            ScanType::NotIntrusive
        }

        fn scan(&self, _context: &Context) -> Result<Vec<ConnectionString>, Error> {
            Ok(self.devices.clone())
        }

        fn open(
            &self,
            _context: &Context,
            connstring: &ConnectionString,
        ) -> Result<Box<dyn OpenedDevice>, Error> {
            Ok(Box::new(FakeDevice {
                name: self.name.to_string(),
                connstring: connstring.clone(),
            }))
        }
    }

    #[test]
    fn build_and_parse_connstring_roundtrip() {
        let connstring = build_connstring("pn532_uart", "port", "/dev/ttyUSB0").unwrap();
        assert_eq!(
            parse_connstring(connstring.as_str(), "pn532_uart", "port").unwrap(),
            "/dev/ttyUSB0"
        );
    }

    #[test]
    fn registry_preserves_driver_order_when_listing() {
        let mut registry = DriverRegistry::new();
        registry.register_driver(Box::new(FakeDriver {
            name: "alpha",
            devices: vec![ConnectionString::new("alpha:first").unwrap()],
        }));
        registry.register_driver(Box::new(FakeDriver {
            name: "beta_usb",
            devices: vec![ConnectionString::new("beta_usb:second").unwrap()],
        }));

        let devices = registry.list_devices(&Context::new()).unwrap();
        assert_eq!(
            devices,
            vec![
                ConnectionString::new("alpha:first").unwrap(),
                ConnectionString::new("beta_usb:second").unwrap()
            ]
        );
    }

    #[test]
    fn registry_open_without_connstring_uses_first_discovered_device() {
        let mut registry = DriverRegistry::new();
        registry.register_driver(Box::new(FakeDriver {
            name: "alpha",
            devices: vec![ConnectionString::new("alpha:first").unwrap()],
        }));

        let device = registry.open(&Context::new(), None).unwrap();
        assert_eq!(device.name, "alpha");
        assert_eq!(device.connstring.as_str(), "alpha:first");
    }

    #[cfg(feature = "secure")]
    #[test]
    fn secure_zero_clears_bytes() {
        let mut secret = [0xAAu8; 8];
        secure_zero(&mut secret).unwrap();
        assert_eq!(secret, [0; 8]);
    }
}
