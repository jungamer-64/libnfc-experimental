use std::path::Path;

use proximate_driver as rt;

pub type ConfiguredDevice = rt::UserDefinedDevice;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceSelector(rt::ConnectionString);

impl DeviceSelector {
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

impl From<rt::ConnectionString> for DeviceSelector {
    fn from(value: rt::ConnectionString) -> Self {
        Self(value)
    }
}

impl From<DeviceSelector> for rt::ConnectionString {
    fn from(value: DeviceSelector) -> Self {
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

    pub fn user_defined_devices(&self) -> &[ConfiguredDevice] {
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

    pub fn with_user_device(mut self, device: ConfiguredDevice) -> Self {
        self.0.user_defined_devices.push(device);
        self
    }

    pub fn push_user_device(&mut self, device: ConfiguredDevice) {
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
    pub selector: DeviceSelector,
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

    pub fn open(&self, selector: &DeviceSelector) -> Result<Device, rt::Error> {
        self.registry
            .open(&self.runtime, Some(selector.as_connection_string()))
            .map(Device::new)
    }

    pub fn open_default(&self) -> Result<Device, rt::Error> {
        self.registry.open(&self.runtime, None).map(Device::new)
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::builder().build()
    }
}

fn configured_name<'a>(
    devices: &'a [ConfiguredDevice],
    selector: &rt::ConnectionString,
) -> Option<&'a str> {
    devices
        .iter()
        .find(|device| device.connstring == *selector)
        .map(|device| device.name.as_str())
}

pub struct Device {
    inner: rt::Device,
}

impl Device {
    fn new(inner: rt::Device) -> Self {
        Self { inner }
    }

    pub fn name(&self) -> &str {
        self.inner.name()
    }

    pub fn selector(&self) -> DeviceSelector {
        self.inner.connstring().clone().into()
    }

    pub fn caps(&self) -> rt::DeviceCaps {
        self.inner.caps()
    }

    pub fn last_error(&self) -> i32 {
        self.inner.last_error()
    }

    pub fn strerror(&self) -> String {
        self.inner.strerror()
    }

    pub fn information_about(&mut self) -> Result<String, rt::Error> {
        self.inner.information_about()
    }

    pub fn set_property_bool(
        &mut self,
        property: rt::Property,
        enable: bool,
    ) -> Result<(), rt::Error> {
        self.inner.set_property_bool(property, enable)
    }

    pub fn set_property_int(
        &mut self,
        property: rt::Property,
        value: i32,
    ) -> Result<(), rt::Error> {
        self.inner.set_property_int(property, value)
    }

    pub fn initiator(&mut self) -> Result<InitiatorDevice<'_>, rt::Error> {
        self.inner
            .initiator()
            .map(|inner| InitiatorDevice { inner })
    }

    pub fn target(&mut self) -> Result<TargetDevice<'_>, rt::Error> {
        self.inner.target().map(|inner| TargetDevice { inner })
    }

    pub fn pn53x(&mut self) -> Result<Pn53xDevice<'_>, rt::Error> {
        self.inner.pn53x().map(|inner| Pn53xDevice { inner })
    }
}

pub struct InitiatorDevice<'a> {
    inner: rt::InitiatorDevice<'a>,
}

impl<'a> InitiatorDevice<'a> {
    pub fn init(&mut self) -> Result<i32, rt::Error> {
        self.inner.init()
    }

    pub fn init_secure_element(&mut self) -> Result<i32, rt::Error> {
        self.inner.init_secure_element()
    }

    pub fn select_passive_target(
        &mut self,
        modulation: rt::Modulation,
        init_data: Option<&[u8]>,
    ) -> Result<Option<rt::Target>, rt::Error> {
        self.inner.select_passive_target(modulation, init_data)
    }

    pub fn list_passive_targets(
        &mut self,
        modulation: rt::Modulation,
        max_targets: usize,
    ) -> Result<Vec<rt::Target>, rt::Error> {
        self.inner.list_passive_targets(modulation, max_targets)
    }

    pub fn poll_target(
        &mut self,
        modulations: &[rt::Modulation],
        poll_nr: u8,
        period: u8,
    ) -> Result<Option<rt::Target>, rt::Error> {
        self.inner.poll_target(modulations, poll_nr, period)
    }

