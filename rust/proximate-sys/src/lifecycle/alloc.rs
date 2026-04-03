use super::abi::{nfc_context, nfc_device};
use super::logging;
use crate::bridge::encode::write_context_to_c;
use crate::c_api_impl::NFC_BUFSIZE_CONNSTRING;
use crate::ffi_support::{as_mut, as_ref, copy_c_string_to_c_buffer};
use crate::{
    ffi_catch_unwind_ptr, ffi_catch_unwind_void, log_error, release_allocated_ptr,
    reset_last_error, set_last_error_message,
};
use libc::{c_char, c_void};
use proximate_driver as rt;
use std::mem::size_of;
use std::ptr;

unsafe fn allocate_zeroed<T>(label: &str) -> *mut T {
    let ptr = unsafe { libc::calloc(1, size_of::<T>()) as *mut T };
    if ptr.is_null() {
        let message = format!("Unable to allocate {}", label);
        log_error(&message);
        set_last_error_message(message);
    }
    ptr
}

unsafe fn nfc_context_alloc_defaults_impl() -> *mut nfc_context {
    let context = unsafe { allocate_zeroed::<nfc_context>("nfc_context") };
    if context.is_null() {
        return ptr::null_mut();
    }
    let runtime = rt::Context::default();
    write_context_to_c(&runtime, context);
    unsafe { set_runtime_context(context, runtime) };

    reset_last_error();
    context
}

unsafe fn nfc_device_new_impl(
    context: *const nfc_context,
    connstring: *const c_char,
) -> *mut nfc_device {
    if connstring.is_null() {
        let message = "NULL connstring in nfc_device_new".to_string();
        log_error(&message);
        set_last_error_message(message);
        return ptr::null_mut();
    }

    let device = unsafe { allocate_zeroed::<nfc_device>("nfc_device") };
    if device.is_null() {
        return ptr::null_mut();
    }

    let Some(device_ref) = (unsafe { as_mut(device) }) else {
        return ptr::null_mut();
    };
    device_ref.context = context;
    let _ = unsafe {
        copy_c_string_to_c_buffer(
            device_ref.connstring.as_mut_ptr(),
            NFC_BUFSIZE_CONNSTRING,
            connstring,
        )
    };

    reset_last_error();
    device
}

pub(crate) unsafe fn set_runtime_context(context: *mut nfc_context, runtime: rt::Context) {
    let Some(context_ref) = (unsafe { as_mut(context) }) else {
        return;
    };

    if !context_ref.runtime_data.is_null() {
        unsafe { drop(Box::from_raw(context_ref.runtime_data as *mut rt::Context)) };
    }

    context_ref.runtime_data = Box::into_raw(Box::new(runtime)).cast();
}

pub(crate) unsafe fn runtime_context_from_c(context: *const nfc_context) -> Option<rt::Context> {
    let context_ref = unsafe { as_ref(context) }?;
    let runtime = context_ref.runtime_data as *const rt::Context;
    if runtime.is_null() {
        None
    } else {
        Some(unsafe { (*runtime).clone() })
    }
}

unsafe fn free_context_allocation(context: *mut nfc_context) {
    if let Some(context_ref) = unsafe { as_mut(context) }
        && !context_ref.runtime_data.is_null()
    {
        unsafe { drop(Box::from_raw(context_ref.runtime_data as *mut rt::Context)) };
        context_ref.runtime_data = ptr::null_mut();
    }
    unsafe { release_allocated_ptr(context as *mut c_void) };
}

unsafe fn nfc_context_new_impl() -> *mut nfc_context {
    let Ok(loaded) = logging::load_context_outcome() else {
        return ptr::null_mut();
    };

    let context = unsafe { nfc_context_alloc_defaults_impl() };
    if context.is_null() {
        return ptr::null_mut();
    }

    write_context_to_c(&loaded.context, context);
    unsafe { set_runtime_context(context, loaded.context.clone()) };
    unsafe { logging::initialize_loaded_context_logging(context) };

    reset_last_error();
    context
}

#[cfg(test)]
pub(crate) unsafe fn nfc_context_alloc_defaults() -> *mut nfc_context {
    ffi_catch_unwind_ptr("nfc_context_alloc_defaults", || unsafe {
        nfc_context_alloc_defaults_impl()
    })
}

pub(crate) unsafe fn nfc_context_new() -> *mut nfc_context {
    ffi_catch_unwind_ptr("nfc_context_new", || unsafe { nfc_context_new_impl() })
}

pub(crate) unsafe fn nfc_device_new(
    context: *const nfc_context,
    connstring: *const c_char,
) -> *mut nfc_device {
    ffi_catch_unwind_ptr("nfc_device_new", || unsafe {
        nfc_device_new_impl(context, connstring)
    })
}

#[cfg(test)]
pub(crate) unsafe fn nfc_device_free(device: *mut nfc_device) {
    ffi_catch_unwind_void("nfc_device_free", || unsafe {
        if device.is_null() {
            return;
        }

        release_allocated_ptr((*device).driver_data);
        release_allocated_ptr(device as *mut c_void);
    });
}

pub(crate) unsafe fn nfc_context_free(context: *mut nfc_context) {
    ffi_catch_unwind_void("nfc_context_free", || unsafe {
        logging::increment_context_free_count_for_tests();
        logging::bridge_context_log_exit();
        free_context_allocation(context);
    });
}
