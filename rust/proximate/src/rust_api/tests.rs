use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use super::*;

struct FakeDevice {
    name: String,
    connstring: ConnectionString,
    property_calls: Vec<(Property, bool)>,
    property_state: Vec<(Property, bool)>,
    supported_modulations: Vec<ModulationType>,
    supported_baud_rates: Vec<BaudRate>,
    passive_targets: VecDeque<Result<Option<Target>, Error>>,
    deselect_calls: usize,
    select_passive_payloads: Vec<Vec<u8>>,
    dep_results: VecDeque<Result<Option<Target>, Error>>,
    target_init_calls: usize,
}

impl Default for FakeDevice {
    fn default() -> Self {
        Self {
            name: String::new(),
            connstring: ConnectionString::new("test").unwrap(),
            property_calls: Vec::new(),
            property_state: Vec::new(),
            supported_modulations: Vec::new(),
            supported_baud_rates: Vec::new(),
            passive_targets: VecDeque::new(),
            deselect_calls: 0,
            select_passive_payloads: Vec::new(),
            dep_results: VecDeque::new(),
            target_init_calls: 0,
        }
    }
}

impl FakeDevice {
    fn new(connstring: &str) -> Self {
        Self {
            name: "fake".to_string(),
            connstring: ConnectionString::new(connstring).unwrap(),
            supported_modulations: vec![
                ModulationType::Iso14443A,
                ModulationType::Iso14443B,
                ModulationType::Felica,
                ModulationType::Dep,
            ],
            supported_baud_rates: vec![BaudRate::Br106, BaudRate::Br212],
            ..Default::default()
        }
    }
}

impl OpenedDevice for FakeDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn connstring(&self) -> &ConnectionString {
        &self.connstring
    }

    fn set_property_bool(&mut self, property: Property, enable: bool) -> Result<(), Error> {
        self.property_calls.push((property, enable));
        match self
            .property_state
            .iter_mut()
            .find(|entry| entry.0 == property)
        {
            Some(entry) => entry.1 = enable,
            None => self.property_state.push((property, enable)),
        }
        Ok(())
    }

    fn set_property_int(&mut self, _property: Property, _value: i32) -> Result<(), Error> {
        Ok(())
    }

    fn supported_modulations(&mut self, _mode: Mode) -> Result<Vec<ModulationType>, Error> {
        Ok(self.supported_modulations.clone())
    }

    fn supported_baud_rates(
        &mut self,
        _mode: Mode,
        _modulation_type: ModulationType,
    ) -> Result<Vec<BaudRate>, Error> {
        Ok(self.supported_baud_rates.clone())
    }

    fn property_bool_state(&self, property: Property) -> Option<bool> {
        self.property_state
            .iter()
            .find(|entry| entry.0 == property)
            .map(|entry| entry.1)
    }

    fn initiator_init_driver(&mut self) -> Result<i32, Error> {
        Ok(0)
    }

    fn select_passive_target_driver(
        &mut self,
        _nm: Modulation,
        init_data: &[u8],
    ) -> Result<Option<Target>, Error> {
        self.select_passive_payloads.push(init_data.to_vec());
        self.passive_targets.pop_front().unwrap_or(Ok(None))
    }

    fn deselect_target_driver(&mut self) -> Result<(), Error> {
        self.deselect_calls += 1;
        Ok(())
    }

    fn select_dep_target_driver(
        &mut self,
        _ndm: DepMode,
        _nbr: BaudRate,
        _initiator: Option<&DepInfo>,
        _timeout: i32,
    ) -> Result<Option<Target>, Error> {
        self.dep_results.pop_front().unwrap_or(Ok(None))
    }

    fn target_init_driver(
        &mut self,
        _target: &mut Target,
        _rx: &mut [u8],
        _timeout: i32,
    ) -> Result<usize, Error> {
        self.target_init_calls += 1;
        Ok(0)
    }
}

struct FakeDriver {
    name: String,
    scan_type: ScanType,
    scan_results: Vec<ConnectionString>,
    open_result: Result<String, Error>,
}

impl Driver for FakeDriver {
    fn name(&self) -> &str {
        &self.name
    }

    fn scan_type(&self) -> ScanType {
        self.scan_type
    }

    fn scan(&self, _context: &Context) -> Result<Vec<ConnectionString>, Error> {
        Ok(self.scan_results.clone())
    }