    pub fn select_dep_target(
        &mut self,
        mode: rt::DepMode,
        baud_rate: rt::BaudRate,
        initiator: Option<&rt::DepInfo>,
        timeout: i32,
    ) -> Result<Option<rt::Target>, rt::Error> {
        self.inner
            .select_dep_target(mode, baud_rate, initiator, timeout)
    }

    pub fn poll_dep_target(
        &mut self,
        mode: rt::DepMode,
        baud_rate: rt::BaudRate,
        initiator: Option<&rt::DepInfo>,
        timeout: i32,
    ) -> Result<Option<rt::Target>, rt::Error> {
        self.inner
            .poll_dep_target(mode, baud_rate, initiator, timeout)
    }

    pub fn deselect_target(&mut self) -> Result<(), rt::Error> {
        self.inner.deselect_target()
    }

    pub fn target_is_present(&mut self, target: Option<&rt::Target>) -> Result<bool, rt::Error> {
        self.inner.target_is_present(target)
    }

    pub fn transceive_bytes(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, rt::Error> {
        self.inner.transceive_bytes(tx, rx, timeout)
    }

    pub fn transceive_bits(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, rt::Error> {
        self.inner
            .transceive_bits(tx, tx_bits_len, tx_parity, rx, rx_parity)
    }

    pub fn transceive_bytes_timed(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
    ) -> Result<(usize, u32), rt::Error> {
        self.inner.transceive_bytes_timed(tx, rx)
    }

    pub fn transceive_bits_timed(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), rt::Error> {
        self.inner
            .transceive_bits_timed(tx, tx_bits_len, tx_parity, rx, rx_parity)
    }

    pub fn abort_command(&mut self) -> Result<(), rt::Error> {
        self.inner.abort_command()
    }

    pub fn idle(&mut self) -> Result<(), rt::Error> {
        self.inner.idle()
    }

    pub fn powerdown(&mut self) -> Result<(), rt::Error> {
        self.inner.powerdown()
    }
}

pub struct TargetDevice<'a> {
    inner: rt::TargetDevice<'a>,
}

impl<'a> TargetDevice<'a> {
    pub fn init(
        &mut self,
        target: &mut rt::Target,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, rt::Error> {
        self.inner.init(target, rx, timeout)
    }

    pub fn send_bytes(&mut self, tx: &[u8], timeout: i32) -> Result<usize, rt::Error> {
        self.inner.send_bytes(tx, timeout)
    }

    pub fn receive_bytes(&mut self, rx: &mut [u8], timeout: i32) -> Result<usize, rt::Error> {
        self.inner.receive_bytes(rx, timeout)
    }

    pub fn send_bits(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
    ) -> Result<usize, rt::Error> {
        self.inner.send_bits(tx, tx_bits_len, tx_parity)
    }

    pub fn receive_bits(
        &mut self,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, rt::Error> {
        self.inner.receive_bits(rx, rx_parity)
    }
}

pub struct Pn53xDevice<'a> {
    inner: rt::Pn53xDevice<'a>,
}

impl<'a> Pn53xDevice<'a> {
    pub fn transceive(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, rt::Error> {
        self.inner.transceive(tx, rx, timeout)
    }

    pub fn read_register(&mut self, register: u16) -> Result<u8, rt::Error> {
        self.inner.read_register(register)
    }

    pub fn write_register(
        &mut self,
        register: u16,
        symbol_mask: u8,
        value: u8,
    ) -> Result<(), rt::Error> {
        self.inner.write_register(register, symbol_mask, value)
    }

    pub fn sam_configuration(&mut self, mode: u8, timeout: i32) -> Result<i32, rt::Error> {
        self.inner.sam_configuration(mode, timeout)
    }
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
        ) -> Result<Box<dyn rt::DeviceBackend>, rt::Error> {
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
        config.push_user_device(ConfiguredDevice {
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
        let context = Context::builder()
            .without_builtin_drivers()
            .register_driver(FakeDriver {
                caps: rt::DeviceCaps::SET_PROPERTY_BOOL,
            })
            .build();

        let scanned = context.scan().unwrap();
        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].display_name, "fake:001");
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
        let selector = DeviceSelector::new("fake:001").unwrap();
        let mut device = context.open(&selector).unwrap();

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
