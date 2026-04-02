#[cfg(feature = "nci_helper")]
use crate::nci::TagInfo;
#[cfg(all(feature = "nci_helper", not(test)))]
use crate::nci::{self as platform_nci, Backend as _};
use proximate_driver::{
    BaudRate, ConnectionString, Context, DeviceCaps, DeviceHandle, DeviceMeta, Driver, Error,
    InfoBackend, InitiatorBackend, Mode, Modulation, ModulationType, Pn53xBackend, Property,
    PropertyBackend, ScanType, Target, TargetBackend, TargetInfo,
};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

#[cfg(not(feature = "nci_helper"))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TagInfo {
    technology: u32,
    handle: u32,
    uid: [u8; 32],
    uid_length: u32,
    protocol: u8,
}

const NFC_SUCCESS: i32 = 0;
const NFC_EIO: i32 = -1;
const NFC_EINVARG: i32 = -2;
const PN71XX_DRIVER_NAME: &str = "pn71xx";
const PN71XX_DEVICE_NAME: &str = "pn71xx-device";
const PN71XX_INFO: &str = "PN71XX nfc driver using libnfc-nci userspace library";
const DESFIRE_ATS: [u8; 4] = [0x75, 0x77, 0x81, 0x02];
const DEFAULT_NFA_TECH_MASK: i32 = 0x07;
#[cfg(test)]
const NFC_SETTLE_DELAY: Duration = Duration::ZERO;
#[cfg(not(test))]
const NFC_SETTLE_DELAY: Duration = Duration::from_secs(1);
const POLL_PERIOD_FACTOR_MICROS: u64 = 150_000;

const TARGET_TYPE_ISO14443_3A: u32 = 0x01;
const TARGET_TYPE_ISO14443_3B: u32 = 0x02;
const TARGET_TYPE_FELICA: u32 = 0x03;
const TARGET_TYPE_MIFARE_CLASSIC: u32 = 0x08;
const TARGET_TYPE_MIFARE_UL: u32 = 0x09;
const TARGET_TYPE_ISO14443_4: u32 = 0x20;

const NFA_PROTOCOL_T1T: u8 = 0x01;

const SUPPORTED_MODULATIONS: &[ModulationType] = &[
    ModulationType::Iso14443A,
    ModulationType::Felica,
    ModulationType::Iso14443B,
    ModulationType::Iso14443Bi,
    ModulationType::Iso14443B2Sr,
    ModulationType::Iso14443B2Ct,
    ModulationType::Jewel,
    ModulationType::Dep,
];

const ISO14443A_SUPPORTED_BAUD_RATES: &[BaudRate] = &[
    BaudRate::Br847,
    BaudRate::Br424,
    BaudRate::Br212,
    BaudRate::Br106,
];
const FELICA_SUPPORTED_BAUD_RATES: &[BaudRate] = &[BaudRate::Br424, BaudRate::Br212];
const DEP_SUPPORTED_BAUD_RATES: &[BaudRate] = &[BaudRate::Br424, BaudRate::Br212, BaudRate::Br106];
const JEWEL_SUPPORTED_BAUD_RATES: &[BaudRate] = &[
    BaudRate::Br847,
    BaudRate::Br424,
    BaudRate::Br212,
    BaudRate::Br106,
];
const ISO14443B_SUPPORTED_BAUD_RATES: &[BaudRate] = &[
    BaudRate::Br847,
    BaudRate::Br424,
    BaudRate::Br212,
    BaudRate::Br106,
];

fn device_error(operation: &'static str, code: i32) -> Error {
    Error::DeviceOperationFailed { operation, code }
}

#[derive(Clone, Debug, Default)]
struct Pn71xxRuntime {
    initialized: bool,
    callbacks_registered: bool,
    discovery_enabled: bool,
    active_device: Option<u64>,
    next_device_id: u64,
}

trait NciBackend: Send + Sync {
    fn initialize(&self) -> i32;
    fn deinitialize(&self);
    fn register_callbacks(&self);
    fn deregister_callbacks(&self);
    fn enable_discovery(
        &self,
        technologies_mask: i32,
        reader_mode: i32,
        enable_host_routing: i32,
        restart: i32,
    );
    fn disable_discovery(&self);
    fn transceive(&self, handle: u32, tx: &[u8], rx: &mut [u8], timeout: i32) -> i32;
    fn current_tag_snapshot(&self) -> Option<TagInfo>;
}

