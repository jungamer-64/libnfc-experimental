use std::path::Path;

use proximate_driver as rt;

pub type DeviceSelector = proximate_types::ConnectionString;
pub type ConfiguredDevice = rt::UserDefinedDevice;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config(rt::ContextConfig);

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load() -> Self {
        Self(rt::Context::load().config)
    }

    pub fn load_from_dir(path: &Path) -> Self {
        Self(rt::Context::load_from_dir(path).config)
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

    pub fn user_defined_devices_mut(&mut self) -> &mut Vec<ConfiguredDevice> {
        &mut self.0.user_defined_devices
    }

    pub fn set_allow_autoscan(&mut self, value: bool) {
        self.0.allow_autoscan = value;
    }

    pub fn set_allow_intrusive_scan(&mut self, value: bool) {
        self.0.allow_intrusive_scan = value;
    }

    pub fn set_log_level(&mut self, value: u32) {
        self.0.log_level = value;
    }

    pub fn into_inner(self) -> rt::ContextConfig {
        self.0
    }
}

impl Default for Config {
    fn default() -> Self {
        Self(rt::ContextConfig::default())
    }
}

impl From<rt::ContextConfig> for Config {
    fn from(value: rt::ContextConfig) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceDescriptor {
    pub name: Option<String>,
    pub selector: DeviceSelector,
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

    pub fn register_driver(mut self, driver: Box<dyn rt::Driver>) -> Self {
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

    pub fn load() -> Self {
        Self::builder().with_config(Config::load()).build()
    }

    pub fn load_from_dir(path: &Path) -> Self {
        Self::builder()
            .with_config(Config::load_from_dir(path))
            .build()
    }

    pub fn config(&self) -> Config {
        Config(self.runtime.config.clone())
    }

    pub fn scan(&self) -> Result<Vec<DeviceDescriptor>, proximate_types::Error> {
        let devices = self.registry.list_devices(&self.runtime)?;
        Ok(devices
            .into_iter()
            .map(|selector| DeviceDescriptor {
                name: configured_name(
                    self.runtime.config.user_defined_devices.as_slice(),
                    &selector,
                )
                .map(str::to_owned),
                selector,
            })
            .collect())
    }

    pub fn open(&self, selector: &DeviceSelector) -> Result<DeviceHandle, proximate_types::Error> {
        self.registry
            .open(&self.runtime, Some(selector))
            .map(DeviceHandle::new)
    }

    pub fn open_default(&self) -> Result<DeviceHandle, proximate_types::Error> {
        self.registry
            .open(&self.runtime, None)
            .map(DeviceHandle::new)
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::builder().build()
    }
}

fn configured_name<'a>(
    devices: &'a [ConfiguredDevice],
    selector: &DeviceSelector,
) -> Option<&'a str> {
    devices
        .iter()
        .find(|device| device.connstring == *selector)
        .map(|device| device.name.as_str())
}

pub struct DeviceHandle {
    inner: rt::Device,
}

impl DeviceHandle {
    fn new(inner: rt::Device) -> Self {
        Self { inner }
    }

    pub fn name(&self) -> &str {
        self.inner.name()
    }

    pub fn selector(&self) -> &DeviceSelector {
        self.inner.connstring()
    }

    pub fn caps(&self) -> proximate_types::DeviceCaps {
        self.inner.caps()
    }

    pub fn last_error(&self) -> i32 {
        self.inner.last_error()
    }

    pub fn strerror(&self) -> String {
        self.inner.strerror()
    }

    pub fn information_about(&mut self) -> Result<String, proximate_types::Error> {
        self.inner.information_about()
    }

    pub fn set_property_bool(
        &mut self,
        property: proximate_types::Property,
        enable: bool,
    ) -> Result<(), proximate_types::Error> {
        self.inner.set_property_bool(property, enable)
    }

    pub fn set_property_int(
        &mut self,
        property: proximate_types::Property,
        value: i32,
    ) -> Result<(), proximate_types::Error> {
        self.inner.set_property_int(property, value)
    }

    pub fn initiator(&mut self) -> InitiatorSession<'_> {
        InitiatorSession {
            device: &mut self.inner,
        }
    }

    pub fn target(&mut self) -> TargetSession<'_> {
        TargetSession {
            device: &mut self.inner,
        }
    }

    pub fn pn53x(&mut self) -> Pn53xControl<'_> {
        Pn53xControl {
            device: &mut self.inner,
        }
    }
}

pub struct InitiatorSession<'a> {
    device: &'a mut rt::Device,
}

impl<'a> InitiatorSession<'a> {
    pub fn init(&mut self) -> Result<i32, proximate_types::Error> {
        self.device.initiator_init()
    }

    pub fn init_secure_element(&mut self) -> Result<i32, proximate_types::Error> {
        self.device.initiator_init_secure_element()
    }

    pub fn select_passive_target(
        &mut self,
        modulation: proximate_types::Modulation,
        init_data: Option<&[u8]>,
    ) -> Result<Option<proximate_types::Target>, proximate_types::Error> {
        self.device.select_passive_target(modulation, init_data)
    }

