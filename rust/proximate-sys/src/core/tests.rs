use super::context::{nfc_exit, nfc_init};
use super::driver_registration::nfc_register_driver;
use super::runtime::{nfc_list_devices, nfc_open};
use crate::c_boundary::external_registry::registry_snapshot;
use crate::c_boundary::raw::fixed_c_buffer_to_string;
use crate::c_boundary::status::NFC_EINVARG;
use crate::initiator::accessors::{nfc_device_get_connstring, nfc_device_get_name};
use crate::lifecycle::{nfc_connstring, nfc_context, scan_type_enum};
use crate::test_support::{
    external_driver, external_state_snapshot, external_test_guard, reset_external_state,
};
use std::ffi::{CStr, CString};
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

static SCAN_CAPACITIES: Mutex<Vec<usize>> = Mutex::new(Vec::new());
static SCAN_FINAL_CAPACITY: AtomicUsize = AtomicUsize::new(16);

unsafe extern "C" fn saturated_external_scan(
    _context: *const nfc_context,
    connstrings: *mut nfc_connstring,
    capacity: usize,
) -> usize {
    SCAN_CAPACITIES.lock().unwrap().push(capacity);
    if capacity < SCAN_FINAL_CAPACITY.load(Ordering::Relaxed) {
        return capacity;
    }
    if !connstrings.is_null() {
        unsafe {
            crate::c_boundary::raw::copy_bytes_to_c_buffer(
                (*connstrings).as_mut_ptr(),
                crate::NFC_BUFSIZE_CONNSTRING,
                b"fakec:one",
            );
        }
    }
    1
}

fn initialized_context() -> *mut nfc_context {
    let mut context = ptr::null_mut();
    unsafe { nfc_init(ptr::addr_of_mut!(context)) };
    assert!(!context.is_null());
    context
}

fn register_fake() {
    let driver = external_driver(c"fakec");
    assert_eq!(unsafe { nfc_register_driver(ptr::addr_of!(driver)) }, 0);
}

#[test]
fn null_driver_registration_is_rejected() {
    let _guard = external_test_guard();
    reset_external_state();
    assert_eq!(unsafe { nfc_register_driver(ptr::null()) }, NFC_EINVARG);
}

#[test]
fn registered_external_driver_scans_through_c_entrypoint() {
    let _guard = external_test_guard();
    reset_external_state();
    SCAN_CAPACITIES.lock().unwrap().clear();
    SCAN_FINAL_CAPACITY.store(16, Ordering::Relaxed);
    let mut driver = external_driver(c"fakec");
    driver.scan = Some(saturated_external_scan);
    assert_eq!(unsafe { nfc_register_driver(ptr::addr_of!(driver)) }, 0);
    let context = initialized_context();
    let mut connstrings = [[0; crate::NFC_BUFSIZE_CONNSTRING]; 4];
    let count = unsafe { nfc_list_devices(context, connstrings.as_mut_ptr(), connstrings.len()) };
    assert!(count >= 1);
    assert_eq!(fixed_c_buffer_to_string(&connstrings[0]), "fakec:one");
    assert_eq!(*SCAN_CAPACITIES.lock().unwrap(), [4, 8, 16]);
    unsafe { nfc_exit(context) };

    reset_external_state();
    SCAN_CAPACITIES.lock().unwrap().clear();
    SCAN_FINAL_CAPACITY.store(usize::MAX, Ordering::Relaxed);
    let mut driver = external_driver(c"fakec");
    driver.scan = Some(saturated_external_scan);
    assert_eq!(unsafe { nfc_register_driver(ptr::addr_of!(driver)) }, 0);
    let context = initialized_context();
    assert_eq!(
        unsafe { nfc_list_devices(context, connstrings.as_mut_ptr(), connstrings.len()) },
        0
    );
    assert_eq!(
        *SCAN_CAPACITIES.lock().unwrap(),
        [4, 8, 16, 32, 64, 128, 256]
    );
    unsafe { nfc_exit(context) };
}

#[test]
fn registered_external_driver_opens_through_c_entrypoint() {
    let _guard = external_test_guard();
    reset_external_state();
    register_fake();
    let context = initialized_context();
    let connstring = CString::new("fakec:one").unwrap();
    let device = unsafe { nfc_open(context, connstring.as_ptr()) };
    assert!(!device.is_null());
    assert_eq!(
        unsafe { CStr::from_ptr(nfc_device_get_name(device)) },
        c"external fake"
    );
    assert_eq!(
        unsafe { CStr::from_ptr(nfc_device_get_connstring(device)) },
        c"fakec:one"
    );
    unsafe {
        crate::c_abi::misc_exports::nfc_close(device);
        nfc_exit(context);
    }
}

