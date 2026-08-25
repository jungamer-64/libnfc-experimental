use super::logging;
use super::{
    AbiContext, AbiDevice, context_into_raw, context_ref, device_into_raw, drop_context,
    nfc_context, nfc_device,
};
use crate::{ffi_catch_unwind_ptr, ffi_catch_unwind_void, reset_last_error};
use proximate_driver as rt;
use std::ptr;

#[cfg(test)]
unsafe fn nfc_context_alloc_defaults_impl() -> *mut nfc_context {
    reset_last_error();
    context_into_raw(AbiContext::new(rt::Context::default()))
}

unsafe fn nfc_context_new_impl() -> *mut nfc_context {
    let Ok(loaded) = logging::load_context_outcome() else {
        return ptr::null_mut();
    };

    logging::initialize_loaded_context_logging(&loaded.context);
    let context = context_into_raw(AbiContext::new(loaded.context));
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

pub(crate) fn runtime_context_from_c(context: *const nfc_context) -> Option<rt::Context> {
    unsafe { context_ref(context) }.map(|context| context.runtime().clone())
}

pub(crate) fn attach_device(device: rt::Device) -> *mut nfc_device {
    device_into_raw(AbiDevice::new(device))
}

pub(crate) unsafe fn nfc_context_free(context: *mut nfc_context) {
    ffi_catch_unwind_void("nfc_context_free", || unsafe {
        #[cfg(test)]
        logging::increment_context_free_count_for_tests();
        logging::finalize_context_logging();
        drop_context(context);
    });
}
