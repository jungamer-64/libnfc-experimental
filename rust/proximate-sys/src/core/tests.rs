use super::context::nfc_exit;
use super::driver_registration::{bridge_close_device, nfc_register_driver};
use super::runtime::{nfc_list_devices, nfc_open};
use crate::c_boundary::NFC_BUFSIZE_CONNSTRING;
use crate::c_boundary::external_registry::{clear_registry, registry_snapshot};
use crate::c_boundary::raw::{c_string_ptr_to_string, fixed_c_buffer_to_string};
use crate::core::LOG_PRIORITY_INFO;
use crate::lifecycle::{
    DEVICE_NAME_LENGTH, NFC_DRIVER_NAME_MAX, nfc_connstring, nfc_context,
    nfc_context_alloc_defaults, nfc_context_new, nfc_device, nfc_device_free, nfc_driver,
    reset_lifecycle_test_state, scan_type_enum, snapshot_lifecycle_test_state,
};
use crate::{test_clear_last_log, test_get_last_log};
use libc::c_char;
use std::ffi::CString;
use std::ptr;
use std::sync::{Mutex, MutexGuard, OnceLock};

const NFC_SUCCESS: libc::c_int = 0;

#[derive(Clone, Default)]
struct FakeDriverState {
    open_calls: Vec<String>,
    scan_calls: Vec<String>,
    close_calls: Vec<String>,
    failing_connstrings: Vec<String>,
    scan_results: Vec<(String, Vec<String>)>,
}

thread_local! {
    static FAKE_DRIVER_STATE: std::cell::RefCell<FakeDriverState> =
        std::cell::RefCell::new(FakeDriverState::default());
}

static CORE_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn core_test_guard() -> MutexGuard<'static, ()> {
    CORE_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("core test mutex should not be poisoned")
}

fn reset_fake_driver_state() {
    FAKE_DRIVER_STATE.with(|cell| {
        *cell.borrow_mut() = FakeDriverState::default();
    });
}

fn with_fake_driver_state<R>(f: impl FnOnce(&mut FakeDriverState) -> R) -> R {
    FAKE_DRIVER_STATE.with(|cell| f(&mut cell.borrow_mut()))
}

fn set_scan_results(driver: &str, results: &[&str]) {
    with_fake_driver_state(|state| {
        state
            .scan_results
            .retain(|(existing, _)| existing != driver);
        state.scan_results.push((
            driver.to_string(),
            results.iter().map(|value| (*value).to_string()).collect(),
        ));
    });
}

fn add_failing_connstring(connstring: &str) {
    with_fake_driver_state(|state| {
        state.failing_connstrings.push(connstring.to_string());
    });
}

fn fake_driver_snapshot() -> FakeDriverState {
    FAKE_DRIVER_STATE.with(|cell| cell.borrow().clone())
}

unsafe fn allocate_fake_device(
    driver: *const nfc_driver,
    driver_name: &str,
    context: *const nfc_context,
    connstring: *const c_char,
) -> *mut nfc_device {
    let device = unsafe { crate::lifecycle::nfc_device_new(context, connstring) };
    if device.is_null() {
        return ptr::null_mut();
    }

    unsafe {
        (*device).driver = driver;
        super::driver_registration::write_bytes_to_char_buffer(
            (*device).name.as_mut_ptr(),
            DEVICE_NAME_LENGTH,
            format!("{}-device", driver_name).as_bytes(),
        );
    }

    device
}

unsafe fn open_named_driver(
    driver_name: &str,
    driver: *const nfc_driver,
    context: *const nfc_context,
    connstring: *const c_char,
) -> *mut nfc_device {
    let conn = c_string_ptr_to_string(connstring, NFC_BUFSIZE_CONNSTRING);
    with_fake_driver_state(|state| {
        state.open_calls.push(driver_name.to_string());
    });

    let should_fail = with_fake_driver_state(|state| {
        state.failing_connstrings.iter().any(|value| value == &conn)
    });
    if should_fail {
        return ptr::null_mut();
    }

    unsafe { allocate_fake_device(driver, driver_name, context, connstring) }
}