    pub fn list_passive_targets(
        &mut self,
        modulation: proximate_types::Modulation,
        max_targets: usize,
    ) -> Result<Vec<proximate_types::Target>, proximate_types::Error> {
        self.device.list_passive_targets(modulation, max_targets)
    }

    pub fn poll_target(
        &mut self,
        modulations: &[proximate_types::Modulation],
        poll_nr: u8,
        period: u8,
    ) -> Result<Option<proximate_types::Target>, proximate_types::Error> {
        self.device.poll_target(modulations, poll_nr, period)
    }

    pub fn select_dep_target(
        &mut self,
        mode: proximate_types::DepMode,
        baud_rate: proximate_types::BaudRate,
        initiator: Option<&proximate_types::DepInfo>,
        timeout: i32,
    ) -> Result<Option<proximate_types::Target>, proximate_types::Error> {
        self.device
            .select_dep_target(mode, baud_rate, initiator, timeout)
    }

    pub fn poll_dep_target(
        &mut self,
        mode: proximate_types::DepMode,
        baud_rate: proximate_types::BaudRate,
        initiator: Option<&proximate_types::DepInfo>,
        timeout: i32,
    ) -> Result<Option<proximate_types::Target>, proximate_types::Error> {
        self.device
            .poll_dep_target(mode, baud_rate, initiator, timeout)
    }

    pub fn deselect_target(&mut self) -> Result<(), proximate_types::Error> {
        self.device.deselect_target()
    }

    pub fn target_is_present(
        &mut self,
        target: Option<&proximate_types::Target>,
    ) -> Result<bool, proximate_types::Error> {
        self.device.target_is_present(target)
    }

