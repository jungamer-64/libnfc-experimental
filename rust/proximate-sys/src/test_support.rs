use crate::c_abi::types::{
    nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_mode, nfc_modulation, nfc_modulation_type,
    nfc_property, nfc_target,
};
use crate::c_boundary::NFC_BUFSIZE_CONNSTRING;
use crate::lifecycle::{
    DEVICE_NAME_LENGTH, nfc_connstring, nfc_context, nfc_driver, scan_type_enum,
};
use crate::lifecycle::{attach_device, nfc_device};
use libc::{c_char, c_int, c_void};
use proximate_driver as rt;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

pub(crate) struct FakeState {
    pub(crate) aborts: AtomicUsize,
    pub(crate) closes: AtomicUsize,
    pub(crate) active: AtomicUsize,
    pub(crate) max_active: AtomicUsize,
    pub(crate) panic_next: AtomicBool,
    pub(crate) block: AtomicBool,
    released: (Mutex<bool>, Condvar),
}

impl Default for FakeState {
    fn default() -> Self {
        Self {
            aborts: AtomicUsize::new(0),
            closes: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
            panic_next: AtomicBool::new(false),
            block: AtomicBool::new(false),
            released: (Mutex::new(false), Condvar::new()),
        }
    }
}

struct FakeAbort(Arc<FakeState>);

impl rt::CommandAbort for FakeAbort {
    fn abort(&self) -> Result<(), rt::Error> {
        self.0.aborts.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut released) = self.0.released.0.lock() {
            *released = true;
            self.0.released.1.notify_all();
        }
        Ok(())
    }
}

struct FakeDevice {
    name: String,
    connstring: rt::ConnectionString,
    state: Arc<FakeState>,
}

impl Drop for FakeDevice {
    fn drop(&mut self) {
        self.state.closes.fetch_add(1, Ordering::SeqCst);
    }
}

impl rt::DeviceMeta for FakeDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn connstring(&self) -> &rt::ConnectionString {
        &self.connstring
    }
}

impl rt::InfoBackend for FakeDevice {
    fn information_about(&mut self) -> Result<String, rt::Error> {
        Ok("fake device information".to_string())
    }
}

impl rt::PropertyBackend for FakeDevice {
    fn set_property_bool(
        &mut self,
        _property: rt::Property,
        _enable: bool,
    ) -> Result<(), rt::Error> {
        Ok(())
    }

    fn set_timeout(
        &mut self,
        _property: rt::TimeoutProperty,
        _timeout: rt::OperationTimeout,
    ) -> Result<(), rt::Error> {
        Ok(())
    }

    fn supported_modulations(
        &mut self,
        _mode: rt::Mode,
    ) -> Result<Vec<rt::ModulationType>, rt::Error> {
        Ok(vec![rt::ModulationType::Iso14443A, rt::ModulationType::Dep])
    }

    fn supported_baud_rates(
        &mut self,
        _mode: rt::Mode,
        _modulation_type: rt::ModulationType,
    ) -> Result<Vec<rt::BaudRate>, rt::Error> {
        Ok(vec![rt::BaudRate::Br106, rt::BaudRate::Br424])
    }
}

impl rt::InitiatorBackend for FakeDevice {
    fn command_abort_handle(&self) -> Option<rt::CommandAbortHandle> {
        Some(Arc::new(FakeAbort(Arc::clone(&self.state))))
    }

    fn initiator_init_driver(&mut self) -> Result<i32, rt::Error> {
        if self.state.panic_next.swap(false, Ordering::SeqCst) {
            panic!("fake backend panic");
        }
        Ok(0)
    }

    fn select_passive_target_driver(
        &mut self,
        modulation: rt::Modulation,
        _init_data: &[u8],
    ) -> Result<Option<rt::Target>, rt::Error> {
        Ok(Some(rt::Target::try_new(
            modulation,
            rt::TargetInfo::Iso14443A {
                atqa: [0x04, 0x00],
                sak: 0x08,
                uid: vec![1, 2, 3, 4],
                ats: Vec::new(),
            },
        )?))
    }

