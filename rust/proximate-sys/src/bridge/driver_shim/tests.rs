use super::external::ExternalDriver;
use super::rust_owned::RustDeviceState;
use super::*;
use proximate_driver::{Driver, OpenedDevice};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct RawScanState {
    capacities: Vec<usize>,
    results: VecDeque<usize>,
    open_attempts: Vec<String>,
}

static DRIVER_ALPHA_NAME: &[u8] = b"alpha\0";
static DRIVER_PRIMARY_USB_NAME: &[u8] = b"primary_usb\0";
static DRIVER_FALLBACK_USB_NAME: &[u8] = b"fallback_usb\0";

thread_local! {
    static RAW_SCAN_STATE: std::cell::RefCell<RawScanState> =
        std::cell::RefCell::new(RawScanState::default());
}

fn with_raw_scan_state<R>(f: impl FnOnce(&mut RawScanState) -> R) -> R {
    RAW_SCAN_STATE.with(|cell| f(&mut cell.borrow_mut()))
}

fn reset_raw_scan_state() {
    RAW_SCAN_STATE.with(|cell| {
        *cell.borrow_mut() = RawScanState::default();
    });
}

unsafe extern "C" fn test_scan(
    _context: *const nfc_context,
    connstrings: *mut crate::lifecycle::nfc_connstring,
    capacity: usize,
) -> usize {
    with_raw_scan_state(|state| state.capacities.push(capacity));
    let count = with_raw_scan_state(|state| state.results.pop_front().unwrap_or_default());
    for index in 0..count.min(capacity) {
        let value = CString::new(format!("alpha:{index:03}")).unwrap();
        unsafe {
            copy_bytes_to_c_buffer(
                connstrings.add(index).cast(),
                NFC_BUFSIZE_CONNSTRING,
                value.as_bytes(),
            );
        }
    }
    count
}

unsafe fn open_raw_device(
    driver: *const nfc_driver,
    context: *const nfc_context,
    connstring: *const c_char,
    name: &[u8],
) -> *mut nfc_device {
    let raw = unsafe { nfc_device_new(context, connstring) };
    if raw.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        (*raw).driver = driver;
        copy_bytes_to_c_buffer((*raw).name.as_mut_ptr(), DEVICE_NAME_LENGTH, &name[..name.len() - 1]);
    }
    raw
}

unsafe extern "C" fn primary_usb_open(
    _context: *const nfc_context,
    connstring: *const c_char,
) -> *mut nfc_device {
    let conn = c_string_ptr_to_string(connstring, bounded_strlen(connstring, NFC_BUFSIZE_CONNSTRING));
    with_raw_scan_state(|state| state.open_attempts.push(format!("primary:{conn}")));
    ptr::null_mut()
}

unsafe extern "C" fn fallback_usb_open(
    context: *const nfc_context,
    connstring: *const c_char,
) -> *mut nfc_device {
    let conn = c_string_ptr_to_string(connstring, bounded_strlen(connstring, NFC_BUFSIZE_CONNSTRING));
    with_raw_scan_state(|state| state.open_attempts.push(format!("fallback:{conn}")));
    unsafe {
        open_raw_device(
            ptr::addr_of!(FALLBACK_USB_DRIVER),
            context,
            connstring,
            b"backend fallback\0",
        )
    }
}

unsafe extern "C" fn raw_close(device: *mut nfc_device) {
    unsafe { crate::lifecycle::nfc_device_free(device) };
}

static SCAN_DRIVER: nfc_driver = nfc_driver {
    name: DRIVER_ALPHA_NAME.as_ptr() as *const c_char,
    scan_type: scan_type_enum::NOT_INTRUSIVE,
    scan: Some(test_scan),
    open: None,
    close: None,
    strerror: None,
    initiator_init: None,
    initiator_init_secure_element: None,
    initiator_select_passive_target: None,
    initiator_poll_target: None,
    initiator_select_dep_target: None,
    initiator_deselect_target: None,
    initiator_transceive_bytes: None,
    initiator_transceive_bits: None,
    initiator_transceive_bytes_timed: None,
    initiator_transceive_bits_timed: None,
    initiator_target_is_present: None,
    target_init: None,
    target_send_bytes: None,
    target_receive_bytes: None,
    target_send_bits: None,
    target_receive_bits: None,
    device_set_property_bool: None,
    device_set_property_int: None,
    get_supported_modulation: None,
    get_supported_baud_rate: None,
    device_get_information_about: None,
    abort_command: None,
    idle: None,
    powerdown: None,
};