#[cfg(all(feature = "nci_helper", not(test)))]
struct SystemNciBackend;

#[cfg(all(feature = "nci_helper", not(test)))]
impl NciBackend for SystemNciBackend {
    fn initialize(&self) -> i32 {
        platform_nci::SystemBackend.initialize()
    }

    fn deinitialize(&self) {
        platform_nci::SystemBackend.deinitialize();
    }

    fn register_callbacks(&self) {
        platform_nci::SystemBackend.register_callbacks();
    }

    fn deregister_callbacks(&self) {
        platform_nci::SystemBackend.deregister_callbacks();
    }

    fn enable_discovery(
        &self,
        technologies_mask: i32,
        reader_mode: i32,
        enable_host_routing: i32,
        restart: i32,
    ) {
        platform_nci::SystemBackend.enable_discovery(
            technologies_mask,
            reader_mode,
            enable_host_routing,
            restart,
        );
    }

    fn disable_discovery(&self) {
        platform_nci::SystemBackend.disable_discovery();
    }

    fn transceive(&self, handle: u32, tx: &[u8], rx: &mut [u8], timeout: i32) -> i32 {
        platform_nci::SystemBackend.transceive(handle, tx, rx, timeout)
    }

    fn current_tag_snapshot(&self) -> Option<TagInfo> {
        platform_nci::SystemBackend.current_tag_snapshot()
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Default)]
struct BackendTestState {
    init_result: i32,
    initialize_calls: usize,
    deinitialize_calls: usize,
    register_calls: usize,
    deregister_calls: usize,
    enable_calls: usize,
    disable_calls: usize,
    last_discovery_args: Option<(i32, i32, i32, i32)>,
    transceive_result: i32,
    transceive_response: Vec<u8>,
    last_transceive_handle: Option<u32>,
    last_transceive_tx: Vec<u8>,
    last_transceive_timeout: Option<i32>,
    current_tag: Option<TagInfo>,
}

#[cfg(test)]
#[derive(Default)]
struct FakeNciBackend {
    state: Mutex<BackendTestState>,
}

#[cfg(test)]
impl NciBackend for FakeNciBackend {
    fn initialize(&self) -> i32 {
        let mut state = self.state.lock().unwrap();
        state.initialize_calls += 1;
        state.init_result
    }

    fn deinitialize(&self) {
        let mut state = self.state.lock().unwrap();
        state.deinitialize_calls += 1;
        state.current_tag = None;
    }

    fn register_callbacks(&self) {
        let mut state = self.state.lock().unwrap();
        state.register_calls += 1;
        state.current_tag = None;
    }

    fn deregister_callbacks(&self) {
        let mut state = self.state.lock().unwrap();
        state.deregister_calls += 1;
        state.current_tag = None;
    }

    fn enable_discovery(
        &self,
        technologies_mask: i32,
        reader_mode: i32,
        enable_host_routing: i32,
        restart: i32,
    ) {
        let mut state = self.state.lock().unwrap();
        state.enable_calls += 1;
        state.last_discovery_args =
            Some((technologies_mask, reader_mode, enable_host_routing, restart));
    }

    fn disable_discovery(&self) {
        self.state.lock().unwrap().disable_calls += 1;
    }

    fn transceive(&self, handle: u32, tx: &[u8], rx: &mut [u8], timeout: i32) -> i32 {
        let mut state = self.state.lock().unwrap();
        state.last_transceive_handle = Some(handle);
        state.last_transceive_timeout = Some(timeout);
        state.last_transceive_tx = tx.to_vec();
        if state.transceive_result <= 0 {
            return state.transceive_result;
        }

        let copy_len = state
            .transceive_response
            .len()
            .min(rx.len())
            .min(state.transceive_result as usize);
        rx[..copy_len].copy_from_slice(&state.transceive_response[..copy_len]);
        state.transceive_result
    }

    fn current_tag_snapshot(&self) -> Option<TagInfo> {
        self.state.lock().unwrap().current_tag
    }
}

#[cfg(all(feature = "nci_helper", not(test)))]
fn backend() -> &'static dyn NciBackend {
    static BACKEND: SystemNciBackend = SystemNciBackend;
    &BACKEND
}

#[cfg(test)]
fn backend() -> &'static FakeNciBackend {
    static BACKEND: OnceLock<FakeNciBackend> = OnceLock::new();
    BACKEND.get_or_init(FakeNciBackend::default)
}