#[test]
fn registration_snapshots_callback_table() {
    let _guard = external_test_guard();
    reset_external_state();
    let mut driver = external_driver(c"fakec");
    assert_eq!(unsafe { nfc_register_driver(ptr::addr_of!(driver)) }, 0);
    driver.open = None;
    driver.scan = None;
    assert!(driver.open.is_none());
    assert!(driver.scan.is_none());
    let context = initialized_context();
    let connstring = CString::new("fakec:one").unwrap();
    let device = unsafe { nfc_open(context, connstring.as_ptr()) };
    assert!(!device.is_null());
    unsafe {
        crate::c_abi::misc_exports::nfc_close(device);
        nfc_exit(context);
    }
}

#[test]
fn external_close_runs_exactly_once() {
    let _guard = external_test_guard();
    reset_external_state();
    register_fake();
    let context = initialized_context();
    let connstring = CString::new("fakec:one").unwrap();
    let device = unsafe { nfc_open(context, connstring.as_ptr()) };
    unsafe { crate::c_abi::misc_exports::nfc_close(device) };
    assert_eq!(external_state_snapshot().closes, 1);
    unsafe { nfc_exit(context) };
}

#[test]
fn exit_clears_external_registry() {
    let _guard = external_test_guard();
    reset_external_state();
    register_fake();
    assert_eq!(registry_snapshot().len(), 1);
    unsafe { nfc_exit(ptr::null_mut()) };
    assert!(registry_snapshot().is_empty());
}

#[test]
fn missing_scan_callback_is_an_operational_scan_failure() {
    let _guard = external_test_guard();
    reset_external_state();
    let mut driver = external_driver(c"fakec");
    driver.scan = None;
    assert_eq!(unsafe { nfc_register_driver(ptr::addr_of!(driver)) }, 0);
    let context = initialized_context();
    let mut connstrings: [nfc_connstring; 1] = [[0; crate::NFC_BUFSIZE_CONNSTRING]; 1];
    assert_eq!(
        unsafe { nfc_list_devices(context, connstrings.as_mut_ptr(), 1) },
        0
    );
    unsafe { nfc_exit(context) };
}

#[test]
fn missing_open_callback_returns_null() {
    let _guard = external_test_guard();
    reset_external_state();
    let mut driver = external_driver(c"fakec");
    driver.open = None;
    assert_eq!(unsafe { nfc_register_driver(ptr::addr_of!(driver)) }, 0);
    let context = initialized_context();
    let connstring = CString::new("fakec:one").unwrap();
    assert!(unsafe { nfc_open(context, connstring.as_ptr()) }.is_null());
    unsafe { nfc_exit(context) };
}

#[test]
fn invalid_scan_discriminant_is_rejected_without_ub() {
    let _guard = external_test_guard();
    reset_external_state();
    let mut driver = external_driver(c"fakec");
    driver.scan_type = scan_type_enum::from_raw(99);
    assert_eq!(
        unsafe { nfc_register_driver(ptr::addr_of!(driver)) },
        NFC_EINVARG
    );
}

#[test]
fn zero_length_list_does_not_require_an_output_pointer() {
    let _guard = external_test_guard();
    reset_external_state();
    register_fake();
    let context = initialized_context();
    assert_eq!(unsafe { nfc_list_devices(context, ptr::null_mut(), 0) }, 0);
    unsafe { nfc_exit(context) };
}

#[test]
fn nfc_init_accepts_null_output_pointer() {
    let _guard = external_test_guard();
    reset_external_state();
    unsafe { nfc_init(ptr::null_mut()) };
}

#[test]
fn registration_order_is_preserved_in_registry_snapshot() {
    let _guard = external_test_guard();
    reset_external_state();
    let first = external_driver(c"first");
    let second = external_driver(c"second");
    assert_eq!(unsafe { nfc_register_driver(ptr::addr_of!(first)) }, 0);
    assert_eq!(unsafe { nfc_register_driver(ptr::addr_of!(second)) }, 0);
    let snapshot = registry_snapshot();
    assert_eq!(snapshot[0].name(), "first");
    assert_eq!(snapshot[1].name(), "second");
}