    fn open(
        &self,
        _context: &Context,
        connstring: &ConnectionString,
    ) -> Result<Box<dyn OpenedDevice>, Error> {
        match &self.open_result {
            Ok(opened_name) => Ok(Box::new(FakeDevice {
                name: opened_name.clone(),
                connstring: connstring.clone(),
                ..FakeDevice::new(connstring.as_str())
            })),
            Err(error) => Err(error.clone()),
        }
    }
}

#[derive(Default)]
struct BackendDeviceState {
    last_error: i32,
    property_calls: Vec<(Property, bool)>,
}

#[derive(Clone)]
struct BackendDevice {
    name: String,
    connstring: ConnectionString,
    state: Arc<Mutex<BackendDeviceState>>,
    unsupported_error_code: i32,
    custom_strerror: Option<String>,
    information_about_result: Result<String, Error>,
    property_bool_result: Result<(), Error>,
    property_int_result: Result<(), Error>,
    supported_modulations_result: Result<Vec<ModulationType>, Error>,
    supported_baud_rates_result: Result<Vec<BaudRate>, Error>,
    native_payload: Option<String>,
}

impl BackendDevice {
    fn new(connstring: &str) -> Self {
        Self {
            name: "backend-device".into(),
            connstring: ConnectionString::new(connstring).unwrap(),
            state: Arc::new(Mutex::new(BackendDeviceState::default())),
            unsupported_error_code: -3,
            custom_strerror: None,
            information_about_result: Ok("info".into()),
            property_bool_result: Ok(()),
            property_int_result: Ok(()),
            supported_modulations_result: Ok(vec![ModulationType::Iso14443A]),
            supported_baud_rates_result: Ok(vec![BaudRate::Br106]),
            native_payload: None,
        }
    }
}

impl DeviceBackend for BackendDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn connstring(&self) -> &ConnectionString {
        &self.connstring
    }

    fn last_error(&self) -> i32 {
        self.state.lock().unwrap().last_error
    }

    fn set_last_error(&mut self, value: i32) {
        self.state.lock().unwrap().last_error = value;
    }

    fn unsupported_error_code(&self) -> i32 {
        self.unsupported_error_code
    }

    fn strerror_backend(&self) -> Option<String> {
        self.custom_strerror.clone()
    }

    fn information_about_backend(&mut self) -> Result<String, Error> {
        self.information_about_result.clone()
    }

    fn set_property_bool_backend(&mut self, property: Property, enable: bool) -> Result<(), Error> {
        self.state
            .lock()
            .unwrap()
            .property_calls
            .push((property, enable));
        self.property_bool_result.clone()
    }

    fn set_property_int_backend(&mut self, _property: Property, _value: i32) -> Result<(), Error> {
        self.property_int_result.clone()
    }

    fn supported_modulations_backend(&mut self, _mode: Mode) -> Result<Vec<ModulationType>, Error> {
        self.supported_modulations_result.clone()
    }

    fn supported_baud_rates_backend(
        &mut self,
        _mode: Mode,
        _modulation_type: ModulationType,
    ) -> Result<Vec<BaudRate>, Error> {
        self.supported_baud_rates_result.clone()
    }

    fn into_native_payload(self: Box<Self>) -> Option<Box<dyn std::any::Any + Send>> {
        self.native_payload
            .map(|payload| Box::new(payload) as Box<dyn std::any::Any + Send>)
    }
}

#[derive(Default)]
struct BackendDriverState {
    scan_capacities: Vec<usize>,
    open_requests: Vec<String>,
}

#[derive(Clone)]
struct BackendDriver {
    name: String,
    scan_type: ScanType,
    scan_sizes: Arc<Mutex<VecDeque<usize>>>,
    state: Arc<Mutex<BackendDriverState>>,
    opened_device: BackendDevice,
    open_error: Option<Error>,
}

impl BackendDriver {
    fn new(name: &str, scan_type: ScanType, scan_sizes: &[usize], opened_connstring: &str) -> Self {
        Self {
            name: name.into(),
            scan_type,
            scan_sizes: Arc::new(Mutex::new(scan_sizes.iter().copied().collect())),
            state: Arc::new(Mutex::new(BackendDriverState::default())),
            opened_device: BackendDevice::new(opened_connstring),
            open_error: None,
        }
    }
}

