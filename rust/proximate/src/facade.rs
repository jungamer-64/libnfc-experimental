use std::fmt;
use std::path::Path;

use proximate_driver as rt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Selector(rt::ConnectionString);

impl Selector {
    pub fn new(value: impl Into<String>) -> Result<Self, rt::Error> {
        Ok(Self(rt::ConnectionString::new(value)?))
    }

    pub fn as_connection_string(&self) -> &rt::ConnectionString {
        &self.0
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_inner(self) -> rt::ConnectionString {
        self.0
    }
}

impl fmt::Display for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<rt::ConnectionString> for Selector {
    fn as_ref(&self) -> &rt::ConnectionString {
        self.as_connection_string()
    }
}

impl From<rt::ConnectionString> for Selector {
    fn from(value: rt::ConnectionString) -> Self {
        Self(value)
    }
}

impl From<Selector> for rt::ConnectionString {
    fn from(value: Selector) -> Self {
        value.0
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Config(rt::ContextConfig);

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn try_load() -> Result<Self, rt::ContextLoadError> {
        rt::Context::try_load().map(|context| Self(context.config))
    }

    pub fn load_or_default() -> Self {
        Self(rt::Context::load_or_default().config)
    }

    pub fn try_load_from_dir(path: &Path) -> Result<Self, rt::ContextLoadError> {
        rt::Context::try_load_from_dir(path).map(|context| Self(context.config))
    }

    pub fn load_from_dir_or_default(path: &Path) -> Self {
        Self(rt::Context::load_from_dir_or_default(path).config)
    }

    pub fn allow_autoscan(&self) -> bool {
        self.0.allow_autoscan
    }

    pub fn allow_intrusive_scan(&self) -> bool {
        self.0.allow_intrusive_scan
    }

    pub fn log_level(&self) -> u32 {
        self.0.log_level
    }

    pub fn user_defined_devices(&self) -> &[rt::UserDefinedDevice] {
        &self.0.user_defined_devices
    }

    pub fn with_allow_autoscan(mut self, value: bool) -> Self {
        self.0.allow_autoscan = value;
        self
    }

    pub fn with_allow_intrusive_scan(mut self, value: bool) -> Self {
        self.0.allow_intrusive_scan = value;
        self
    }

    pub fn with_log_level(mut self, value: u32) -> Self {
        self.0.log_level = value;
        self
    }

    pub fn with_user_device(mut self, device: rt::UserDefinedDevice) -> Self {
        self.0.user_defined_devices.push(device);
        self
    }

    pub fn push_user_device(&mut self, device: rt::UserDefinedDevice) {
        self.0.user_defined_devices.push(device);
    }

    pub fn clear_user_devices(&mut self) {
        self.0.user_defined_devices.clear();
    }

    pub fn into_inner(self) -> rt::ContextConfig {
        self.0
    }
}

impl From<rt::ContextConfig> for Config {
    fn from(value: rt::ContextConfig) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceInfo {
    pub display_name: String,
    pub selector: Selector,
    pub caps_hint: Option<rt::DeviceCaps>,
}

pub struct ContextBuilder {
    config: Config,
    registry: rt::DriverRegistry,
    builtin_drivers: bool,
}

impl ContextBuilder {
    pub fn new() -> Self {
        Self {
            config: Config::default(),
            registry: rt::DriverRegistry::new(),
            builtin_drivers: true,
        }
    }

    pub fn with_config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }

    pub fn without_builtin_drivers(mut self) -> Self {
        self.builtin_drivers = false;
        self
    }

    pub fn register_driver(mut self, driver: impl rt::Driver + 'static) -> Self {
        self.registry.register_driver(Box::new(driver));
        self
    }

    pub fn register_boxed_driver(mut self, driver: Box<dyn rt::Driver>) -> Self {
        self.registry.register_driver(driver);
        self
    }

    pub fn build(mut self) -> Context {
        if self.builtin_drivers {
            proximate_native::register_builtin_drivers(&mut self.registry);
        }

        Context {
            runtime: rt::Context::with_config(self.config.into_inner()),
            registry: self.registry,
        }
    }
}

impl Default for ContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Context {
    runtime: rt::Context,
    registry: rt::DriverRegistry,
}

impl Context {
    pub fn builder() -> ContextBuilder {
        ContextBuilder::new()
    }

    pub fn try_load() -> Result<Self, rt::ContextLoadError> {
        Ok(Self::builder().with_config(Config::try_load()?).build())
    }

