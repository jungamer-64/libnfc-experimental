use crate::{
    NFC_BUFSIZE_CONNSTRING, ffi_catch_unwind_ptr, ffi_catch_unwind_void, log_error,
    release_allocated_ptr, reset_last_error, set_last_error_message,
};
use libc::{c_char, c_int, c_uint, c_void};
use std::mem::size_of;
use std::ptr;

const DEVICE_NAME_LENGTH: usize = 256;
const MAX_USER_DEFINED_DEVICES: usize = 4;
const DEFAULT_CONTEXT_LOG_LEVEL: u32 = if cfg!(debug_assertions) { 3 } else { 1 };

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct nfc_driver {
    _private: [u8; 0],
}

#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[repr(C)]
pub struct nfc_user_defined_device {
    pub name: [c_char; DEVICE_NAME_LENGTH],
    pub connstring: [c_char; NFC_BUFSIZE_CONNSTRING],
    pub optional: bool,
}

#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[repr(C)]
pub struct nfc_context {
    pub allow_autoscan: bool,
    pub allow_intrusive_scan: bool,
    pub log_level: u32,
    pub user_defined_devices: [nfc_user_defined_device; MAX_USER_DEFINED_DEVICES],
    pub user_defined_device_count: c_uint,
}

#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[repr(C)]
pub struct nfc_device {
    pub context: *const nfc_context,
    pub driver: *const nfc_driver,
    pub driver_data: *mut c_void,
    pub chip_data: *mut c_void,
    pub name: [c_char; DEVICE_NAME_LENGTH],
    pub connstring: [c_char; NFC_BUFSIZE_CONNSTRING],
    pub bCrc: bool,
    pub bPar: bool,
    pub bEasyFraming: bool,
    pub bInfiniteSelect: bool,
    pub bAutoIso14443_4: bool,
    pub btSupportByte: u8,
    pub last_error: c_int,
}

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

    unsafe {
        (*context).allow_autoscan = true;
        (*context).allow_intrusive_scan = false;
        (*context).log_level = DEFAULT_CONTEXT_LOG_LEVEL;
    }

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

    let copy_len = crate::bounded_strlen(connstring, NFC_BUFSIZE_CONNSTRING.saturating_sub(1));
    unsafe {
        (*device).context = context;
        if copy_len > 0 {
            ptr::copy_nonoverlapping(connstring, (*device).connstring.as_mut_ptr(), copy_len);
        }
        (*device).connstring[copy_len] = 0;
    }

    reset_last_error();
    device
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_context_alloc_defaults() -> *mut nfc_context {
    ffi_catch_unwind_ptr("nfc_context_alloc_defaults", || unsafe {
        nfc_context_alloc_defaults_impl()
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_new(
    context: *const nfc_context,
    connstring: *const c_char,
) -> *mut nfc_device {
    ffi_catch_unwind_ptr("nfc_device_new", || unsafe {
        nfc_device_new_impl(context, connstring)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nfc_device_free(device: *mut nfc_device) {
    ffi_catch_unwind_void("nfc_device_free", || unsafe {
        if device.is_null() {
            return;
        }

        release_allocated_ptr((*device).driver_data);
        release_allocated_ptr(device as *mut c_void);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{CStr, CString};

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
            release_allocated_ptr(context as *mut c_void);
        }
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

        let err = crate::nfc_get_last_error();
        assert!(!err.is_null());
        let recovered = unsafe { CStr::from_ptr(err) }.to_str().unwrap();
        assert!(recovered.contains("NULL connstring in nfc_device_new"));
    }

    #[test]
    fn lifecycle_pointer_panic_is_normalized_to_null() {
        reset_last_error();
        let ptr = ffi_catch_unwind_ptr::<nfc_context, _>("lifecycle_ptr_panic", || panic!("boom"));
        assert!(ptr.is_null());

        let err = crate::nfc_get_last_error();
        assert!(!err.is_null());
        let recovered = unsafe { CStr::from_ptr(err) }.to_str().unwrap();
        assert!(recovered.contains("panic in lifecycle_ptr_panic"));
    }

    #[test]
    fn lifecycle_void_panic_is_normalized_to_noop() {
        reset_last_error();
        ffi_catch_unwind_void("lifecycle_void_panic", || panic!("boom"));

        let err = crate::nfc_get_last_error();
        assert!(!err.is_null());
        let recovered = unsafe { CStr::from_ptr(err) }.to_str().unwrap();
        assert!(recovered.contains("panic in lifecycle_void_panic"));
    }
}