    pub fn transceive_bytes(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, proximate_types::Error> {
        self.device.transceive_bytes(tx, rx, timeout)
    }

    pub fn transceive_bits(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, proximate_types::Error> {
        self.device
            .transceive_bits(tx, tx_bits_len, tx_parity, rx, rx_parity)
    }

    pub fn transceive_bytes_timed(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
    ) -> Result<(usize, u32), proximate_types::Error> {
        self.device.transceive_bytes_timed(tx, rx)
    }

    pub fn transceive_bits_timed(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), proximate_types::Error> {
        self.device
            .transceive_bits_timed(tx, tx_bits_len, tx_parity, rx, rx_parity)
    }
}

pub struct TargetSession<'a> {
    device: &'a mut rt::Device,
}

impl<'a> TargetSession<'a> {
    pub fn init(
        &mut self,
        target: &mut proximate_types::Target,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, proximate_types::Error> {
        self.device.target_init(target, rx, timeout)
    }

    pub fn send_bytes(&mut self, tx: &[u8], timeout: i32) -> Result<usize, proximate_types::Error> {
        self.device.target_send_bytes(tx, timeout)
    }

    pub fn receive_bytes(
        &mut self,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, proximate_types::Error> {
        self.device.target_receive_bytes(rx, timeout)
    }

    pub fn send_bits(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
    ) -> Result<usize, proximate_types::Error> {
        self.device.target_send_bits(tx, tx_bits_len, tx_parity)
    }

    pub fn receive_bits(
        &mut self,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, proximate_types::Error> {
        self.device.target_receive_bits(rx, rx_parity)
    }
}

pub struct Pn53xControl<'a> {
    device: &'a mut rt::Device,
}

impl<'a> Pn53xControl<'a> {
    pub fn transceive(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, proximate_types::Error> {
        self.device
            .handle_mut()
            .pn53x_transceive_driver(tx, rx, timeout)
    }

    pub fn read_register(&mut self, register: u16) -> Result<u8, proximate_types::Error> {
        self.device
            .handle_mut()
            .pn53x_read_register_driver(register)
    }

    pub fn write_register(
        &mut self,
        register: u16,
        symbol_mask: u8,
        value: u8,
    ) -> Result<(), proximate_types::Error> {
        self.device
            .handle_mut()
            .pn53x_write_register_driver(register, symbol_mask, value)
    }

    pub fn sam_configuration(
        &mut self,
        mode: u8,
        timeout: i32,
    ) -> Result<i32, proximate_types::Error> {
        self.device
            .handle_mut()
            .pn532_sam_configuration_driver(mode, timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct FakeDevice {
        name: String,
        connstring: DeviceSelector,
        caps: proximate_types::DeviceCaps,
    }

    impl FakeDevice {
        fn new(connstring: &str, caps: proximate_types::DeviceCaps) -> Self {
            Self {
                name: "fake".into(),
                connstring: DeviceSelector::new(connstring).unwrap(),
                caps,
            }
        }
    }

    impl rt::OpenedDevice for FakeDevice {
        fn name(&self) -> &str {
            &self.name
        }

        fn connstring(&self) -> &DeviceSelector {
            &self.connstring
        }

        fn caps(&self) -> proximate_types::DeviceCaps {
            self.caps
        }

        fn set_property_bool(
            &mut self,
            _property: proximate_types::Property,
            _enable: bool,
        ) -> Result<(), proximate_types::Error> {
            Ok(())
        }

        fn set_property_int(
            &mut self,
            _property: proximate_types::Property,
            _value: i32,
        ) -> Result<(), proximate_types::Error> {
            Ok(())
        }

        fn supported_modulations(
            &mut self,
            _mode: proximate_types::Mode,
        ) -> Result<Vec<proximate_types::ModulationType>, proximate_types::Error> {
            Ok(vec![proximate_types::ModulationType::Iso14443A])
        }

        fn supported_baud_rates(
            &mut self,
            _mode: proximate_types::Mode,
            _modulation_type: proximate_types::ModulationType,
        ) -> Result<Vec<proximate_types::BaudRate>, proximate_types::Error> {
            Ok(vec![proximate_types::BaudRate::Br106])
        }
    }

    struct FakeDriver {
        name: String,
        scan_results: Vec<DeviceSelector>,
        caps: proximate_types::DeviceCaps,
    }

    impl rt::Driver for FakeDriver {
        fn name(&self) -> &str {
            &self.name
        }

        fn scan_type(&self) -> proximate_types::ScanType {
            proximate_types::ScanType::NotIntrusive
        }

        fn scan(
            &self,
            _context: &rt::Context,
        ) -> Result<Vec<DeviceSelector>, proximate_types::Error> {
            Ok(self.scan_results.clone())
        }

        fn open(
            &self,
            _context: &rt::Context,
            connstring: &DeviceSelector,
        ) -> Result<Box<dyn rt::OpenedDevice>, proximate_types::Error> {
            Ok(Box::new(FakeDevice::new(connstring.as_str(), self.caps)))
        }
    }

    fn temp_config_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("proximate-facade-{nonce}-{}", std::process::id()))
    }

    #[test]
    fn config_load_from_dir_surfaces_user_defined_devices() {
        let root = temp_config_root();
        fs::create_dir_all(root.join("devices.d")).unwrap();
        fs::write(
            root.join("libnfc.conf"),
            "device.name = \"config device\"\ndevice.connstring = pn532_spi:/dev/spidev0.0\n",
        )
        .unwrap();
        fs::write(
            root.join("devices.d/extra.conf"),
            "name = \"extra device\"\nconnstring = pn532_i2c:/dev/i2c-1\n",
        )
        .unwrap();

        let config = Config::load_from_dir(&root);
        assert_eq!(config.user_defined_devices().len(), 2);
        assert_eq!(config.user_defined_devices()[0].name, "config device");
        assert_eq!(
            config.user_defined_devices()[1].connstring.as_str(),
            "pn532_i2c:/dev/i2c-1"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn context_scan_returns_named_descriptors() {
        let mut config = Config::default();
        config.set_allow_autoscan(false);
        config.user_defined_devices_mut().push(ConfiguredDevice {
            name: "named device".into(),
            connstring: DeviceSelector::new("pn532_uart:/dev/ttyUSB0").unwrap(),
            optional: false,
        });

        let context = Context::builder()
            .with_config(config)
            .without_builtin_drivers()
            .build();

        let devices = context.scan().unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name.as_deref(), Some("named device"));
        assert_eq!(devices[0].selector.as_str(), "pn532_uart:/dev/ttyUSB0");
    }

    #[test]
    fn open_default_uses_registered_driver_and_preserves_selector() {
        let mut config = Config::default();
        config.set_allow_autoscan(false);
        config.user_defined_devices_mut().push(ConfiguredDevice {
            name: "default device".into(),
            connstring: DeviceSelector::new("fake:device").unwrap(),
            optional: false,
        });

        let context = Context::builder()
            .with_config(config)
            .without_builtin_drivers()
            .register_driver(Box::new(FakeDriver {
                name: "fake".into(),
                scan_results: Vec::new(),
                caps: proximate_types::DeviceCaps::SET_PROPERTY_BOOL,
            }))
            .build();

        let handle = context.open_default().unwrap();
        assert_eq!(handle.selector().as_str(), "fake:device");
        assert_eq!(handle.name(), "default device");
    }

    #[test]
    fn typed_sessions_surface_capability_failures() {
        let context = Context::builder()
            .without_builtin_drivers()
            .register_driver(Box::new(FakeDriver {
                name: "fake".into(),
                scan_results: vec![DeviceSelector::new("fake:device").unwrap()],
                caps: proximate_types::DeviceCaps::SET_PROPERTY_BOOL,
            }))
            .build();

        let mut handle = context
            .open(&DeviceSelector::new("fake:device").unwrap())
            .unwrap();
        let error = handle
            .initiator()
            .transceive_bytes(&[0x01], &mut [0u8; 8], 25)
            .unwrap_err();
        assert!(matches!(
            error,
            proximate_types::Error::MissingCapability(_)
        ));
    }
}