impl DriverBackend for BackendDriver {
    fn name(&self) -> &str {
        &self.name
    }

    fn scan_type(&self) -> ScanType {
        self.scan_type
    }

    fn scan_with_capacity(
        &self,
        _context: &Context,
        capacity: usize,
    ) -> Result<Vec<ConnectionString>, Error> {
        self.state.lock().unwrap().scan_capacities.push(capacity);
        let size = self.scan_sizes.lock().unwrap().pop_front().unwrap_or(0);
        (0..size)
            .map(|index| ConnectionString::new(format!("{}:{index:03}", self.name)))
            .collect()
    }

    fn open(
        &self,
        _context: &Context,
        connstring: &ConnectionString,
    ) -> Result<Box<dyn DeviceBackend>, Error> {
        self.state
            .lock()
            .unwrap()
            .open_requests
            .push(connstring.as_str().to_string());
        if let Some(error) = &self.open_error {
            return Err(error.clone());
        }
        let mut opened = self.opened_device.clone();
        opened.connstring = connstring.clone();
        Ok(Box::new(opened))
    }
}

fn modulation(modulation_type: ModulationType, baud_rate: BaudRate) -> Modulation {
    Modulation {
        modulation_type,
        baud_rate,
    }
}

fn dep_target() -> Target {
    Target {
        modulation: modulation(ModulationType::Dep, BaudRate::Br106),
        info: TargetInfo::Dep(DepInfo {
            nfcid3: [0x11; 10],
            did: 0x22,
            bs: 0x33,
            br: 0x44,
            timeout: 0x55,
            pp: 0x66,
            general_bytes: vec![0xaa, 0xbb],
            mode: DepMode::Passive,
        }),
    }
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct ScopedEnv {
    saved: Vec<(String, Option<String>)>,
}

impl ScopedEnv {
    fn new() -> Self {
        Self { saved: Vec::new() }
    }

    fn save(&mut self, key: &str) {
        if self.saved.iter().any(|(saved_key, _)| saved_key == key) {
            return;
        }
        self.saved.push((
            key.to_string(),
            std::env::var_os(key).map(|value| value.to_string_lossy().into_owned()),
        ));
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

struct TempConfigDir {
    root: PathBuf,
}

impl TempConfigDir {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "proximate-conf-{}-{}",
            process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write_file(&self, relative: &str, contents: &str) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }
}

impl Drop for TempConfigDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
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
fn context_sources_apply_precedence_and_cap() {
    let conf_connstring = ConnectionString::new("pn53x_usb:conf").unwrap();
    let default_connstring = ConnectionString::new("pn53x_usb:default").unwrap();
    let env_connstring = ConnectionString::new("pn53x_usb:selected").unwrap();
    let context = Context::from_sources(ContextSources {
        config_file: Some(ContextConfig {
            allow_autoscan: false,
            allow_intrusive_scan: true,
            log_level: 7,
            user_defined_devices: vec![
                UserDefinedDevice {
                    name: "conf".into(),
                    connstring: conf_connstring,
                    optional: false,
                },
                UserDefinedDevice {
                    name: "extra".into(),
                    connstring: ConnectionString::new("pn53x_usb:extra").unwrap(),
                    optional: false,
                },
            ],
        }),
        default_device: Some(UserDefinedDevice {
            name: "default".into(),
            connstring: default_connstring,
            optional: false,
        }),
        selected_device: Some(UserDefinedDevice {
            name: "selected".into(),
            connstring: env_connstring.clone(),
            optional: false,
        }),
        allow_autoscan: Some(true),
        allow_intrusive_scan: Some(false),
        log_level: Some(42),
        max_user_defined_devices: Some(1),
    });

    assert!(context.config.allow_autoscan);
    assert!(!context.config.allow_intrusive_scan);
    assert_eq!(context.config.log_level, 42);
    assert_eq!(context.config.user_defined_devices.len(), 1);
    assert_eq!(
        context.config.user_defined_devices[0].connstring,
        env_connstring
    );
}

#[test]
fn driver_registry_honors_intrusive_scan_and_usb_fallback() {
    let mut registry = DriverRegistry::new();
    registry.register_driver(Box::new(FakeDriver {
        name: "fallback_usb".into(),
        scan_type: ScanType::Intrusive,
        scan_results: vec![ConnectionString::new("usb:001").unwrap()],
        open_result: Ok("usb-fallback".into()),
    }));
    registry.register_driver(Box::new(FakeDriver {
        name: "primary_usb".into(),
        scan_type: ScanType::NotIntrusive,
        scan_results: vec![ConnectionString::new("usb:002").unwrap()],
        open_result: Err(Error::DriverOpenFailed("boom".into())),
    }));

    let context = Context::with_config(ContextConfig {
        allow_autoscan: true,
        allow_intrusive_scan: false,
        log_level: 1,
        user_defined_devices: Vec::new(),
    });

    let listed = registry.list_devices(&context).unwrap();
    assert_eq!(listed, vec![ConnectionString::new("usb:002").unwrap()]);

    let opened = registry
        .open(&context, Some(&ConnectionString::new("usb").unwrap()))
        .unwrap();
    assert_eq!(opened.name(), "usb-fallback");
}

#[test]
fn driver_registry_prefers_user_defined_name_override() {
    let connstring = ConnectionString::new("pn53x_usb:device").unwrap();
    let context = Context::with_config(ContextConfig {
        allow_autoscan: true,
        allow_intrusive_scan: false,
        log_level: 1,
        user_defined_devices: vec![UserDefinedDevice {
            name: "friendly name".into(),
            connstring: connstring.clone(),
            optional: false,
        }],
    });

    let mut registry = DriverRegistry::new();
    registry.register_driver(Box::new(FakeDriver {
        name: "pn53x_usb".into(),
        scan_type: ScanType::NotIntrusive,
        scan_results: vec![connstring.clone()],
        open_result: Ok("raw driver name".into()),
    }));

    let device = registry.open(&context, Some(&connstring)).unwrap();
    assert_eq!(device.name(), "friendly name");
}

#[test]
fn backend_driver_wrapper_grows_scan_capacity_until_result_is_not_saturated() {
    let backend = BackendDriver::new("alpha", ScanType::NotIntrusive, &[4, 8, 3], "alpha:open");
    let state = backend.state.clone();

    let mut registry = DriverRegistry::new();
    registry.register_driver(wrap_driver_backend(Box::new(backend)));

    let context = Context::default();
    let listed = registry.list_devices(&context).unwrap();

    assert_eq!(listed.len(), 3);
    assert_eq!(state.lock().unwrap().scan_capacities, vec![4, 8, 16]);
}

#[test]
fn backend_driver_wrapper_stops_scanning_at_max_capacity() {
    let backend = BackendDriver::new(
        "alpha",
        ScanType::NotIntrusive,
        &[4, 8, 16, 32, 64, 128, 256],
        "alpha:open",
    );
    let state = backend.state.clone();

    let mut registry = DriverRegistry::new();
    registry.register_driver(wrap_driver_backend(Box::new(backend)));

    let context = Context::default();
    let listed = registry.list_devices(&context).unwrap();

    assert_eq!(listed.len(), 256);
    assert_eq!(
        state.lock().unwrap().scan_capacities,
        vec![4, 8, 16, 32, 64, 128, 256]
    );
}

#[test]
fn backend_driver_registry_keeps_usb_fallback_and_name_override_behavior() {
    let mut fallback = BackendDriver::new("fallback_usb", ScanType::Intrusive, &[], "usb:open");
    fallback.opened_device.name = "backend fallback".into();
    let fallback_state = fallback.state.clone();

    let mut primary = BackendDriver::new("primary_usb", ScanType::NotIntrusive, &[], "usb:open");
    primary.open_error = Some(Error::DriverOpenFailed("boom".into()));
    let primary_state = primary.state.clone();

    let mut registry = DriverRegistry::new();
    registry.register_driver(wrap_driver_backend(Box::new(fallback)));
    registry.register_driver(wrap_driver_backend(Box::new(primary)));

    let connstring = ConnectionString::new("usb").unwrap();
    let context = Context::with_config(ContextConfig {
        allow_autoscan: true,
        allow_intrusive_scan: false,
        log_level: 1,
        user_defined_devices: vec![UserDefinedDevice {
            name: "friendly backend".into(),
            connstring: ConnectionString::new("usb").unwrap(),
            optional: false,
        }],
    });

    let device = registry.open(&context, Some(&connstring)).unwrap();
    assert_eq!(device.name(), "friendly backend");
    assert_eq!(
        primary_state.lock().unwrap().open_requests,
        vec!["usb".to_string()]
    );
    assert_eq!(
        fallback_state.lock().unwrap().open_requests,
        vec!["usb".to_string()]
    );
}

#[test]
fn backend_device_wrapper_normalizes_unsupported_operations_and_clears_last_error_on_success() {
    let mut backend = BackendDevice::new("alpha:001");
    backend.information_about_result =
        Err(Error::UnsupportedOperation("device_get_information_about"));
    backend.supported_modulations_result =
        Err(Error::UnsupportedOperation("get_supported_modulation"));
    backend.property_bool_result = Ok(());

    let state = backend.state.clone();
    let mut device = wrap_device_backend(Box::new(backend));

    let error = device.information_about().unwrap_err();
    assert_eq!(
        error,
        Error::DeviceOperationFailed {
            operation: "device_get_information_about",
            code: -3,
        }
    );
    assert_eq!(device.last_error(), -3);

    let supported = device.supported_modulations(Mode::Initiator).unwrap();
    assert!(supported.is_empty());
    assert_eq!(device.last_error(), -3);

    device
        .set_property_bool(Property::ActivateField, true)
        .unwrap();
    assert_eq!(device.last_error(), 0);
    assert_eq!(
        state.lock().unwrap().property_calls,
        vec![(Property::ActivateField, true)]
    );
}

#[test]
fn backend_device_wrapper_uses_custom_strerror_fallback_and_delegates_native_payload() {
    let mut backend = BackendDevice::new("alpha:001");
    backend.custom_strerror = Some("backend strerror".into());
    backend.native_payload = Some("payload".into());
    backend.state.lock().unwrap().last_error = -6;

    let device = wrap_device_backend(Box::new(backend.clone()));
    assert_eq!(device.strerror(), "backend strerror");

    let mut fallback_backend = backend;
    fallback_backend.custom_strerror = None;
    let fallback = wrap_device_backend(Box::new(fallback_backend));
    assert_eq!(fallback.strerror(), "Timeout");

    let payload = wrap_device_backend(Box::new(BackendDevice {
        native_payload: Some("payload".into()),
        ..BackendDevice::new("alpha:001")
    }))
    .into_native_payload()
    .unwrap();
    assert_eq!(
        *payload.downcast::<String>().unwrap(),
        "payload".to_string()
    );
}

#[test]
fn initiator_init_applies_expected_property_sequence() {
    let mut device = FakeDevice::new("pn53x_usb");
    device.initiator_init().unwrap();

    assert_eq!(
        device.property_calls,
        vec![
            (Property::ActivateField, false),
            (Property::ActivateField, true),
            (Property::InfiniteSelect, true),
            (Property::AutoIso14443_4, true),
            (Property::ForceIso14443A, true),
            (Property::ForceSpeed106, true),
            (Property::AcceptInvalidFrames, false),
            (Property::AcceptMultipleFrames, false),
        ]
    );
}

#[test]
fn select_passive_target_uses_default_payload_and_validates_modulation() {
    let mut device = FakeDevice::new("pn53x_usb");
    device.passive_targets.push_back(Ok(None));

    let result = device
        .select_passive_target(modulation(ModulationType::Felica, BaudRate::Br212), None)
        .unwrap();
    assert!(result.is_none());
    assert_eq!(
        device.select_passive_payloads,
        vec![vec![0x00, 0xff, 0xff, 0x01, 0x00]]
    );

    let error = device
        .select_passive_target(modulation(ModulationType::Iso14443A, BaudRate::Br847), None)
        .unwrap_err();
    assert_eq!(error, Error::InvalidArgument("baud rate not supported"));
}

#[test]
fn list_passive_targets_dedupes_and_restores_infinite_select() {
    let mut device = FakeDevice::new("pn53x_usb");
    device.property_state.push((Property::InfiniteSelect, true));
    let target = Target::new(modulation(ModulationType::Iso14443A, BaudRate::Br106));
    device.passive_targets.push_back(Ok(Some(target.clone())));
    device.passive_targets.push_back(Ok(Some(target.clone())));

    let listed = device
        .list_passive_targets(modulation(ModulationType::Iso14443A, BaudRate::Br106), 4)
        .unwrap();

    assert_eq!(listed, vec![target]);
    assert_eq!(device.deselect_calls, 1);
    assert_eq!(
        device.property_bool_state(Property::InfiniteSelect),
        Some(true)
    );
}

#[test]
fn poll_dep_target_retries_timeout_and_restores_infinite_select() {
    let mut device = FakeDevice::new("pn53x_usb");
    device
        .property_state
        .push((Property::InfiniteSelect, false));
    device
        .dep_results
        .push_back(Err(Error::DeviceOperationFailed {
            operation: "select_dep_target",
            code: -6,
        }));
    device
        .dep_results
        .push_back(Err(Error::DeviceOperationFailed {
            operation: "select_dep_target",
            code: -6,
        }));
    device.dep_results.push_back(Ok(Some(dep_target())));

    let target = device
        .poll_dep_target(DepMode::Passive, BaudRate::Br106, None, 1000)
        .unwrap();

    assert_eq!(target, Some(dep_target()));
    assert_eq!(
        device.property_bool_state(Property::InfiniteSelect),
        Some(false)
    );
}

#[test]
fn target_init_applies_target_property_sequence() {
    let mut device = FakeDevice::new("pn53x_usb");
    let mut target = Target::new(modulation(ModulationType::Iso14443A, BaudRate::Br106));
    let mut rx = [0u8; 4];

    device.target_init(&mut target, &mut rx, 250).unwrap();

    assert_eq!(device.target_init_calls, 1);
    assert_eq!(
        device.property_calls,
        vec![
            (Property::AcceptInvalidFrames, false),
            (Property::AcceptMultipleFrames, false),
            (Property::HandleCrc, true),
            (Property::HandleParity, true),
            (Property::AutoIso14443_4, true),
            (Property::EasyFraming, true),
            (Property::ActivateCrypto1, false),
            (Property::ActivateField, false),
        ]
    );
}

#[test]
fn decode_connstring_preserves_segments() {
    let connstring = ConnectionString::new("pn53x_usb:bus:device").unwrap();
    let decoded = decode_connstring(&connstring, "pn53x_usb", "usb").unwrap();

    assert_eq!(decoded.match_depth, 3);
    assert_eq!(decoded.param1.as_deref(), Some("bus"));
    assert_eq!(decoded.param2.as_deref(), Some("device"));
}

#[test]
fn registered_driver_set_preserves_insertion_and_probe_order() {
    let mut registry = RegisteredDriverSet::new();
    registry.register("alpha").unwrap();
    registry.register("beta").unwrap();

    assert_eq!(registry.snapshot(), vec!["alpha", "beta"]);
    assert_eq!(
        registry.snapshot().into_iter().rev().collect::<Vec<_>>(),
        vec!["beta", "alpha"]
    );
}

#[test]
fn registered_driver_set_only_registers_builtins_when_empty() {
    let mut registry = RegisteredDriverSet::new();
    registry
        .register_builtins_if_empty(["alpha", "beta"])
        .unwrap();
    registry.register_builtins_if_empty(["gamma"]).unwrap();

    assert_eq!(registry.snapshot(), vec!["alpha", "beta"]);
}

#[test]
fn list_devices_outcome_warns_only_for_manual_selection_mode_without_devices() {
    let registry = DriverRegistry::new();
    let context = Context::with_config(ContextConfig {
        allow_autoscan: false,
        allow_intrusive_scan: false,
        log_level: 1,
        user_defined_devices: Vec::new(),
    });

    let outcome = registry.list_devices_outcome(&context).unwrap();
    assert!(outcome.warn_manual_selection);
    assert!(outcome.devices.is_empty());

    let with_manual_device = Context::with_config(ContextConfig {
        allow_autoscan: false,
        allow_intrusive_scan: false,
        log_level: 1,
        user_defined_devices: vec![UserDefinedDevice {
            name: "manual".into(),
            connstring: ConnectionString::new("alpha:manual").unwrap(),
            optional: false,
        }],
    });

    assert!(
        !registry
            .list_devices_outcome(&with_manual_device)
            .unwrap()
            .warn_manual_selection
    );
}

#[test]
fn open_without_connstring_uses_first_listed_device() {
    struct ScanCountingDriver {
        name: &'static str,
        scan_results: Vec<ConnectionString>,
        opened_name: &'static str,
        scan_calls: Arc<AtomicUsize>,
    }

    impl Driver for ScanCountingDriver {
        fn name(&self) -> &str {
            self.name
        }

        fn scan_type(&self) -> ScanType {
            ScanType::NotIntrusive
        }

        fn scan(&self, _context: &Context) -> Result<Vec<ConnectionString>, Error> {
            self.scan_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.scan_results.clone())
        }

        fn open(
            &self,
            _context: &Context,
            connstring: &ConnectionString,
        ) -> Result<Box<dyn OpenedDevice>, Error> {
            Ok(Box::new(FakeDevice {
                name: self.opened_name.to_string(),
                connstring: connstring.clone(),
                ..FakeDevice::new(connstring.as_str())
            }))
        }
    }

    let alpha_scans = Arc::new(AtomicUsize::new(0));
    let beta_scans = Arc::new(AtomicUsize::new(0));
    let mut registry = DriverRegistry::new();
    registry.register_driver(Box::new(ScanCountingDriver {
        name: "alpha",
        scan_results: vec![ConnectionString::new("alpha:001").unwrap()],
        opened_name: "alpha-device",
        scan_calls: alpha_scans.clone(),
    }));
    registry.register_driver(Box::new(ScanCountingDriver {
        name: "beta",
        scan_results: vec![ConnectionString::new("beta:001").unwrap()],
        opened_name: "beta-device",
        scan_calls: beta_scans.clone(),
    }));

    let context = Context::with_config(ContextConfig {
        allow_autoscan: true,
        allow_intrusive_scan: false,
        log_level: 1,
        user_defined_devices: Vec::new(),
    });

    let device = registry.open(&context, None).unwrap();
    assert_eq!(
        device.connstring(),
        &ConnectionString::new("beta:001").unwrap()
    );
    assert_eq!(beta_scans.load(Ordering::SeqCst), 1);
    assert_eq!(alpha_scans.load(Ordering::SeqCst), 0);
}

#[test]
fn load_from_dir_loads_config_files_and_devices_d_entries() {
    let _env_guard = env_lock().lock().unwrap();
    let mut env = ScopedEnv::new();
    clear_env(&mut env);

    let confdir = TempConfigDir::new();
    confdir.write_file(
        "libnfc.conf",
        concat!(
            "allow_autoscan = false\n",
            "allow_intrusive_scan = true\n",
            "log_level = 7\n",
            "device.name = \"config device\"\n",
            "device.connstring = pn532_spi:/dev/spidev0.0\n",
            "device.optional = True\n"
        ),
    );
    confdir.write_file(
        "devices.d/extra.conf",
        concat!(
            "name = \"extra device\"\n",
            "connstring = pn532_i2c:/dev/i2c-1\n",
            "optional = 1\n"
        ),
    );

    let outcome = Context::load_from_dir_with_diagnostics(confdir.path()).unwrap();
    let context = outcome.context;

    assert!(!context.config.allow_autoscan);
    assert!(context.config.allow_intrusive_scan);
    assert_eq!(context.config.log_level, 7);
    assert_eq!(context.config.user_defined_devices.len(), 2);
    assert_eq!(context.config.user_defined_devices[0].name, "config device");
    assert_eq!(
        context.config.user_defined_devices[0].connstring.as_str(),
        "pn532_spi:/dev/spidev0.0"
    );
    assert!(context.config.user_defined_devices[0].optional);
    assert_eq!(context.config.user_defined_devices[1].name, "extra device");
    assert_eq!(
        context.config.user_defined_devices[1].connstring.as_str(),
        "pn532_i2c:/dev/i2c-1"
    );
    assert!(context.config.user_defined_devices[1].optional);
    assert!(outcome.diagnostics.iter().any(|entry| {
        entry
            .message
            .contains("key: [allow_autoscan], value: [false]")
    }));
}

#[test]
fn load_from_dir_logs_parse_errors_and_caps_device_count() {
    let _env_guard = env_lock().lock().unwrap();
    let mut env = ScopedEnv::new();
    clear_env(&mut env);

    let confdir = TempConfigDir::new();
    confdir.write_file(
        "libnfc.conf",
        concat!(
            "unknown.key = value\n",
            "broken line\n",
            "device.name = first\n",
            "device.connstring = pn532_uart:/dev/ttyUSB0\n",
            "device.name = second\n",
            "device.connstring = pn53x_usb:001:002\n",
            "device.name = third\n",
            "device.connstring = pn532_spi:/dev/spidev0.0\n",
            "device.name = fourth\n",
            "device.connstring = pn532_i2c:/dev/i2c-1\n",
            "device.name = fifth\n",
            "device.connstring = pn71xx:/dev/nfc0\n"
        ),
    );

    let outcome = Context::load_from_dir_with_diagnostics(confdir.path()).unwrap();
    assert_eq!(outcome.context.config.user_defined_devices.len(), 4);
    assert!(outcome.diagnostics.iter().any(|entry| {
        entry
            .message
            .contains("Unknown key in config line: unknown.key = value")
    }));
    assert!(outcome.diagnostics.iter().any(|entry| {
        entry
            .message
            .contains("Parse error on line #2: broken line")
    }));
    assert!(outcome.diagnostics.iter().any(|entry| {
        entry
            .message
            .contains("Configuration exceeded maximum user-defined devices.")
    }));
}

#[test]
fn load_from_dir_libnfc_device_overrides_config_and_default_device() {
    let _env_guard = env_lock().lock().unwrap();
    let mut env = ScopedEnv::new();
    clear_env(&mut env);
    env.set("LIBNFC_DEFAULT_DEVICE", "pn532_uart:/dev/ttyUSB0");
    env.set("LIBNFC_DEVICE", "pn53x_usb:001:002");

    let confdir = TempConfigDir::new();
    confdir.write_file(
        "libnfc.conf",
        concat!(
            "device.name = \"config device\"\n",
            "device.connstring = pn532_spi:/dev/spidev0.0\n",
            "device.optional = true\n"
        ),
    );

    let context = Context::load_from_dir(confdir.path());
    assert_eq!(context.config.user_defined_devices.len(), 1);
    assert_eq!(
        context.config.user_defined_devices[0].name,
        "user defined device"
    );
    assert_eq!(
        context.config.user_defined_devices[0].connstring.as_str(),
        "pn53x_usb:001:002"
    );
}

#[test]
fn load_from_dir_applies_env_boolean_and_log_level_overrides() {
    let _env_guard = env_lock().lock().unwrap();
    let mut env = ScopedEnv::new();
    clear_env(&mut env);
    env.set("LIBNFC_AUTO_SCAN", "false");
    env.set("LIBNFC_INTRUSIVE_SCAN", "true");
    env.set("LIBNFC_LOG_LEVEL", "42");

    let confdir = TempConfigDir::new();
    confdir.write_file(
        "libnfc.conf",
        concat!(
            "allow_autoscan = true\n",
            "allow_intrusive_scan = false\n",
            "log_level = 7\n"
        ),
    );

    let context = Context::load_from_dir(confdir.path());
    assert!(!context.config.allow_autoscan);
    assert!(context.config.allow_intrusive_scan);
    assert_eq!(context.config.log_level, 42);
}

#[test]
fn load_keeps_lowercase_only_boolean_semantics() {
    let _env_guard = env_lock().lock().unwrap();
    let mut env = ScopedEnv::new();
    clear_env(&mut env);
    env.set("LIBNFC_INTRUSIVE_SCAN", "True");

    let context = Context::load();
    assert!(!context.config.allow_intrusive_scan);
}

#[test]
fn load_with_diagnostics_rejects_oversized_default_device_env() {
    let _env_guard = env_lock().lock().unwrap();
    let mut env = ScopedEnv::new();
    clear_env(&mut env);
    env.set(
        "LIBNFC_DEFAULT_DEVICE",
        &"x".repeat(crate::NFC_BUFSIZE_CONNSTRING),
    );

    let failure = Context::load_with_diagnostics().unwrap_err();
    assert_eq!(
        failure.last_error.as_deref(),
        Some("Failed to copy LIBNFC_DEFAULT_DEVICE environment variable")
    );
    assert_eq!(failure.diagnostics.len(), 1);
    assert_eq!(
        failure.diagnostics[0].category,
        ContextDiagnosticCategory::General
    );
}

#[test]
fn public_version_labels_and_error_messages_are_stable() {
    assert!(!version().is_empty());
    assert_eq!(BaudRate::Br106.label(), "106 kbps");
    assert_eq!(ModulationType::Dep.label(), "D.E.P.");
    assert_eq!(device_error_message(-6), "Timeout");
}