    fn transceive_bytes_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        _timeout: rt::OperationTimeout,
    ) -> Result<usize, rt::Error> {
        let active = self.state.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.state.max_active.fetch_max(active, Ordering::SeqCst);
        if self.state.block.load(Ordering::SeqCst) {
            let (lock, wake) = &self.state.released;
            let mut released = lock.lock().map_err(|_| rt::Error::Io("fake lock"))?;
            while !*released {
                released = wake
                    .wait(released)
                    .map_err(|_| rt::Error::Io("fake wait"))?;
            }
        } else {
            std::thread::sleep(Duration::from_millis(10));
        }
        let count = tx.len().min(rx.len());
        rx[..count].copy_from_slice(&tx[..count]);
        self.state.active.fetch_sub(1, Ordering::SeqCst);
        Ok(count)
    }
}

impl rt::TargetBackend for FakeDevice {
    fn target_send_bytes_driver(
        &mut self,
        tx: &[u8],
        _timeout: rt::OperationTimeout,
    ) -> Result<usize, rt::Error> {
        Ok(tx.len())
    }

    fn target_receive_bytes_driver(
        &mut self,
        rx: &mut [u8],
        _timeout: rt::OperationTimeout,
    ) -> Result<usize, rt::Error> {
        let bytes = [0xaa, 0xbb];
        let count = bytes.len().min(rx.len());
        rx[..count].copy_from_slice(&bytes[..count]);
        Ok(count)
    }
}

impl rt::Pn53xBackend for FakeDevice {}

struct FakeDriver {
    state: Arc<FakeState>,
}

impl rt::Driver for FakeDriver {
    fn name(&self) -> &str {
        "fake"
    }

    fn scan_type(&self) -> rt::ScanType {
        rt::ScanType::NotIntrusive
    }

    fn scan(&self, _context: &rt::Context) -> Result<rt::DriverScan, rt::Error> {
        let connstring = rt::ConnectionString::new("fake:device")?;
        Ok(rt::DriverScan::Complete(vec![self.describe_discovered(
            "fake device".to_string(),
            connstring,
        )]))
    }

    fn open(
        &self,
        _context: &rt::Context,
        connstring: &rt::ConnectionString,
    ) -> Result<Box<dyn rt::DeviceHandle>, rt::Error> {
        Ok(Box::new(FakeDevice {
            name: "fake device".to_string(),
            connstring: connstring.clone(),
            state: Arc::clone(&self.state),
        }))
    }
}

pub(crate) fn fake_abi_device() -> (*mut nfc_device, Arc<FakeState>) {
    let state = Arc::new(FakeState::default());
    let mut registry = rt::DriverRegistry::new();
    registry.register_driver(Box::new(FakeDriver {
        state: Arc::clone(&state),
    }));
    let context = rt::Context::new();
    let connstring = rt::ConnectionString::new("fake:device").expect("valid test connstring");
    let device = registry
        .open(&context, Some(&connstring))
        .expect("fake driver opens");
    (attach_device(device), state)
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ExternalState {
    pub(crate) scans: usize,
    pub(crate) opens: usize,
    pub(crate) closes: usize,
    pub(crate) aborts: usize,
    pub(crate) operations: Vec<&'static str>,
}

static EXTERNAL_STATE: std::sync::OnceLock<Mutex<ExternalState>> = std::sync::OnceLock::new();
static EXTERNAL_TEST_LOCK: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();

fn external_state() -> &'static Mutex<ExternalState> {
    EXTERNAL_STATE.get_or_init(|| Mutex::new(ExternalState::default()))
}

pub(crate) fn external_test_guard() -> std::sync::MutexGuard<'static, ()> {
    EXTERNAL_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("external test lock")
}

pub(crate) fn reset_external_state() {
    *external_state().lock().expect("external state") = ExternalState::default();
    crate::c_boundary::external_registry::clear_registry();
}

pub(crate) fn external_state_snapshot() -> ExternalState {
    external_state().lock().expect("external state").clone()
}

fn record_external(operation: &'static str) {
    external_state()
        .lock()
        .expect("external state")
        .operations
        .push(operation);
}