unsafe fn scan_named_driver(
    driver_name: &str,
    connstrings: *mut nfc_connstring,
    connstrings_len: usize,
) -> usize {
    with_fake_driver_state(|state| {
        state.scan_calls.push(driver_name.to_string());
    });

    let configured = with_fake_driver_state(|state| {
        state
            .scan_results
            .iter()
            .find(|(name, _)| name == driver_name)
            .map(|(_, results)| results.clone())
            .unwrap_or_default()
    });

    let mut copied = 0usize;
    for result in configured.iter().take(connstrings_len) {
        let c_result = CString::new(result.as_bytes()).unwrap();
        if unsafe {
            super::driver_registration::copy_connstring_safely(
                c_result.as_ptr(),
                connstrings.add(copied),
            )
        } {
            copied += 1;
        }
    }

    copied
}

unsafe extern "C" fn alpha_scan(
    _context: *const nfc_context,
    connstrings: *mut nfc_connstring,
    connstrings_len: usize,
) -> usize {
    unsafe { scan_named_driver("alpha", connstrings, connstrings_len) }
}

unsafe extern "C" fn alpha_open(
    context: *const nfc_context,
    connstring: *const c_char,
) -> *mut nfc_device {
    unsafe {
        open_named_driver(
            "alpha",
            ptr::addr_of!(TEST_DRIVER_ALPHA),
            context,
            connstring,
        )
    }
}

unsafe extern "C" fn alpha_close(device: *mut nfc_device) {
    with_fake_driver_state(|state| {
        state.close_calls.push("alpha".to_string());
    });
    unsafe { nfc_device_free(device) };
}