fn runtime() -> &'static Mutex<Pn71xxRuntime> {
    static RUNTIME: OnceLock<Mutex<Pn71xxRuntime>> = OnceLock::new();
    RUNTIME.get_or_init(|| Mutex::new(Pn71xxRuntime::default()))
}

fn clear_runtime_state() {
    let mut state = runtime().lock().unwrap();
    *state = Pn71xxRuntime::default();
}

fn normalize_inactive_runtime() {
    let snapshot = runtime().lock().unwrap().clone();
    if snapshot.active_device.is_some() {
        return;
    }

    if snapshot.discovery_enabled {
        backend().disable_discovery();
    }
    if snapshot.callbacks_registered {
        backend().deregister_callbacks();
    }
    if snapshot.initialized {
        backend().deinitialize();
    }

    clear_runtime_state();
}

fn technology_matches(tag: &TagInfo, modulation: ModulationType) -> bool {
    match modulation {
        ModulationType::Iso14443A => matches!(
            tag.technology,
            TARGET_TYPE_ISO14443_4
                | TARGET_TYPE_ISO14443_3A
                | TARGET_TYPE_MIFARE_CLASSIC
                | TARGET_TYPE_MIFARE_UL
        ),
        ModulationType::Iso14443B
        | ModulationType::Iso14443Bi
        | ModulationType::Iso14443B2Sr
        | ModulationType::Iso14443B2Ct => tag.technology == TARGET_TYPE_ISO14443_3B,
        ModulationType::Felica => tag.technology == TARGET_TYPE_FELICA,
        ModulationType::Jewel => {
            tag.technology == TARGET_TYPE_ISO14443_3A && tag.protocol == NFA_PROTOCOL_T1T
        }
        _ => false,
    }
}

fn current_tag_snapshot() -> Option<TagInfo> {
    backend().current_tag_snapshot()
}

fn build_target(tag: &TagInfo, nm: Modulation) -> Option<Target> {
    if !technology_matches(tag, nm.modulation_type) {
        return None;
    }

    let uid_len = (tag.uid_length as usize).min(tag.uid.len());
    if uid_len == 0 {
        return None;
    }

    let target = match nm.modulation_type {
        ModulationType::Iso14443A => Target {
            modulation: nm,
            info: TargetInfo::Iso14443A {
                atqa: [0x00, 0x00],
                sak: if tag.technology == TARGET_TYPE_MIFARE_CLASSIC {
                    0x08
                } else {
                    0x20
                },
                uid: tag.uid[..uid_len].to_vec(),
                ats: if tag.technology == TARGET_TYPE_MIFARE_CLASSIC {
                    Vec::new()
                } else {
                    DESFIRE_ATS.to_vec()
                },
            },
        },
        ModulationType::Iso14443B => {
            let mut pupi = [0u8; 4];
            let copy_len = uid_len.min(pupi.len());
            pupi[..copy_len].copy_from_slice(&tag.uid[..copy_len]);
            Target {
                modulation: nm,
                info: TargetInfo::Iso14443B {
                    pupi,
                    application_data: [0; 4],
                    protocol_info: [0; 3],
                    card_identifier: 0,
                },
            }
        }
        ModulationType::Iso14443Bi => {
            let mut div = [0u8; 4];
            let copy_len = uid_len.min(div.len());
            div[..copy_len].copy_from_slice(&tag.uid[..copy_len]);
            Target {
                modulation: nm,
                info: TargetInfo::Iso14443Bi {
                    div,
                    version_log: 0,
                    config: 0,
                    atr: Vec::new(),
                },
            }
        }
        ModulationType::Iso14443B2Sr => {
            let mut uid = [0u8; 8];
            let copy_len = uid_len.min(uid.len());
            uid[..copy_len].copy_from_slice(&tag.uid[..copy_len]);
            Target {
                modulation: nm,
                info: TargetInfo::Iso14443B2Sr { uid },
            }
        }
        ModulationType::Iso14443B2Ct => {
            let mut uid = [0u8; 4];
            let copy_len = uid_len.min(uid.len());
            uid[..copy_len].copy_from_slice(&tag.uid[..copy_len]);
            Target {
                modulation: nm,
                info: TargetInfo::Iso14443B2Ct {
                    uid,
                    product_code: 0,
                    fabrication_code: 0,
                },
            }
        }
        ModulationType::Felica => {
            let mut id = [0u8; 8];
            let copy_len = uid_len.min(id.len());
            id[..copy_len].copy_from_slice(&tag.uid[..copy_len]);
            Target {
                modulation: nm,
                info: TargetInfo::Felica {
                    len: copy_len,
                    response_code: 0,
                    id,
                    pad: [0; 8],
                    system_code: [0; 2],
                },
            }
        }
        ModulationType::Jewel => {
            let mut id = [0u8; 4];
            let copy_len = uid_len.min(id.len());
            id[..copy_len].copy_from_slice(&tag.uid[..copy_len]);
            Target {
                modulation: nm,
                info: TargetInfo::Jewel {
                    sens_res: [0; 2],
                    id,
                },
            }
        }
        _ => return None,
    };

    Some(target)
}
fn close_active_device(device_id: u64) {
    let snapshot = runtime().lock().unwrap().clone();
    if snapshot.active_device != Some(device_id) {
        return;
    }

    if snapshot.discovery_enabled {
        backend().disable_discovery();
    }
    if snapshot.callbacks_registered {
        backend().deregister_callbacks();
    }

    {
        let mut state = runtime().lock().unwrap();
        state.discovery_enabled = false;
        state.callbacks_registered = false;
        state.active_device = None;
    }

    if snapshot.initialized {
        backend().deinitialize();
    }

    runtime().lock().unwrap().initialized = false;
}