    pub fn load_or_default() -> Self {
        Self::builder()
            .with_config(Config::load_or_default())
            .build()
    }

    pub fn try_load_from_dir(path: &Path) -> Result<Self, rt::ContextLoadError> {
        Ok(Self::builder()
            .with_config(Config::try_load_from_dir(path)?)
            .build())
    }

    pub fn load_from_dir_or_default(path: &Path) -> Self {
        Self::builder()
            .with_config(Config::load_from_dir_or_default(path))
            .build()
    }

    pub fn config(&self) -> Config {
        Config(self.runtime.config.clone())
    }

    pub fn scan(&self) -> Result<Vec<DeviceInfo>, rt::Error> {
        let devices = self.registry.list_devices(&self.runtime)?;
        Ok(devices
            .into_iter()
            .map(|selector| {
                let display_name = configured_name(
                    self.runtime.config.user_defined_devices.as_slice(),
                    &selector,
                )
                .map(str::to_owned)
                .unwrap_or_else(|| selector.as_str().to_owned());
                DeviceInfo {
                    display_name,
                    selector: selector.into(),
                    caps_hint: None,
                }
            })
            .collect())
    }

    pub fn open(&self, selector: &Selector) -> Result<rt::Device, rt::Error> {
        self.registry
            .open(&self.runtime, Some(selector.as_connection_string()))
    }

    pub fn open_default(&self) -> Result<rt::Device, rt::Error> {
        self.registry.open(&self.runtime, None)
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::builder().build()
    }
}

fn configured_name<'a>(
    devices: &'a [rt::UserDefinedDevice],
    selector: &rt::ConnectionString,
) -> Option<&'a str> {
    devices
        .iter()
        .find(|device| device.connstring == *selector)
        .map(|device| device.name.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::sync::{Mutex, OnceLock};

    struct FakeDevice {
        name: String,
        connstring: rt::ConnectionString,
        caps: rt::DeviceCaps,
    }

    impl FakeDevice {
        fn new(connstring: &str, caps: rt::DeviceCaps) -> Self {
            Self {
                name: "fake".into(),
                connstring: rt::ConnectionString::new(connstring).unwrap(),
                caps,
            }
        }
    }

    impl rt::DeviceMeta for FakeDevice {
        fn name(&self) -> &str {
            &self.name
        }

        fn connstring(&self) -> &rt::ConnectionString {
            &self.connstring
        }

        fn caps(&self) -> rt::DeviceCaps {
            self.caps
        }
    }

    impl rt::InfoBackend for FakeDevice {}

    impl rt::PropertyBackend for FakeDevice {
        fn set_property_bool(
            &mut self,
            _property: rt::Property,
            _enable: bool,
        ) -> Result<(), rt::Error> {
            Ok(())
        }

        fn set_property_int(
            &mut self,
            _property: rt::Property,
            _value: i32,
        ) -> Result<(), rt::Error> {
            Ok(())
        }

        fn supported_modulations(
            &mut self,
            _mode: rt::Mode,
        ) -> Result<Vec<rt::ModulationType>, rt::Error> {
            Ok(vec![rt::ModulationType::Iso14443A])
        }

        fn supported_baud_rates(
            &mut self,
            _mode: rt::Mode,
            _modulation_type: rt::ModulationType,
        ) -> Result<Vec<rt::BaudRate>, rt::Error> {
            Ok(vec![rt::BaudRate::Br106])
        }
    }

    impl rt::InitiatorBackend for FakeDevice {}

    impl rt::TargetBackend for FakeDevice {}

    impl rt::Pn53xBackend for FakeDevice {}

    struct FakeDriver {
        caps: rt::DeviceCaps,
    }

    impl rt::Driver for FakeDriver {
        fn name(&self) -> &str {
            "fake"
        }

        fn scan_type(&self) -> rt::ScanType {
            rt::ScanType::NotIntrusive
        }

        fn scan(&self, _context: &rt::Context) -> Result<Vec<rt::ConnectionString>, rt::Error> {
            Ok(vec![rt::ConnectionString::new("fake:001").unwrap()])
        }

        fn open(
            &self,
            _context: &rt::Context,
            connstring: &rt::ConnectionString,
        ) -> Result<Box<dyn rt::DeviceHandle>, rt::Error> {
            Ok(Box::new(FakeDevice::new(connstring.as_str(), self.caps)))
        }
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct ScopedEnv {
        saved: Vec<(String, Option<OsString>)>,
    }

    impl ScopedEnv {
        fn new() -> Self {
            Self { saved: Vec::new() }
        }

        fn save(&mut self, key: &str) {
            if self.saved.iter().any(|(saved_key, _)| saved_key == key) {
                return;
            }
            self.saved.push((key.to_string(), std::env::var_os(key)));
        }

        fn set(&mut self, key: &str, value: &str) {
            self.save(key);
            unsafe { std::env::set_var(key, value) };
        }

        fn remove(&mut self, key: &str) {
            self.save(key);
            unsafe { std::env::remove_var(key) };
        }
    }

    impl Drop for ScopedEnv {
        fn drop(&mut self) {
            for (key, value) in self.saved.drain(..).rev() {
                match value {
                    Some(value) => unsafe { std::env::set_var(&key, value) },
                    None => unsafe { std::env::remove_var(&key) },
                }
            }
        }
    }

    fn clear_env(env: &mut ScopedEnv) {
        for key in [
            "LIBNFC_DEFAULT_DEVICE",
            "LIBNFC_DEVICE",
            "LIBNFC_AUTO_SCAN",
            "LIBNFC_INTRUSIVE_SCAN",
            "LIBNFC_LOG_LEVEL",
        ] {
            env.remove(key);
        }
    }

    #[test]
    fn config_builder_updates_user_devices() {
        let mut config = Config::new().with_allow_autoscan(false).with_log_level(4);
        config.push_user_device(rt::UserDefinedDevice {
            name: "named".into(),
            connstring: rt::ConnectionString::new("fake:001").unwrap(),
            optional: false,
        });

        assert!(!config.allow_autoscan());
        assert_eq!(config.log_level(), 4);
        assert_eq!(config.user_defined_devices().len(), 1);

        config.clear_user_devices();
        assert!(config.user_defined_devices().is_empty());
    }

    #[test]
    fn context_scan_returns_device_info() {
        let config =
            Config::new()
                .with_allow_autoscan(false)
                .with_user_device(rt::UserDefinedDevice {
                    name: "named".into(),
                    connstring: rt::ConnectionString::new("fake:001").unwrap(),
                    optional: false,
                });
        let context = Context::builder()
            .with_config(config)
            .without_builtin_drivers()
            .build();

        let scanned = context.scan().unwrap();
        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].display_name, "named");
        assert_eq!(scanned[0].selector.as_str(), "fake:001");
        assert_eq!(scanned[0].caps_hint, None);
    }