static TEST_DRIVER_ALPHA_NAME: &[u8] = b"alpha\0";
static TEST_DRIVER_ALPHA: nfc_driver = nfc_driver {
    name: TEST_DRIVER_ALPHA_NAME.as_ptr() as *const c_char,
    scan_type: scan_type_enum::NOT_INTRUSIVE,
    scan: Some(alpha_scan),
    open: Some(alpha_open),
    close: Some(alpha_close),
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

unsafe extern "C" fn beta_usb_open(
    context: *const nfc_context,
    connstring: *const c_char,
) -> *mut nfc_device {
    unsafe {
        open_named_driver(
            "beta_usb",
            ptr::addr_of!(TEST_DRIVER_BETA_USB),
            context,
            connstring,
        )
    }
}

unsafe extern "C" fn beta_usb_close(device: *mut nfc_device) {
    with_fake_driver_state(|state| {
        state.close_calls.push("beta_usb".to_string());
    });
    unsafe { nfc_device_free(device) };
}

static TEST_DRIVER_BETA_USB_NAME: &[u8] = b"beta_usb\0";
static TEST_DRIVER_BETA_USB: nfc_driver = nfc_driver {
    name: TEST_DRIVER_BETA_USB_NAME.as_ptr() as *const c_char,
    scan_type: scan_type_enum::NOT_INTRUSIVE,
    scan: None,
    open: Some(beta_usb_open),
    close: Some(beta_usb_close),
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

unsafe extern "C" fn gamma_usb_open(
    _context: *const nfc_context,
    _connstring: *const c_char,
) -> *mut nfc_device {
    with_fake_driver_state(|state| {
        state.open_calls.push("gamma_usb".to_string());
    });
    ptr::null_mut()
}

unsafe extern "C" fn gamma_usb_close(device: *mut nfc_device) {
    with_fake_driver_state(|state| {
        state.close_calls.push("gamma_usb".to_string());
    });
    unsafe { nfc_device_free(device) };
}

static TEST_DRIVER_GAMMA_USB_NAME: &[u8] = b"gamma_usb\0";
static TEST_DRIVER_GAMMA_USB: nfc_driver = nfc_driver {
    name: TEST_DRIVER_GAMMA_USB_NAME.as_ptr() as *const c_char,
    scan_type: scan_type_enum::NOT_INTRUSIVE,
    scan: None,
    open: Some(gamma_usb_open),
    close: Some(gamma_usb_close),
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

unsafe extern "C" fn intrusive_scan(
    _context: *const nfc_context,
    connstrings: *mut nfc_connstring,
    connstrings_len: usize,
) -> usize {
    unsafe { scan_named_driver("intrusive", connstrings, connstrings_len) }
}

unsafe extern "C" fn intrusive_open(
    context: *const nfc_context,
    connstring: *const c_char,
) -> *mut nfc_device {
    unsafe {
        open_named_driver(
            "intrusive",
            ptr::addr_of!(TEST_DRIVER_INTRUSIVE),
            context,
            connstring,
        )
    }
}

unsafe extern "C" fn intrusive_close(device: *mut nfc_device) {
    with_fake_driver_state(|state| {
        state.close_calls.push("intrusive".to_string());
    });
    unsafe { nfc_device_free(device) };
}

static TEST_DRIVER_INTRUSIVE_NAME: &[u8] = b"intrusive\0";
static TEST_DRIVER_INTRUSIVE: nfc_driver = nfc_driver {
    name: TEST_DRIVER_INTRUSIVE_NAME.as_ptr() as *const c_char,
    scan_type: scan_type_enum::INTRUSIVE,
    scan: Some(intrusive_scan),
    open: Some(intrusive_open),
    close: Some(intrusive_close),
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

fn registry_probe_order_names() -> Vec<String> {
    registry_snapshot()
        .iter()
        .rev()
        .map(|handle| {
            let driver = unsafe { &*handle.0 };
            c_string_ptr_to_string(driver.name, NFC_DRIVER_NAME_MAX)
        })
        .collect()
}

fn reset_core_test_world() {
    clear_registry();
    reset_fake_driver_state();
    super::driver_registration::reset_core_bridge_test_state();
    reset_lifecycle_test_state();
    crate::test_reset_log_level();
    test_clear_last_log();
}

#[test]
fn register_driver_preserves_existing_probe_order() {
    let _guard = core_test_guard();
    reset_core_test_world();

    unsafe {
        assert_eq!(
            nfc_register_driver(ptr::addr_of!(TEST_DRIVER_ALPHA)),
            NFC_SUCCESS
        );
        assert_eq!(
            nfc_register_driver(ptr::addr_of!(TEST_DRIVER_BETA_USB)),
            NFC_SUCCESS
        );
    }

    assert_eq!(
        registry_probe_order_names(),
        vec!["beta_usb".to_string(), "alpha".to_string()]
    );
}

#[test]
fn init_registers_builtins_only_once() {
    let _guard = core_test_guard();
    reset_core_test_world();

    let mut context = ptr::null_mut();

    unsafe {
        super::context::nfc_init_impl(&mut context);
        assert!(!context.is_null());
        super::context::nfc_init_impl(&mut context);
        nfc_exit(context);
    }

    assert_eq!(
        registry_probe_order_names(),
        Vec::<String>::new(),
        "nfc_exit should clear the registry after the second init"
    );
}

#[test]
fn init_skips_builtins_when_custom_driver_already_registered() {
    let _guard = core_test_guard();
    reset_core_test_world();

    let mut context = ptr::null_mut();

    unsafe {
        assert_eq!(
            nfc_register_driver(ptr::addr_of!(TEST_DRIVER_BETA_USB)),
            NFC_SUCCESS
        );
        super::context::nfc_init_impl(&mut context);
    }

    assert_eq!(registry_probe_order_names(), vec!["beta_usb".to_string()]);

    unsafe { nfc_exit(context) };
}

#[test]
fn exit_clears_registry_and_frees_context() {
    let _guard = core_test_guard();
    reset_core_test_world();

    let context = unsafe { nfc_context_new() };
    assert!(!context.is_null());

    unsafe {
        assert_eq!(
            nfc_register_driver(ptr::addr_of!(TEST_DRIVER_ALPHA)),
            NFC_SUCCESS
        );
        nfc_exit(context);
    }

    assert!(registry_snapshot().is_empty());
    assert_eq!(snapshot_lifecycle_test_state().context_free_calls, 1);
}

#[test]
fn open_matches_exact_driver_name_and_usb_suffix() {
    let _guard = core_test_guard();
    reset_core_test_world();

    unsafe {
        assert_eq!(
            nfc_register_driver(ptr::addr_of!(TEST_DRIVER_ALPHA)),
            NFC_SUCCESS
        );
        assert_eq!(
            nfc_register_driver(ptr::addr_of!(TEST_DRIVER_BETA_USB)),
            NFC_SUCCESS
        );
        assert_eq!(
            nfc_register_driver(ptr::addr_of!(TEST_DRIVER_GAMMA_USB)),
            NFC_SUCCESS
        );
    }

    let context = unsafe { nfc_context_new() };
    let exact = CString::new("alpha:port=1").unwrap();
    let usb = CString::new("usb").unwrap();

    let exact_device = unsafe { nfc_open(context, exact.as_ptr()) };
    assert!(!exact_device.is_null());
    unsafe { bridge_close_device(exact_device) };

    let usb_device = unsafe { nfc_open(context, usb.as_ptr()) };
    assert!(!usb_device.is_null());
    unsafe { bridge_close_device(usb_device) };

    let snapshot = fake_driver_snapshot();
    assert_eq!(
        snapshot.open_calls,
        vec![
            "alpha".to_string(),
            "gamma_usb".to_string(),
            "beta_usb".to_string()
        ]
    );

    unsafe { nfc_exit(context) };
}

#[test]
fn open_uses_list_devices_when_connstring_is_null() {
    let _guard = core_test_guard();
    reset_core_test_world();
    set_scan_results("alpha", &["alpha:port=1"]);

    unsafe {
        assert_eq!(
            nfc_register_driver(ptr::addr_of!(TEST_DRIVER_ALPHA)),
            NFC_SUCCESS
        );
    }

    let context = unsafe { nfc_context_alloc_defaults() };
    let device = unsafe { nfc_open(context, ptr::null()) };

    assert!(!device.is_null());
    assert_eq!(
        fixed_c_buffer_to_string(unsafe { &(*device).connstring }),
        "alpha:port=1".to_string()
    );

    unsafe {
        bridge_close_device(device);
        nfc_exit(context);
    }
}

#[test]
fn open_applies_user_defined_device_name() {
    let _guard = core_test_guard();
    reset_core_test_world();

    unsafe {
        assert_eq!(
            nfc_register_driver(ptr::addr_of!(TEST_DRIVER_ALPHA)),
            NFC_SUCCESS
        );
    }

    let context = unsafe { nfc_context_alloc_defaults() };
    let conn = CString::new("alpha").unwrap();
    unsafe {
        (*context).user_defined_device_count = 1;
        assert!(super::driver_registration::write_bytes_to_char_buffer(
            (*context).user_defined_devices[0].name.as_mut_ptr(),
            DEVICE_NAME_LENGTH,
            b"my-reader"
        ));
        assert!(super::driver_registration::copy_connstring_safely(
            conn.as_ptr(),
            &mut (*context).user_defined_devices[0].connstring
        ));
    }

    let device = unsafe { nfc_open(context, conn.as_ptr()) };
    assert!(!device.is_null());
    assert_eq!(
        fixed_c_buffer_to_string(unsafe { &(*device).name }),
        "my-reader".to_string()
    );

    unsafe {
        bridge_close_device(device);
        nfc_exit(context);
    }
}

#[test]
fn open_closes_device_when_name_override_copy_fails() {
    let _guard = core_test_guard();
    reset_core_test_world();

    unsafe {
        assert_eq!(
            nfc_register_driver(ptr::addr_of!(TEST_DRIVER_ALPHA)),
            NFC_SUCCESS
        );
    }

    let context = unsafe { nfc_context_alloc_defaults() };
    let conn = CString::new("alpha").unwrap();
    unsafe {
        (*context).user_defined_device_count = 1;
        for byte in (*context).user_defined_devices[0].name.iter_mut() {
            *byte = b'A' as c_char;
        }
        assert!(super::driver_registration::copy_connstring_safely(
            conn.as_ptr(),
            &mut (*context).user_defined_devices[0].connstring
        ));
    }

    let device = unsafe { nfc_open(context, conn.as_ptr()) };
    assert!(device.is_null());
    assert_eq!(
        super::driver_registration::snapshot_core_bridge_test_state().close_calls,
        1
    );

    unsafe { nfc_exit(context) };
}

#[test]
fn list_devices_skips_unavailable_optional_entries_without_touching_log_env() {
    let _guard = core_test_guard();
    reset_core_test_world();
    add_failing_connstring("alpha:optional");

    unsafe {
        assert_eq!(
            nfc_register_driver(ptr::addr_of!(TEST_DRIVER_ALPHA)),
            NFC_SUCCESS
        );
    }

    let context = unsafe { nfc_context_alloc_defaults() };
    let optional = CString::new("alpha:optional").unwrap();
    unsafe {
        (*context).user_defined_device_count = 1;
        (*context).allow_autoscan = false;
        assert!(super::driver_registration::write_bytes_to_char_buffer(
            (*context).user_defined_devices[0].name.as_mut_ptr(),
            DEVICE_NAME_LENGTH,
            b"optional-reader"
        ));
        assert!(super::driver_registration::copy_connstring_safely(
            optional.as_ptr(),
            &mut (*context).user_defined_devices[0].connstring
        ));
        (*context).user_defined_devices[0].optional = true;
    }

    unsafe { std::env::remove_var("LIBNFC_LOG_LEVEL") };

    let mut connstrings = [[0 as c_char; NFC_BUFSIZE_CONNSTRING]; 2];
    let found = unsafe { nfc_list_devices(context, connstrings.as_mut_ptr(), connstrings.len()) };
    assert_eq!(found, 0);

    let restored = unsafe { libc::getenv(c"LIBNFC_LOG_LEVEL".as_ptr()) };
    assert!(restored.is_null());

    unsafe {
        nfc_exit(context);
    }
}

#[test]
fn list_devices_warns_when_autoscan_is_disabled_without_manual_devices() {
    let _guard = core_test_guard();
    reset_core_test_world();
    crate::logger::log_init(LOG_PRIORITY_INFO.into());

    let context = unsafe { nfc_context_alloc_defaults() };
    unsafe {
        (*context).allow_autoscan = false;
    }

    let mut connstrings = [[0 as c_char; NFC_BUFSIZE_CONNSTRING]; 1];
    let found = unsafe { nfc_list_devices(context, connstrings.as_mut_ptr(), connstrings.len()) };
    assert_eq!(found, 0);
    assert_eq!(
        test_get_last_log(),
        Some("Warning: user must specify device(s) manually when autoscan is disabled".to_string())
    );

    unsafe { nfc_exit(context) };
}

#[test]
fn list_devices_respects_intrusive_scan_flag() {
    let _guard = core_test_guard();
    reset_core_test_world();
    set_scan_results("intrusive", &["intrusive:device"]);

    unsafe {
        assert_eq!(
            nfc_register_driver(ptr::addr_of!(TEST_DRIVER_INTRUSIVE)),
            NFC_SUCCESS
        );
    }

    let context = unsafe { nfc_context_alloc_defaults() };
    let mut connstrings = [[0 as c_char; NFC_BUFSIZE_CONNSTRING]; 1];

    let without_intrusive =
        unsafe { nfc_list_devices(context, connstrings.as_mut_ptr(), connstrings.len()) };
    assert_eq!(without_intrusive, 0);

    unsafe {
        (*context).allow_intrusive_scan = true;
    }
    let with_intrusive =
        unsafe { nfc_list_devices(context, connstrings.as_mut_ptr(), connstrings.len()) };
    assert_eq!(with_intrusive, 1);
    assert_eq!(
        fixed_c_buffer_to_string(&connstrings[0]),
        "intrusive:device".to_string()
    );

    unsafe { nfc_exit(context) };
}
