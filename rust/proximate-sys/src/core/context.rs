use super::*;

pub(super) unsafe fn nfc_init_impl(context: *mut *mut nfc_context) {
    if context.is_null() {
        log_general_error("nfc_init: NULL context pointer");
        return;
    }

    unsafe {
        *context = nfc_context_new();
        if (*context).is_null() {
            libc::perror(MALLOC_LABEL);
            return;
        }
    }
}

pub unsafe fn nfc_init(context: *mut *mut nfc_context) {
    ffi_catch_unwind_void("nfc_init", || unsafe { nfc_init_impl(context) });
}

pub unsafe fn nfc_exit(context: *mut nfc_context) {
    ffi_catch_unwind_void("nfc_exit", || unsafe {
        clear_registry();
        crate::lifecycle::nfc_context_free(context);
    });
}