#[allow(non_snake_case)]
#[repr(C)]
struct LegacyTestDevice {
    context: *const nfc_context,
    driver: *const nfc_driver,
    driver_data: *mut c_void,
    chip_data: *mut c_void,
    command_abort: *mut c_void,
    name: [c_char; DEVICE_NAME_LENGTH],
    connstring: [c_char; NFC_BUFSIZE_CONNSTRING],
    bCrc: bool,
    bPar: bool,
    bEasyFraming: bool,
    bInfiniteSelect: bool,
    bAutoIso14443_4: bool,
    btSupportByte: u8,
    last_error: c_int,
}

unsafe fn copy_c_bytes(destination: *mut c_char, capacity: usize, value: &[u8]) {
    let count = value.len().min(capacity.saturating_sub(1));
    unsafe {
        std::ptr::copy_nonoverlapping(value.as_ptr(), destination.cast::<u8>(), count);
        *destination.add(count) = 0;
    }
}

unsafe extern "C" fn external_scan(
    _context: *const nfc_context,
    connstrings: *mut nfc_connstring,
    capacity: usize,
) -> usize {
    external_state().lock().expect("external state").scans += 1;
    if capacity != 0 && !connstrings.is_null() {
        unsafe {
            copy_c_bytes(
                (*connstrings).as_mut_ptr(),
                NFC_BUFSIZE_CONNSTRING,
                b"fakec:one",
            )
        };
    }
    1
}

unsafe extern "C" fn external_open(
    context: *const nfc_context,
    connstring: *const c_char,
) -> *mut nfc_device {
    external_state().lock().expect("external state").opens += 1;
    if connstring.is_null() {
        return std::ptr::null_mut();
    }
    let mut device: Box<LegacyTestDevice> = Box::new(unsafe { std::mem::zeroed() });
    device.context = context;
    unsafe {
        copy_c_bytes(
            device.name.as_mut_ptr(),
            device.name.len(),
            b"external fake",
        );
        let length = libc::strnlen(connstring, NFC_BUFSIZE_CONNSTRING);
        let value = std::slice::from_raw_parts(connstring.cast::<u8>(), length);
        copy_c_bytes(
            device.connstring.as_mut_ptr(),
            device.connstring.len(),
            value,
        );
    }
    Box::into_raw(device).cast()
}

unsafe extern "C" fn external_close(raw: *mut nfc_device) {
    external_state().lock().expect("external state").closes += 1;
    if !raw.is_null() {
        unsafe { drop(Box::from_raw(raw.cast::<LegacyTestDevice>())) };
    }
}

unsafe extern "C" fn external_strerror(_raw: *const nfc_device) -> *const c_char {
    c"external failure".as_ptr()
}

unsafe extern "C" fn external_init(_raw: *mut nfc_device) -> c_int {
    record_external("initiator_init");
    0
}

unsafe extern "C" fn external_secure_init(_raw: *mut nfc_device) -> c_int {
    record_external("secure_init");
    0
}

fn sample_target(modulation: nfc_modulation) -> nfc_target {
    nfc_target {
        nm: modulation,
        nti: crate::c_abi::types::nfc_target_info {
            nai: crate::c_abi::types::nfc_iso14443a_info {
                abtAtqa: [4, 0],
                btSak: 8,
                szUidLen: 4,
                abtUid: [1, 2, 3, 4, 0, 0, 0, 0, 0, 0],
                szAtsLen: 0,
                abtAts: [0; 254],
            },
        },
    }
}

unsafe extern "C" fn external_select(
    _raw: *mut nfc_device,
    modulation: nfc_modulation,
    _data: *const u8,
    _data_len: usize,
    target: *mut nfc_target,
) -> c_int {
    record_external("select");
    if !target.is_null() {
        unsafe { target.write_unaligned(sample_target(modulation)) };
    }
    1
}

unsafe extern "C" fn external_poll(
    raw: *mut nfc_device,
    modulations: *const nfc_modulation,
    count: usize,
    _iterations: u8,
    _period: u8,
    target: *mut nfc_target,
) -> c_int {
    record_external("poll");
    if count == 0 || modulations.is_null() {
        return 0;
    }
    unsafe {
        external_select(
            raw,
            modulations.read_unaligned(),
            std::ptr::null(),
            0,
            target,
        )
    }
}