    #[test]
    fn device_views_fail_when_capability_is_missing() {
        let context = Context::builder()
            .without_builtin_drivers()
            .register_driver(FakeDriver {
                caps: rt::DeviceCaps::SET_PROPERTY_BOOL,
            })
            .build();
        let selector = Selector::new("fake:001").unwrap();
        let mut device: rt::Device = context.open(&selector).unwrap();

        assert!(matches!(
            device.initiator(),
            Err(rt::Error::MissingCapability(_))
        ));
        assert!(matches!(
            device.target(),
            Err(rt::Error::MissingCapability(_))
        ));
        assert!(matches!(
            device.pn53x(),
            Err(rt::Error::MissingCapability(_))
        ));
    }

    #[test]
    fn open_default_returns_runtime_device() {
        let context = Context::builder()
            .without_builtin_drivers()
            .register_driver(FakeDriver {
                caps: rt::DeviceCaps::SET_PROPERTY_BOOL,
            })
            .build();

        let device: rt::Device = context.open_default().unwrap();
        assert_eq!(device.name(), "fake");
        assert_eq!(device.connstring().as_str(), "fake:001");
    }

    #[test]
    fn try_load_surfaces_context_load_error() {
        let _env_guard = env_lock().lock().unwrap();
        let mut env = ScopedEnv::new();
        clear_env(&mut env);
        env.set(
            "LIBNFC_DEFAULT_DEVICE",
            &"x".repeat(rt::NFC_BUFSIZE_CONNSTRING),
        );

        let error = Config::try_load().unwrap_err();
        assert_eq!(
            error.message(),
            "Failed to copy LIBNFC_DEFAULT_DEVICE environment variable"
        );
    }

    #[test]
    fn load_or_default_swallows_context_load_error() {
        let _env_guard = env_lock().lock().unwrap();
        let mut env = ScopedEnv::new();
        clear_env(&mut env);
        env.set(
            "LIBNFC_DEFAULT_DEVICE",
            &"x".repeat(rt::NFC_BUFSIZE_CONNSTRING),
        );

        let config = Config::load_or_default();
        assert!(config.allow_autoscan());
        assert!(!config.allow_intrusive_scan());
    }
}