pub(super) struct Pn71xxDriver;

impl Pn71xxDriver {
    pub(super) fn new() -> Self {
        Self
    }
}

impl Driver for Pn71xxDriver {
    fn name(&self) -> &str {
        PN71XX_DRIVER_NAME
    }

    fn scan_type(&self) -> ScanType {
        ScanType::NotIntrusive
    }

    fn scan(&self, _context: &Context) -> Result<Vec<ConnectionString>, Error> {
        normalize_inactive_runtime();

        if runtime().lock().unwrap().active_device.is_some() {
            return Ok(vec![ConnectionString::new(PN71XX_DRIVER_NAME).unwrap()]);
        }

        if backend().initialize() != 0 {
            return Ok(Vec::new());
        }
        backend().deinitialize();

        Ok(vec![ConnectionString::new(PN71XX_DRIVER_NAME).unwrap()])
    }

    fn open(
        &self,
        _context: &Context,
        connstring: &ConnectionString,
    ) -> Result<Box<dyn DeviceHandle>, Error> {
        normalize_inactive_runtime();

        if runtime().lock().unwrap().active_device.is_some() {
            return Err(Error::DriverOpenFailed(
                "pn71xx only supports one active device at a time".to_string(),
            ));
        }

        let rc = backend().initialize();
        if rc != 0 {
            return Err(Error::DriverOpenFailed(format!(
                "pn71xx backend initialization failed with rc={rc}"
            )));
        }

        backend().register_callbacks();
        backend().enable_discovery(DEFAULT_NFA_TECH_MASK, 1, 0, 0);
        thread::sleep(NFC_SETTLE_DELAY);

        let device_id = {
            let mut state = runtime().lock().unwrap();
            let device_id = state.next_device_id;
            state.next_device_id = state.next_device_id.wrapping_add(1);
            state.initialized = true;
            state.callbacks_registered = true;
            state.discovery_enabled = true;
            state.active_device = Some(device_id);
            device_id
        };

        Ok(Box::new(Pn71xxDevice {
            device_id,
            connstring: connstring.clone(),
            last_error: NFC_SUCCESS,
        }))
    }
}

struct Pn71xxDevice {
    device_id: u64,
    connstring: ConnectionString,
    last_error: i32,
}

impl Pn71xxDevice {
    fn succeed<T>(&mut self, value: T) -> Result<T, Error> {
        self.last_error = NFC_SUCCESS;
        Ok(value)
    }

    fn fail<T>(&mut self, operation: &'static str, code: i32) -> Result<T, Error> {
        self.last_error = code;
        Err(device_error(operation, code))
    }
}

impl Drop for Pn71xxDevice {
    fn drop(&mut self) {
        close_active_device(self.device_id);
    }
}

impl DeviceMeta for Pn71xxDevice {
    fn name(&self) -> &str {
        PN71XX_DEVICE_NAME
    }

    fn connstring(&self) -> &ConnectionString {
        &self.connstring
    }

