use super::alloc::{
    nfc_context_alloc_defaults, nfc_context_free, nfc_context_new, nfc_device_free, nfc_device_new,
};
use super::logging::{reset_lifecycle_test_state, snapshot_lifecycle_test_state};
use super::*;
use crate::{ffi_catch_unwind_ptr, ffi_catch_unwind_void, release_allocated_ptr, reset_last_error};
use libc::c_void;
use proximate_driver::set_test_conf_root;
use std::ffi::{CStr, CString, OsString};
use std::fs;
use std::path::PathBuf;
use std::process;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

const DEFAULT_CONTEXT_LOG_LEVEL: u32 = if cfg!(libnfc_debug) { 3 } else { 1 };
const USER_DEFINED_DEFAULT_DEVICE_NAME: &[u8] = b"user defined default device";
const USER_DEFINED_DEVICE_NAME: &[u8] = b"user defined device";

fn release_context(context: *mut nfc_context) {
    unsafe { release_allocated_ptr(context as *mut c_void) };
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

    fn install(&self) {
        set_test_conf_root(Some(self.root.clone()));
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

fn reset_test_world() {
    reset_lifecycle_test_state();
    set_test_conf_root(None);
    crate::test_reset_log_level();
    crate::test_clear_last_log();
}

#[test]
fn context_alloc_defaults_matches_c_defaults() {
    let context = unsafe { nfc_context_alloc_defaults() };
    assert!(!context.is_null());

    unsafe {
        assert!((*context).allow_autoscan);
        assert!(!(*context).allow_intrusive_scan);
        assert_eq!((*context).log_level, DEFAULT_CONTEXT_LOG_LEVEL);
        assert_eq!((*context).user_defined_device_count, 0);
        assert_eq!((*context).user_defined_devices[0].name[0], 0);
        assert_eq!((*context).user_defined_devices[0].connstring[0], 0);
        assert!(!(*context).user_defined_devices[0].optional);
    }

    release_context(context);
}

#[test]
fn context_new_applies_defaults_and_initializes_logging() {
    let _env_guard = env_lock().lock().unwrap();
    let mut env = ScopedEnv::new();
    clear_env(&mut env);
    reset_test_world();

    let context = unsafe { nfc_context_new() };
    assert!(!context.is_null());

    unsafe {
        assert!((*context).allow_autoscan);
        assert!(!(*context).allow_intrusive_scan);
        assert_eq!((*context).log_level, DEFAULT_CONTEXT_LOG_LEVEL);
        assert_eq!((*context).user_defined_device_count, 0);
    }

    let bridge_state = snapshot_lifecycle_test_state();
    assert_eq!(bridge_state.log_init_calls, 1);
    assert_eq!(bridge_state.log_exit_calls, 0);
    assert_eq!(bridge_state.events, vec!["log_init"]);
    if DEFAULT_CONTEXT_LOG_LEVEL >= crate::c_api_impl::LOG_PRIORITY_DEBUG.into() {
        assert_eq!(
            crate::test_get_last_log().as_deref(),
            Some("0 device(s) defined by user")
        );
    } else {
        assert_eq!(crate::test_get_last_log(), None);
    }

    release_context(context);
}

#[test]
fn context_new_reflects_libnfc_default_device() {
    let _env_guard = env_lock().lock().unwrap();
    let mut env = ScopedEnv::new();
    clear_env(&mut env);
    env.set("LIBNFC_DEFAULT_DEVICE", "pn532_uart:/dev/ttyUSB0");
    reset_test_world();

    let context = unsafe { nfc_context_new() };
    assert!(!context.is_null());

    unsafe {
        assert_eq!((*context).user_defined_device_count, 1);
        let device = &(*context).user_defined_devices[0];
        assert_eq!(
            CStr::from_ptr(device.name.as_ptr()).to_bytes(),
            USER_DEFINED_DEFAULT_DEVICE_NAME
        );
        assert_eq!(
            CStr::from_ptr(device.connstring.as_ptr()).to_bytes(),
            b"pn532_uart:/dev/ttyUSB0"
        );
    }

    release_context(context);
}

#[test]
fn context_new_loads_config_files_and_devices_d_entries() {
    let _env_guard = env_lock().lock().unwrap();
    let mut env = ScopedEnv::new();
    clear_env(&mut env);
    reset_test_world();
    crate::logger::log_init(3);

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
    confdir.install();

    let context = unsafe { nfc_context_new() };
    assert!(!context.is_null());

    unsafe {
        assert!(!(*context).allow_autoscan);
        assert!((*context).allow_intrusive_scan);
        assert_eq!((*context).log_level, 7);
        assert_eq!((*context).user_defined_device_count, 2);

        let first = &(*context).user_defined_devices[0];
        assert_eq!(
            CStr::from_ptr(first.name.as_ptr()).to_bytes(),
            b"config device"
        );
        assert_eq!(
            CStr::from_ptr(first.connstring.as_ptr()).to_bytes(),
            b"pn532_spi:/dev/spidev0.0"
        );
        assert!(first.optional);

        let second = &(*context).user_defined_devices[1];
        assert_eq!(
            CStr::from_ptr(second.name.as_ptr()).to_bytes(),
            b"extra device"
        );
        assert_eq!(
            CStr::from_ptr(second.connstring.as_ptr()).to_bytes(),
            b"pn532_i2c:/dev/i2c-1"
        );
        assert!(second.optional);
    }

    let logs = crate::test_get_logs();
    assert!(
        logs.iter()
            .any(|entry| entry.contains("allow_autoscan is set to false")),
        "captured logs: {:?}",
        logs
    );

    release_context(context);
}

#[test]
fn context_new_logs_parse_errors_and_caps_device_count() {
    let _env_guard = env_lock().lock().unwrap();
    let mut env = ScopedEnv::new();
    clear_env(&mut env);
    reset_test_world();
    crate::logger::log_init(3);

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
    confdir.install();

    let context = unsafe { nfc_context_new() };
    assert!(!context.is_null());

    unsafe {
        assert_eq!(
            (*context).user_defined_device_count as usize,
            MAX_USER_DEFINED_DEVICES
        );
    }

    let logs = crate::test_get_logs();
    assert!(
        logs.iter()
            .any(|entry| entry.contains("Unknown key in config line: unknown.key = value")),
        "captured logs: {:?}",
        logs
    );
    assert!(
        logs.iter()
            .any(|entry| entry.contains("Parse error on line #2: broken line")),
        "captured logs: {:?}",
        logs
    );
    assert!(
        logs.iter()
            .any(|entry| entry.contains("Configuration exceeded maximum user-defined devices."))
    );

    release_context(context);
}

#[test]
fn context_new_libnfc_device_overrides_config_and_default_device() {
    let _env_guard = env_lock().lock().unwrap();
    let mut env = ScopedEnv::new();
    clear_env(&mut env);
    env.set("LIBNFC_DEFAULT_DEVICE", "pn532_uart:/dev/ttyUSB0");
    env.set("LIBNFC_DEVICE", "pn53x_usb:001:002");
    reset_test_world();

    let confdir = TempConfigDir::new();
    confdir.write_file(
        "libnfc.conf",
        concat!(
            "device.name = \"config device\"\n",
            "device.connstring = pn532_spi:/dev/spidev0.0\n",
            "device.optional = true\n"
        ),
    );
    confdir.install();

    let context = unsafe { nfc_context_new() };
    assert!(!context.is_null());

    unsafe {
        assert_eq!((*context).user_defined_device_count, 1);
        let device = &(*context).user_defined_devices[0];
        assert_eq!(
            CStr::from_ptr(device.name.as_ptr()).to_bytes(),
            USER_DEFINED_DEVICE_NAME
        );
        assert_eq!(
            CStr::from_ptr(device.connstring.as_ptr()).to_bytes(),
            b"pn53x_usb:001:002"
        );
    }

    release_context(context);
}

#[test]
fn context_new_applies_env_boolean_and_log_level_overrides() {
    let _env_guard = env_lock().lock().unwrap();
    let mut env = ScopedEnv::new();
    clear_env(&mut env);
    env.set("LIBNFC_AUTO_SCAN", "false");
    env.set("LIBNFC_INTRUSIVE_SCAN", "true");
    env.set("LIBNFC_LOG_LEVEL", "42");
    reset_test_world();

    let confdir = TempConfigDir::new();
    confdir.write_file(
        "libnfc.conf",
        concat!(
            "allow_autoscan = true\n",
            "allow_intrusive_scan = false\n",
            "log_level = 7\n"
        ),
    );
    confdir.install();

    let context = unsafe { nfc_context_new() };
    assert!(!context.is_null());

    unsafe {
        assert!(!(*context).allow_autoscan);
        assert!((*context).allow_intrusive_scan);
        assert_eq!((*context).log_level, 42);
    }

    release_context(context);
}

#[test]
fn context_new_keeps_lowercase_only_boolean_semantics() {
    let _env_guard = env_lock().lock().unwrap();
    let mut env = ScopedEnv::new();
    clear_env(&mut env);
    env.set("LIBNFC_INTRUSIVE_SCAN", "True");
    reset_test_world();

    let context = unsafe { nfc_context_new() };
    assert!(!context.is_null());

    unsafe {
        assert!(!(*context).allow_intrusive_scan);
    }

    release_context(context);
}

#[test]
fn context_free_calls_log_exit_and_accepts_null() {
    let _env_guard = env_lock().lock().unwrap();
    let mut env = ScopedEnv::new();
    clear_env(&mut env);
    reset_test_world();

    let confdir = TempConfigDir::new();
    confdir.write_file("libnfc.conf", "");
    confdir.install();

    let context = unsafe { nfc_context_new() };
    assert!(!context.is_null());

    unsafe {
        nfc_context_free(context);
        nfc_context_free(ptr::null_mut());
    }

    let state = snapshot_lifecycle_test_state();
    assert_eq!(state.context_free_calls, 2);
    assert_eq!(state.log_exit_calls, 2);
    assert_eq!(state.events, vec!["log_init", "log_exit", "log_exit"]);
}

#[test]
fn device_new_initializes_expected_fields() {
    let connstring = CString::new("pn53x_usb:/dev/usb").unwrap();
    let device = unsafe { nfc_device_new(ptr::null(), connstring.as_ptr()) };
    assert!(!device.is_null());

    unsafe {
        assert!((*device).context.is_null());
        assert!((*device).driver.is_null());
        assert!((*device).driver_data.is_null());
        assert!((*device).chip_data.is_null());
        assert_eq!((*device).name[0], 0);
        assert_eq!(
            CStr::from_ptr((*device).connstring.as_ptr()).to_bytes(),
            connstring.as_bytes()
        );
        assert!(!(*device).bCrc);
        assert!(!(*device).bPar);
        assert!(!(*device).bEasyFraming);
        assert!(!(*device).bInfiniteSelect);
        assert!(!(*device).bAutoIso14443_4);
        assert_eq!((*device).btSupportByte, 0);
        assert_eq!((*device).last_error, 0);

        (*device).driver_data = libc::malloc(8);
        nfc_device_free(device);
    }
}

#[test]
fn device_new_rejects_null_connstring() {
    reset_last_error();
    let device = unsafe { nfc_device_new(ptr::null(), ptr::null()) };
    assert!(device.is_null());

    let err = crate::c_api_impl::nfc_get_last_error();
    assert!(!err.is_null());
    let recovered = unsafe { CStr::from_ptr(err) }.to_str().unwrap();
    assert!(recovered.contains("NULL connstring in nfc_device_new"));
}

#[test]
#[cfg_attr(feature = "test_no_catch", should_panic(expected = "boom"))]
fn lifecycle_pointer_panic_is_normalized_to_null() {
    reset_last_error();
    let _ptr = ffi_catch_unwind_ptr::<nfc_context, _>("lifecycle_ptr_panic", || panic!("boom"));
    #[cfg(not(feature = "test_no_catch"))]
    {
        assert!(_ptr.is_null());

        let err = crate::c_api_impl::nfc_get_last_error();
        assert!(!err.is_null());
        let recovered = unsafe { CStr::from_ptr(err) }.to_str().unwrap();
        assert!(recovered.contains("panic in lifecycle_ptr_panic"));
    }
}

#[test]
#[cfg_attr(feature = "test_no_catch", should_panic(expected = "boom"))]
fn lifecycle_void_panic_is_normalized_to_noop() {
    reset_last_error();
    ffi_catch_unwind_void("lifecycle_void_panic", || panic!("boom"));

    #[cfg(not(feature = "test_no_catch"))]
    {
        let err = crate::c_api_impl::nfc_get_last_error();
        assert!(!err.is_null());
        let recovered = unsafe { CStr::from_ptr(err) }.to_str().unwrap();
        assert!(recovered.contains("panic in lifecycle_void_panic"));
    }
}