unsafe extern "C" fn external_select_dep(
    _raw: *mut nfc_device,
    _mode: nfc_dep_mode,
    baud: nfc_baud_rate,
    _initiator: *const nfc_dep_info,
    target: *mut nfc_target,
    _timeout: c_int,
) -> c_int {
    record_external("select_dep");
    if !target.is_null() {
        let modulation = nfc_modulation {
            nmt: nfc_modulation_type::NMT_DEP,
            nbr: baud,
        };
        let dep = crate::c_abi::types::nfc_dep_info {
            abtNFCID3: [0; 10],
            btDID: 0,
            btBS: 0,
            btBR: 0,
            btTO: 0,
            btPP: 0,
            abtGB: [0; 48],
            szGB: 0,
            ndm: nfc_dep_mode::NDM_PASSIVE,
        };
        unsafe {
            target.write_unaligned(nfc_target {
                nm: modulation,
                nti: crate::c_abi::types::nfc_target_info { ndi: dep },
            })
        };
    }
    1
}

unsafe extern "C" fn external_simple(_raw: *mut nfc_device) -> c_int {
    record_external("simple");
    0
}

unsafe extern "C" fn external_present(_raw: *mut nfc_device, _target: *const nfc_target) -> c_int {
    record_external("present");
    0
}

unsafe extern "C" fn external_transceive_bytes(
    _raw: *mut nfc_device,
    tx: *const u8,
    tx_len: usize,
    rx: *mut u8,
    rx_len: usize,
    _timeout: c_int,
) -> c_int {
    record_external("transceive_bytes");
    let count = tx_len.min(rx_len);
    if count != 0 {
        unsafe { std::ptr::copy_nonoverlapping(tx, rx, count) };
    }
    count as c_int
}

unsafe extern "C" fn external_transceive_bits(
    raw: *mut nfc_device,
    tx: *const u8,
    tx_bits: usize,
    _tx_parity: *const u8,
    rx: *mut u8,
    _rx_parity: *mut u8,
) -> c_int {
    unsafe { external_transceive_bytes(raw, tx, tx_bits.div_ceil(8), rx, tx_bits.div_ceil(8), 0) }
}

unsafe extern "C" fn external_transceive_bytes_timed(
    raw: *mut nfc_device,
    tx: *const u8,
    tx_len: usize,
    rx: *mut u8,
    rx_len: usize,
    cycles: *mut u32,
) -> c_int {
    if !cycles.is_null() {
        unsafe { *cycles = 42 };
    }
    unsafe { external_transceive_bytes(raw, tx, tx_len, rx, rx_len, 0) }
}

unsafe extern "C" fn external_transceive_bits_timed(
    raw: *mut nfc_device,
    tx: *const u8,
    tx_bits: usize,
    parity: *const u8,
    rx: *mut u8,
    rx_parity: *mut u8,
    cycles: *mut u32,
) -> c_int {
    if !cycles.is_null() {
        unsafe { *cycles = 43 };
    }
    unsafe { external_transceive_bits(raw, tx, tx_bits, parity, rx, rx_parity) }
}

unsafe extern "C" fn external_target_init(
    _raw: *mut nfc_device,
    _target: *mut nfc_target,
    rx: *mut u8,
    rx_len: usize,
    _timeout: c_int,
) -> c_int {
    record_external("target_init");
    if rx_len != 0 {
        unsafe { *rx = 0x5a };
        1
    } else {
        0
    }
}

unsafe extern "C" fn external_target_send_bytes(
    _raw: *mut nfc_device,
    _tx: *const u8,
    len: usize,
    _timeout: c_int,
) -> c_int {
    record_external("target_send_bytes");
    len as c_int
}

unsafe extern "C" fn external_target_receive_bytes(
    _raw: *mut nfc_device,
    rx: *mut u8,
    len: usize,
    _timeout: c_int,
) -> c_int {
    record_external("target_receive_bytes");
    if len != 0 {
        unsafe { *rx = 0xa5 };
        1
    } else {
        0
    }
}

unsafe extern "C" fn external_target_send_bits(
    _raw: *mut nfc_device,
    _tx: *const u8,
    bits: usize,
    _parity: *const u8,
) -> c_int {
    record_external("target_send_bits");
    bits as c_int
}

