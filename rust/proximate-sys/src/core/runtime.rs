use super::{log_general_debug, log_general_error, log_general_info};
use crate::bridge::decode::{context_from_c, decode_connstring_ptr};
use crate::bridge::driver_shim::attach_rust_device;
use crate::bridge::encode::ConnstringsOut;
use crate::bridge::external_registry::register_external_drivers;
use crate::ffi_catch_unwind_ptr;
use crate::lifecycle::{nfc_connstring, nfc_context, nfc_device};
use libc::{c_char, size_t};
use proximate_driver as rt;
use std::ptr;

fn register_compiled_bridge_drivers(_registry: &mut rt::DriverRegistry) {}

fn create_runtime_registry() -> rt::DriverRegistry {
    let mut registry = rt::DriverRegistry::new();
    proximate_native::register_builtin_drivers(&mut registry);
    register_compiled_bridge_drivers(&mut registry);
    register_external_drivers(&mut registry);
    registry
}

unsafe fn nfc_open_impl(context: *mut nfc_context, connstring: *const c_char) -> *mut nfc_device {
    let runtime_context = context_from_c(context.cast_const());
    let registry = create_runtime_registry();
    let requested: Option<rt::ConnectionString> = match decode_connstring_ptr(connstring) {
        Ok(connstring) => connstring,
        Err(_) => return ptr::null_mut(),
    };

    match registry.open(&runtime_context, requested.as_ref()) {
        Ok(device) => attach_rust_device(device, context.cast_const()).unwrap_or(ptr::null_mut()),
        Err(error) => {
            log_general_debug(&format!("nfc_open failed: {:?}", error));
            ptr::null_mut()
        }
    }
}

unsafe fn nfc_list_devices_impl(
    context: *mut nfc_context,
    connstrings: *mut nfc_connstring,
    connstrings_len: usize,
) -> usize {
    let Some(output): Option<ConnstringsOut> =
        (unsafe { ConnstringsOut::from_raw(connstrings, connstrings_len) })
    else {
        return 0;
    };
    let runtime_context = context_from_c(context.cast_const());
    let registry = create_runtime_registry();
    let Ok(outcome) = registry.list_devices_outcome(&runtime_context) else {
        return 0;
    };
    if outcome.warn_manual_selection {
        log_general_info("Warning: user must specify device(s) manually when autoscan is disabled");
    }

    output.write_back(outcome.devices)
}

fn ffi_catch_unwind_size_t<F>(context: &str, operation: F) -> size_t
where
    F: FnOnce() -> size_t,
    F: std::panic::UnwindSafe,
{
    #[cfg(not(feature = "test_no_catch"))]
    {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)) {
            Ok(result) => result,
            Err(_) => {
                log_general_error(&format!("panic in {}", context));
                0
            }
        }
    }

    #[cfg(feature = "test_no_catch")]
    {
        let _ = context;
        operation()
    }
}

pub(crate) unsafe fn nfc_open(
    context: *mut nfc_context,
    connstring: *const c_char,
) -> *mut nfc_device {
    ffi_catch_unwind_ptr("nfc_open", || unsafe { nfc_open_impl(context, connstring) })
}

pub(crate) unsafe fn nfc_list_devices(
    context: *mut nfc_context,
    connstrings: *mut nfc_connstring,
    connstrings_len: size_t,
) -> size_t {
    ffi_catch_unwind_size_t("nfc_list_devices", || unsafe {
        nfc_list_devices_impl(context, connstrings, connstrings_len)
    })
}
