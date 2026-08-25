use super::{
    context_ref, nfc_context_alloc_defaults, nfc_context_free, nfc_context_new,
    reset_lifecycle_test_state, runtime_context_from_c, snapshot_lifecycle_test_state,
};
use std::ptr;

#[test]
fn default_handle_owns_default_runtime_context() {
    let context = unsafe { nfc_context_alloc_defaults() };
    assert!(!context.is_null());
    let runtime = runtime_context_from_c(context).unwrap();
    assert!(runtime.config.allow_autoscan);
    assert!(!runtime.config.allow_intrusive_scan);
    assert!(runtime.config.user_defined_devices.is_empty());
    unsafe { nfc_context_free(context) };
}

#[test]
fn null_context_decodes_as_absent() {
    assert!(runtime_context_from_c(ptr::null()).is_none());
}

#[test]
fn opaque_context_borrow_returns_owned_runtime() {
    let context = unsafe { nfc_context_alloc_defaults() };
    let borrowed = unsafe { context_ref(context) }.unwrap();
    assert_eq!(
        borrowed.runtime().config,
        runtime_context_from_c(context).unwrap().config
    );
    unsafe { nfc_context_free(context) };
}

#[test]
fn context_free_accepts_null() {
    unsafe { nfc_context_free(ptr::null_mut()) };
}

#[test]
fn loaded_context_initializes_and_finalizes_logging_once() {
    reset_lifecycle_test_state();
    let context = unsafe { nfc_context_new() };
    assert!(!context.is_null());
    let after_init = snapshot_lifecycle_test_state();
    assert_eq!(after_init.log_init_calls, 1);
    assert_eq!(after_init.log_exit_calls, 0);
    unsafe { nfc_context_free(context) };
    let after_exit = snapshot_lifecycle_test_state();
    assert_eq!(after_exit.log_exit_calls, 1);
    assert_eq!(after_exit.context_free_calls, 1);
}

#[test]
fn separately_allocated_contexts_have_distinct_identity() {
    let first = unsafe { nfc_context_alloc_defaults() };
    let second = unsafe { nfc_context_alloc_defaults() };
    assert_ne!(first, second);
    unsafe {
        nfc_context_free(first);
        nfc_context_free(second);
    }
}

#[test]
fn runtime_context_snapshot_is_not_an_alias() {
    let context = unsafe { nfc_context_alloc_defaults() };
    let mut snapshot = runtime_context_from_c(context).unwrap();
    snapshot.config.allow_autoscan = false;
    assert!(
        runtime_context_from_c(context)
            .unwrap()
            .config
            .allow_autoscan
    );
    unsafe { nfc_context_free(context) };
}
