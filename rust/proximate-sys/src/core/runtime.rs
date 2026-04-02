use super::*;
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
    let requested = if connstring.is_null() {
        None
    } else {
        let value = c_string_ptr_to_string(
            connstring,
            bounded_strlen(connstring, NFC_BUFSIZE_CONNSTRING),
        );
        match rt::ConnectionString::new(value) {
            Ok(connstring) => Some(connstring),
            Err(_) => return ptr::null_mut(),
        }
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
    if connstrings.is_null() || connstrings_len == 0 {
        return 0;
    }
    let runtime_context = context_from_c(context.cast_const());
    let registry = create_runtime_registry();
    let Ok(outcome) = registry.list_devices_outcome(&runtime_context) else {
        return 0;
    };
    if outcome.warn_manual_selection {
        log_general_info("Warning: user must specify device(s) manually when autoscan is disabled");
    }

    let mut written = 0usize;
    for connstring in outcome.devices.into_iter().take(connstrings_len) {
        let value = connstring.as_str();
        let destination = unsafe { connstrings.add(written) };
        if unsafe {
            copy_bytes_to_c_buffer(destination.cast(), NFC_BUFSIZE_CONNSTRING, value.as_bytes())
        } {
            written += 1;
        }
    }
    written
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

pub unsafe fn nfc_open(context: *mut nfc_context, connstring: *const c_char) -> *mut nfc_device {
    ffi_catch_unwind_ptr("nfc_open", || unsafe { nfc_open_impl(context, connstring) })
}

pub unsafe fn nfc_list_devices(
    context: *mut nfc_context,
    connstrings: *mut nfc_connstring,
    connstrings_len: size_t,
) -> size_t {
    ffi_catch_unwind_size_t("nfc_list_devices", || unsafe {
        nfc_list_devices_impl(context, connstrings, connstrings_len)
    })
}