    fn caps(&self) -> DeviceCaps {
        DeviceCaps::INFO
            | DeviceCaps::SET_PROPERTY_BOOL
            | DeviceCaps::SET_PROPERTY_INT
            | DeviceCaps::SUPPORTED_MODULATIONS
            | DeviceCaps::SUPPORTED_BAUD_RATES
            | DeviceCaps::INITIATOR_INIT
            | DeviceCaps::SELECT_PASSIVE_TARGET
            | DeviceCaps::POLL_TARGET
            | DeviceCaps::DESELECT_TARGET
            | DeviceCaps::TARGET_IS_PRESENT
            | DeviceCaps::TRANSCEIVE_BYTES
            | DeviceCaps::ABORT_COMMAND
            | DeviceCaps::IDLE
            | DeviceCaps::POWERDOWN
    }

    fn last_error(&self) -> i32 {
        self.last_error
    }
}

impl InfoBackend for Pn71xxDevice {
    fn information_about(&mut self) -> Result<String, Error> {
        self.succeed(PN71XX_INFO.to_string())
    }
}

impl PropertyBackend for Pn71xxDevice {
    fn set_property_bool(&mut self, _property: Property, _enable: bool) -> Result<(), Error> {
        self.succeed(())
    }

    fn set_property_int(&mut self, _property: Property, _value: i32) -> Result<(), Error> {
        self.succeed(())
    }

    fn supported_modulations(&mut self, _mode: Mode) -> Result<Vec<ModulationType>, Error> {
        self.succeed(SUPPORTED_MODULATIONS.to_vec())
    }

    fn supported_baud_rates(
        &mut self,
        _mode: Mode,
        modulation_type: ModulationType,
    ) -> Result<Vec<BaudRate>, Error> {
        let values = match modulation_type {
            ModulationType::Felica => FELICA_SUPPORTED_BAUD_RATES,
            ModulationType::Iso14443A => ISO14443A_SUPPORTED_BAUD_RATES,
            ModulationType::Iso14443B
            | ModulationType::Iso14443Bi
            | ModulationType::Iso14443B2Sr
            | ModulationType::Iso14443B2Ct => ISO14443B_SUPPORTED_BAUD_RATES,
            ModulationType::Jewel => JEWEL_SUPPORTED_BAUD_RATES,
            ModulationType::Dep => DEP_SUPPORTED_BAUD_RATES,
            _ => return self.fail("pn71xx_get_supported_baud_rate", NFC_EINVARG),
        };
        self.succeed(values.to_vec())
    }
}

impl InitiatorBackend for Pn71xxDevice {
    fn initiator_init_driver(&mut self) -> Result<i32, Error> {
        self.succeed(0)
    }

    fn select_passive_target_driver(
        &mut self,
        nm: Modulation,
        _init_data: &[u8],
    ) -> Result<Option<Target>, Error> {
        self.succeed(current_tag_snapshot().and_then(|tag| build_target(&tag, nm)))
    }

    fn poll_target_driver(
        &mut self,
        modulations: &[Modulation],
        poll_nr: u8,
        period: u8,
    ) -> Result<Option<Target>, Error> {
        let sleep_duration = Duration::from_micros(period as u64 * POLL_PERIOD_FACTOR_MICROS);
        for _ in 0..poll_nr {
            for modulation in modulations {
                if let Some(target) = self.select_passive_target_driver(*modulation, &[])? {
                    return self.succeed(Some(target));
                }
            }
            if !sleep_duration.is_zero() {
                thread::sleep(sleep_duration);
            }
        }

        self.succeed(None)
    }

    fn deselect_target_driver(&mut self) -> Result<(), Error> {
        self.succeed(())
    }

    fn transceive_bytes_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        _timeout: i32,
    ) -> Result<usize, Error> {
        let Some(tag) = current_tag_snapshot() else {
            return self.fail("pn71xx_transceive_bytes", NFC_EINVARG);
        };

        let received = backend().transceive(tag.handle, tx, rx, 500);
        if received <= 0 {
            return self.fail("pn71xx_transceive_bytes", NFC_EIO);
        }

        self.succeed(received as usize)
    }

    fn target_is_present_driver(&mut self, _target: Option<&Target>) -> Result<bool, Error> {
        self.succeed(current_tag_snapshot().is_some())
    }

    fn abort_command_driver(&mut self) -> Result<(), Error> {
        self.succeed(())
    }

    fn idle_driver(&mut self) -> Result<(), Error> {
        self.succeed(())
    }

    fn powerdown_driver(&mut self) -> Result<(), Error> {
        self.succeed(())
    }
}