static PRIMARY_USB_DRIVER: nfc_driver = nfc_driver {
    name: DRIVER_PRIMARY_USB_NAME.as_ptr() as *const c_char,
    scan_type: scan_type_enum::NOT_INTRUSIVE,
    scan: None,
    open: Some(primary_usb_open),
    close: Some(raw_close),
    strerror: None,
    initiator_init: None,
    initiator_init_secure_element: None,
    initiator_select_passive_target: None,
    initiator_poll_target: None,
    initiator_select_dep_target: None,
    initiator_deselect_target: None,
    initiator_transceive_bytes: None,
    initiator_transceive_bits: None,
    initiator_transceive_bytes_timed: None,
    initiator_transceive_bits_timed: None,
    initiator_target_is_present: None,
    target_init: None,
    target_send_bytes: None,
    target_receive_bytes: None,
    target_send_bits: None,
    target_receive_bits: None,
    device_set_property_bool: None,
    device_set_property_int: None,
    get_supported_modulation: None,
    get_supported_baud_rate: None,
    device_get_information_about: None,
    abort_command: None,
    idle: None,
    powerdown: None,
};

static FALLBACK_USB_DRIVER: nfc_driver = nfc_driver {
    name: DRIVER_FALLBACK_USB_NAME.as_ptr() as *const c_char,
    scan_type: scan_type_enum::NOT_INTRUSIVE,
    scan: None,
    open: Some(fallback_usb_open),
    close: Some(raw_close),
    strerror: None,
    initiator_init: None,
    initiator_init_secure_element: None,
    initiator_select_passive_target: None,
    initiator_poll_target: None,
    initiator_select_dep_target: None,
    initiator_deselect_target: None,
    initiator_transceive_bytes: None,
    initiator_transceive_bits: None,
    initiator_transceive_bytes_timed: None,
    initiator_transceive_bits_timed: None,
    initiator_target_is_present: None,
    target_init: None,
    target_send_bytes: None,
    target_receive_bytes: None,
    target_send_bits: None,
    target_receive_bits: None,
    device_set_property_bool: None,
    device_set_property_int: None,
    get_supported_modulation: None,
    get_supported_baud_rate: None,
    device_get_information_about: None,
    abort_command: None,
    idle: None,
    powerdown: None,
};

struct FakeRustHandle {
    name: String,
    connstring: rt::ConnectionString,
    caps: rt::DeviceCaps,
    property_calls: Arc<Mutex<Vec<(rt::Property, bool)>>>,
    info_result: Result<String, rt::Error>,
}

impl rt::OpenedDevice for FakeRustHandle {
    fn name(&self) -> &str {
        &self.name
    }

    fn connstring(&self) -> &rt::ConnectionString {
        &self.connstring
    }

    fn caps(&self) -> rt::DeviceCaps {
        self.caps
    }

    fn information_about(&mut self) -> Result<String, rt::Error> {
        self.info_result.clone()
    }

    fn set_property_bool(&mut self, property: rt::Property, enable: bool) -> Result<(), rt::Error> {
        self.property_calls.lock().unwrap().push((property, enable));
        Ok(())
    }

    fn set_property_int(&mut self, _property: rt::Property, _value: i32) -> Result<(), rt::Error> {
        Ok(())
    }