unsafe extern "C" fn external_target_receive_bits(
    _raw: *mut nfc_device,
    rx: *mut u8,
    len: usize,
    _parity: *mut u8,
) -> c_int {
    record_external("target_receive_bits");
    if len != 0 {
        unsafe { *rx = 0x3c };
        8
    } else {
        0
    }
}

unsafe extern "C" fn external_property_bool(
    raw: *mut nfc_device,
    property: nfc_property,
    enable: bool,
) -> c_int {
    record_external("property_bool");
    let device = unsafe { &mut *raw.cast::<LegacyTestDevice>() };
    if property == nfc_property::NP_EASY_FRAMING {
        device.bEasyFraming = enable;
    }
    0
}

unsafe extern "C" fn external_property_int(
    _raw: *mut nfc_device,
    _property: nfc_property,
    _value: c_int,
) -> c_int {
    record_external("property_int");
    0
}

static EXTERNAL_MODULATIONS: [nfc_modulation_type; 2] = [
    nfc_modulation_type::NMT_ISO14443A,
    nfc_modulation_type::NMT_UNDEFINED,
];
static EXTERNAL_BAUD_RATES: [nfc_baud_rate; 2] =
    [nfc_baud_rate::NBR_106, nfc_baud_rate::NBR_UNDEFINED];

unsafe extern "C" fn external_modulations(
    _raw: *mut nfc_device,
    _mode: nfc_mode,
    output: *mut *const nfc_modulation_type,
) -> c_int {
    if !output.is_null() {
        unsafe { *output = EXTERNAL_MODULATIONS.as_ptr() };
    }
    0
}

unsafe extern "C" fn external_baud_rates(
    _raw: *mut nfc_device,
    _mode: nfc_mode,
    _modulation: nfc_modulation_type,
    output: *mut *const nfc_baud_rate,
) -> c_int {
    if !output.is_null() {
        unsafe { *output = EXTERNAL_BAUD_RATES.as_ptr() };
    }
    0
}

unsafe extern "C" fn external_information(
    _raw: *mut nfc_device,
    output: *mut *mut c_char,
) -> c_int {
    let value = b"external information\0";
    let allocation = unsafe { libc::malloc(value.len()) }.cast::<u8>();
    if allocation.is_null() {
        return -80;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(value.as_ptr(), allocation, value.len());
        *output = allocation.cast();
    }
    0
}

unsafe extern "C" fn external_abort(_raw: *mut nfc_device) -> c_int {
    external_state().lock().expect("external state").aborts += 1;
    0
}

pub(crate) fn external_driver(name: &'static std::ffi::CStr) -> nfc_driver {
    nfc_driver {
        name: name.as_ptr(),
        scan_type: scan_type_enum::NOT_INTRUSIVE,
        scan: Some(external_scan),
        open: Some(external_open),
        close: Some(external_close),
        strerror: Some(external_strerror),
        initiator_init: Some(external_init),
        initiator_init_secure_element: Some(external_secure_init),
        initiator_select_passive_target: Some(external_select),
        initiator_poll_target: Some(external_poll),
        initiator_select_dep_target: Some(external_select_dep),
        initiator_deselect_target: Some(external_simple),
        initiator_transceive_bytes: Some(external_transceive_bytes),
        initiator_transceive_bits: Some(external_transceive_bits),
        initiator_transceive_bytes_timed: Some(external_transceive_bytes_timed),
        initiator_transceive_bits_timed: Some(external_transceive_bits_timed),
        initiator_target_is_present: Some(external_present),
        target_init: Some(external_target_init),
        target_send_bytes: Some(external_target_send_bytes),
        target_receive_bytes: Some(external_target_receive_bytes),
        target_send_bits: Some(external_target_send_bits),
        target_receive_bits: Some(external_target_receive_bits),
        device_set_property_bool: Some(external_property_bool),
        device_set_property_int: Some(external_property_int),
        get_supported_modulation: Some(external_modulations),
        get_supported_baud_rate: Some(external_baud_rates),
        device_get_information_about: Some(external_information),
        abort_command: Some(external_abort),
        idle: Some(external_simple),
        powerdown: Some(external_simple),
    }
}