impl TargetBackend for Pn71xxDevice {}

impl Pn53xBackend for Pn71xxDevice {}

#[cfg(test)]
fn reset_test_world() {
    clear_runtime_state();
    *backend().state.lock().unwrap() = BackendTestState::default();
}

#[cfg(test)]
fn emit_tag_arrival_for_tests(tag: TagInfo) {
    let callbacks_registered = runtime().lock().unwrap().callbacks_registered;
    if callbacks_registered {
        backend().state.lock().unwrap().current_tag = Some(tag);
    }
}

#[cfg(test)]
fn emit_tag_departure_for_tests() {
    let callbacks_registered = runtime().lock().unwrap().callbacks_registered;
    if callbacks_registered {
        backend().state.lock().unwrap().current_tag = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    trait TestDeviceOps {
        fn select_passive_target(
            &mut self,
            modulation: Modulation,
            init_data: Option<&[u8]>,
        ) -> Result<Option<Target>, Error>;

        fn poll_target(
            &mut self,
            modulations: &[Modulation],
            poll_nr: u8,
            period: u8,
        ) -> Result<Option<Target>, Error>;

        fn transceive_bytes(
            &mut self,
            tx: &[u8],
            rx: &mut [u8],
            timeout: i32,
        ) -> Result<usize, Error>;

        fn target_is_present(&mut self, target: Option<&Target>) -> Result<bool, Error>;
    }

    impl TestDeviceOps for proximate_driver::Device {
        fn select_passive_target(
            &mut self,
            modulation: Modulation,
            init_data: Option<&[u8]>,
        ) -> Result<Option<Target>, Error> {
            let mut initiator = self.initiator()?;
            initiator.select_passive_target(modulation, init_data)
        }

        fn poll_target(
            &mut self,
            modulations: &[Modulation],
            poll_nr: u8,
            period: u8,
        ) -> Result<Option<Target>, Error> {
            let mut initiator = self.initiator()?;
            initiator.poll_target(modulations, poll_nr, period)
        }

        fn transceive_bytes(
            &mut self,
            tx: &[u8],
            rx: &mut [u8],
            timeout: i32,
        ) -> Result<usize, Error> {
            let mut initiator = self.initiator()?;
            initiator.transceive_bytes(tx, rx, timeout)
        }

        fn target_is_present(&mut self, target: Option<&Target>) -> Result<bool, Error> {
            let mut initiator = self.initiator()?;
            initiator.target_is_present(target)
        }
    }

    fn test_guard() -> &'static Mutex<()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD.get_or_init(|| Mutex::new(()))
    }

    fn make_tag(technology: u32, uid: &[u8], protocol: u8) -> TagInfo {
        let mut tag = TagInfo {
            technology,
            handle: 0x1234,
            protocol,
            ..Default::default()
        };
        let copy_len = uid.len().min(tag.uid.len());
        tag.uid[..copy_len].copy_from_slice(&uid[..copy_len]);
        tag.uid_length = copy_len as u32;
        tag
    }

    fn open_device(connstring: &ConnectionString) -> proximate_driver::Device {
        let driver = Pn71xxDriver::new();
        proximate_driver::Device::from_handle(driver.open(&Context::new(), connstring).unwrap())
    }

    #[test]
    fn scan_reports_success_and_failure() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        let driver = Pn71xxDriver::new();
        let found = driver.scan(&Context::new()).unwrap();
        assert_eq!(found, vec![ConnectionString::new("pn71xx").unwrap()]);
        let snapshot = backend().state.lock().unwrap().clone();
        assert_eq!(snapshot.initialize_calls, 1);
        assert_eq!(snapshot.deinitialize_calls, 1);

        reset_test_world();
        backend().state.lock().unwrap().init_result = -1;
        let found = driver.scan(&Context::new()).unwrap();
        assert!(found.is_empty());
        let snapshot = backend().state.lock().unwrap().clone();
        assert_eq!(snapshot.initialize_calls, 1);
        assert_eq!(snapshot.deinitialize_calls, 0);
    }

    #[test]
    fn open_works_without_prior_scan() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        let connstring = ConnectionString::new("pn71xx").unwrap();
        let device = open_device(&connstring);
        assert_eq!(device.name(), "pn71xx-device");
        let snapshot = backend().state.lock().unwrap().clone();
        assert_eq!(snapshot.initialize_calls, 1);
        assert_eq!(snapshot.register_calls, 1);
        assert_eq!(snapshot.enable_calls, 1);

        drop(device);
    }

    #[test]
    fn second_concurrent_open_is_rejected() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        let connstring = ConnectionString::new("pn71xx").unwrap();
        let first = open_device(&connstring);
        let driver = Pn71xxDriver::new();
        let error = match driver.open(&Context::new(), &connstring) {
            Ok(_) => panic!("second open should be rejected"),
            Err(error) => error,
        };
        assert!(matches!(error, Error::DriverOpenFailed(_)));

        drop(first);
    }

    #[test]
    fn close_tears_down_runtime_and_backend() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        let connstring = ConnectionString::new("pn71xx").unwrap();
        let device = open_device(&connstring);
        emit_tag_arrival_for_tests(make_tag(TARGET_TYPE_ISO14443_3A, &[0x11, 0x22], 0));

        drop(device);

        let runtime = runtime().lock().unwrap().clone();
        assert!(!runtime.initialized);
        assert!(!runtime.callbacks_registered);
        assert!(!runtime.discovery_enabled);
        assert!(runtime.active_device.is_none());

        let backend = backend().state.lock().unwrap().clone();
        assert_eq!(backend.disable_calls, 1);
        assert_eq!(backend.deregister_calls, 1);
        assert_eq!(backend.deinitialize_calls, 1);
        assert!(backend.current_tag.is_none());
    }

    #[test]
    fn callbacks_update_cached_tag_state() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        let connstring = ConnectionString::new("pn71xx").unwrap();
        let device = open_device(&connstring);

        emit_tag_arrival_for_tests(make_tag(TARGET_TYPE_ISO14443_3A, &[0x44], 0));
        assert!(current_tag_snapshot().is_some());

        emit_tag_departure_for_tests();
        assert!(current_tag_snapshot().is_none());

        drop(device);
    }

    #[test]
    fn select_passive_target_maps_supported_technology_families() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        let connstring = ConnectionString::new("pn71xx").unwrap();
        let mut device = open_device(&connstring);

        let cases = [
            (
                make_tag(TARGET_TYPE_MIFARE_CLASSIC, &[0x01, 0x02, 0x03, 0x04], 0),
                Modulation {
                    modulation_type: ModulationType::Iso14443A,
                    baud_rate: BaudRate::Br106,
                },
            ),
            (
                make_tag(TARGET_TYPE_ISO14443_3A, &[0x10, 0x11, 0x12, 0x13], 0),
                Modulation {
                    modulation_type: ModulationType::Iso14443A,
                    baud_rate: BaudRate::Br106,
                },
            ),
            (
                make_tag(TARGET_TYPE_ISO14443_3B, &[0x21, 0x22, 0x23, 0x24], 0),
                Modulation {
                    modulation_type: ModulationType::Iso14443B,
                    baud_rate: BaudRate::Br106,
                },
            ),
            (
                make_tag(TARGET_TYPE_ISO14443_3B, &[0x31, 0x32, 0x33, 0x34], 0),
                Modulation {
                    modulation_type: ModulationType::Iso14443Bi,
                    baud_rate: BaudRate::Br106,
                },
            ),
            (
                make_tag(
                    TARGET_TYPE_ISO14443_3B,
                    &[0x41, 0x42, 0x43, 0x44, 0x45, 0x46],
                    0,
                ),
                Modulation {
                    modulation_type: ModulationType::Iso14443B2Sr,
                    baud_rate: BaudRate::Br106,
                },
            ),
            (
                make_tag(TARGET_TYPE_ISO14443_3B, &[0x51, 0x52, 0x53, 0x54], 0),
                Modulation {
                    modulation_type: ModulationType::Iso14443B2Ct,
                    baud_rate: BaudRate::Br106,
                },
            ),
            (
                make_tag(
                    TARGET_TYPE_FELICA,
                    &[0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68],
                    0,
                ),
                Modulation {
                    modulation_type: ModulationType::Felica,
                    baud_rate: BaudRate::Br212,
                },
            ),
            (
                make_tag(
                    TARGET_TYPE_ISO14443_3A,
                    &[0x71, 0x72, 0x73, 0x74],
                    NFA_PROTOCOL_T1T,
                ),
                Modulation {
                    modulation_type: ModulationType::Jewel,
                    baud_rate: BaudRate::Br106,
                },
            ),
        ];

        for (tag, modulation) in cases {
            emit_tag_arrival_for_tests(tag);
            let target = device
                .select_passive_target(modulation, None)
                .unwrap()
                .expect("target should be present");

            match target.info {
                TargetInfo::Iso14443A { uid, sak, ats, .. } => {
                    assert_eq!(uid, tag.uid[..tag.uid_length as usize].to_vec());
                    if tag.technology == TARGET_TYPE_MIFARE_CLASSIC {
                        assert_eq!(sak, 0x08);
                        assert!(ats.is_empty());
                    } else {
                        assert_eq!(sak, 0x20);
                        assert_eq!(ats, DESFIRE_ATS.to_vec());
                    }
                }
                TargetInfo::Iso14443B { pupi, .. } => assert_eq!(pupi, [0x21, 0x22, 0x23, 0x24]),
                TargetInfo::Iso14443Bi { div, .. } => assert_eq!(div, [0x31, 0x32, 0x33, 0x34]),
                TargetInfo::Iso14443B2Sr { uid } => {
                    assert_eq!(&uid[..6], &[0x41, 0x42, 0x43, 0x44, 0x45, 0x46])
                }
                TargetInfo::Iso14443B2Ct { uid, .. } => assert_eq!(uid, [0x51, 0x52, 0x53, 0x54]),
                TargetInfo::Felica { id, .. } => {
                    assert_eq!(id, [0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68])
                }
                TargetInfo::Jewel { id, .. } => assert_eq!(id, [0x71, 0x72, 0x73, 0x74]),
                _ => panic!("unexpected target kind"),
            }
        }
    }

    #[test]
    fn poll_target_retries_until_tag_appears() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        let connstring = ConnectionString::new("pn71xx").unwrap();
        let mut device = open_device(&connstring);
        let worker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));
            emit_tag_arrival_for_tests(make_tag(TARGET_TYPE_ISO14443_3A, &[0xAA, 0xBB], 0));
        });

        let modulations = [Modulation {
            modulation_type: ModulationType::Iso14443A,
            baud_rate: BaudRate::Br106,
        }];
        let target = device
            .poll_target(&modulations, 2, 1)
            .unwrap()
            .expect("target should appear");
        worker.join().unwrap();
        match target.info {
            TargetInfo::Iso14443A { uid, .. } => assert_eq!(uid, vec![0xAA, 0xBB]),
            _ => panic!("unexpected target kind"),
        }
    }

    #[test]
    fn transceive_bytes_handles_missing_and_present_tags() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        let connstring = ConnectionString::new("pn71xx").unwrap();
        let mut device = open_device(&connstring);
        let tx = [0x30u8, 0x04];
        let mut rx = [0u8; 8];

        let missing = device.transceive_bytes(&tx, &mut rx, 250).unwrap_err();
        assert_eq!(missing.device_code(), Some(NFC_EINVARG));

        emit_tag_arrival_for_tests(make_tag(TARGET_TYPE_ISO14443_3A, &[0x01], 0));
        {
            let mut state = backend().state.lock().unwrap();
            state.transceive_result = 4;
            state.transceive_response = vec![0xDE, 0xAD, 0xBE, 0xEF];
        }

        let received = device.transceive_bytes(&tx, &mut rx, 250).unwrap();
        assert_eq!(received, 4);
        assert_eq!(&rx[..4], &[0xDE, 0xAD, 0xBE, 0xEF]);

        let state = backend().state.lock().unwrap().clone();
        assert_eq!(state.last_transceive_handle, Some(0x1234));
        assert_eq!(state.last_transceive_tx, tx);
        assert_eq!(state.last_transceive_timeout, Some(500));
    }

    #[test]
    fn target_is_present_follows_tag_cache() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        let connstring = ConnectionString::new("pn71xx").unwrap();
        let mut device = open_device(&connstring);
        assert!(!device.target_is_present(None).unwrap());

        emit_tag_arrival_for_tests(make_tag(TARGET_TYPE_ISO14443_3A, &[0x01], 0));
        assert!(device.target_is_present(None).unwrap());

        emit_tag_departure_for_tests();
        assert!(!device.target_is_present(None).unwrap());
    }

    #[test]
    fn device_get_information_about_returns_expected_string() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        let connstring = ConnectionString::new("pn71xx").unwrap();
        let mut device = open_device(&connstring);
        assert_eq!(
            device.information_about().unwrap(),
            "PN71XX nfc driver using libnfc-nci userspace library"
        );
    }
}