    fn supported_modulations(&mut self, _mode: rt::Mode) -> Result<Vec<rt::ModulationType>, rt::Error> {
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

fn make_rust_shim_device(handle: FakeRustHandle) -> *mut nfc_device {
    let conn = CString::new(handle.connstring.as_str()).unwrap();
    let raw = unsafe { nfc_device_new(ptr::null(), conn.as_ptr()) };
    assert!(!raw.is_null());
    let caps = handle.caps();
    let state = Box::new(RustDeviceState {
        handle: Box::new(handle),
        strerror: CString::new("shim").unwrap(),
        supported_modulations: Vec::new(),
        supported_baud_rates: Vec::new(),
    });
    let driver = Box::new(super::rust_owned::build_rust_device_shim_driver(caps));
    unsafe {
        (*raw).driver = Box::into_raw(driver);
        (*raw).driver_data = Box::into_raw(state).cast();
    }
    raw
}

#[test]
fn external_driver_grows_scan_capacity_until_result_is_not_saturated() {
    reset_raw_scan_state();
    with_raw_scan_state(|state| state.results = VecDeque::from([4, 8, 3]));
    let driver = ExternalDriver::new(ptr::addr_of!(SCAN_DRIVER));

    let listed = driver.scan(&rt::Context::default()).unwrap();

    assert_eq!(listed.len(), 3);
    assert_eq!(with_raw_scan_state(|state| state.capacities.clone()), vec![4, 8, 16]);
}

#[test]
fn external_driver_stops_scanning_at_max_capacity() {
    reset_raw_scan_state();
    with_raw_scan_state(|state| state.results = VecDeque::from([4, 8, 16, 32, 64, 128, 256]));
    let driver = ExternalDriver::new(ptr::addr_of!(SCAN_DRIVER));

    let listed = driver.scan(&rt::Context::default()).unwrap();

    assert_eq!(listed.len(), 256);
    assert_eq!(
        with_raw_scan_state(|state| state.capacities.clone()),
        vec![4, 8, 16, 32, 64, 128, 256]
    );
}

#[test]
fn external_driver_registry_keeps_usb_fallback_and_name_override_behavior() {
    reset_raw_scan_state();
    let mut registry = rt::DriverRegistry::new();
    registry.register_driver(Box::new(ExternalDriver::new(ptr::addr_of!(FALLBACK_USB_DRIVER))));
    registry.register_driver(Box::new(ExternalDriver::new(ptr::addr_of!(PRIMARY_USB_DRIVER))));

    let context = rt::Context::with_config(rt::ContextConfig {
        allow_autoscan: true,
        allow_intrusive_scan: false,
        log_level: 1,
        user_defined_devices: vec![rt::UserDefinedDevice {
            name: "friendly backend".into(),
            connstring: rt::ConnectionString::new("usb").unwrap(),
            optional: false,
        }],
    });

    let connstring = rt::ConnectionString::new("usb").unwrap();
    let device = registry.open(&context, Some(&connstring)).unwrap();

    assert_eq!(device.name(), "friendly backend");
    assert_eq!(
        with_raw_scan_state(|state| state.open_attempts.clone()),
        vec!["primary:usb".to_string(), "fallback:usb".to_string()]
    );
}

#[test]
fn borrowed_rust_device_normalizes_missing_caps_and_clears_last_error_on_success() {
    let property_calls = Arc::new(Mutex::new(Vec::new()));
    let raw = make_rust_shim_device(FakeRustHandle {
        name: "rust-shim".into(),
        connstring: rt::ConnectionString::new("alpha:001").unwrap(),
        caps: rt::DeviceCaps::SET_PROPERTY_BOOL,
        property_calls: property_calls.clone(),
        info_result: Err(rt::Error::UnsupportedOperation("device_get_information_about")),
    });

    let mut device = borrowed_device(raw);
    let error = device.information_about().unwrap_err();
    assert_eq!(error, rt::Error::MissingCapability("device_get_information_about"));
    assert_eq!(device.last_error(), NFC_EDEVNOTSUPP);

    device
        .set_property_bool(rt::Property::ActivateField, true)
        .unwrap();
    assert_eq!(device.last_error(), 0);
    assert_eq!(
        property_calls.lock().unwrap().clone(),
        vec![(rt::Property::ActivateField, true)]
    );

    unsafe {
        let driver = (*raw).driver as *mut nfc_driver;
        let close = (*driver).close.unwrap();
        close(raw);
    }
}
