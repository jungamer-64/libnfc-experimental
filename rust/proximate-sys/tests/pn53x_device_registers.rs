#![cfg(feature = "c_ffi")]

use proximate_sys::{
    nfc_close, nfc_context, nfc_device, nfc_device_get_name, nfc_exit, nfc_init, nfc_open,
};
use std::ffi::{CStr, CString};
use std::ptr;

const PN53X_REG_CIU_TXMODE: u16 = 0x6302;

struct OpenedDevice {
    context: *mut nfc_context,
    device: *mut nfc_device,
}

impl Drop for OpenedDevice {
    fn drop(&mut self) {
        unsafe {
            if !self.device.is_null() {
                nfc_close(self.device);
            }
            if !self.context.is_null() {
                nfc_exit(self.context);
            }
        }
    }
}

struct RegisterRestore {
    device: *mut nfc_device,
    register: u16,
    value: u8,
}

impl Drop for RegisterRestore {
    fn drop(&mut self) {
        unsafe {
            let _ =
                proximate_sys::pn53x_write_register(self.device, self.register, 0xff, self.value);
        }
    }
}

fn configured_connstring() -> Option<CString> {
    std::env::var("LIBNFC_PN53X_TEST_CONNSTRING")
        .ok()
        .map(|value| {
            CString::new(value).expect("LIBNFC_PN53X_TEST_CONNSTRING must not contain NUL")
        })
}

fn open_device() -> Option<OpenedDevice> {
    let mut context = ptr::null_mut();
    unsafe {
        nfc_init(&mut context);
    }
    if context.is_null() {
        eprintln!("Skipping PN53x device test: nfc_init() did not create a context");
        return None;
    }

    let connstring = configured_connstring();
    let device = unsafe {
        match connstring.as_ref() {
            Some(value) => nfc_open(context, value.as_ptr()),
            None => nfc_open(context, ptr::null()),
        }
    };

    if device.is_null() {
        if let Some(value) = connstring.as_ref() {
            eprintln!(
                "Skipping PN53x device test: could not open configured device {}",
                value.to_string_lossy()
            );
        } else {
            eprintln!("Skipping PN53x device test: could not open any device");
        }
        unsafe {
            nfc_exit(context);
        }
        return None;
    }

    Some(OpenedDevice { context, device })
}

fn device_name(device: *mut nfc_device) -> String {
    unsafe {
        let ptr = nfc_device_get_name(device);
        if ptr.is_null() {
            "<unknown>".to_string()
        } else {
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    }
}

fn require_pn53x_support(device: *mut nfc_device, status: i32, operation: &str) -> bool {
    if status == 0 {
        return true;
    }

    eprintln!(
        "Skipping PN53x device test: {} returned {} on device {}",
        operation,
        status,
        device_name(device)
    );
    false
}

#[test]
#[ignore = "requires a real PN53x-compatible device"]
fn register_access_roundtrip_on_real_device() {
    let Some(opened) = open_device() else {
        return;
    };

    let mut original = 0u8;
    let status = unsafe {
        proximate_sys::pn53x_read_register(opened.device, PN53X_REG_CIU_TXMODE, &mut original)
    };
    if !require_pn53x_support(opened.device, status, "pn53x_read_register(valid register)") {
        return;
    }

    let _restore = RegisterRestore {
        device: opened.device,
        register: PN53X_REG_CIU_TXMODE,
        value: original,
    };

    let mut value = 0u8;
    assert_eq!(
        unsafe {
            proximate_sys::pn53x_write_register(opened.device, PN53X_REG_CIU_TXMODE, 0xff, 0xaa)
        },
        0,
        "writing 0xaa should succeed on {}",
        device_name(opened.device)
    );
    assert_eq!(
        unsafe {
            proximate_sys::pn53x_read_register(opened.device, PN53X_REG_CIU_TXMODE, &mut value)
        },
        0,
        "reading back 0xaa should succeed on {}",
        device_name(opened.device)
    );
    assert_eq!(value, 0xaa);

    assert_eq!(
        unsafe {
            proximate_sys::pn53x_write_register(opened.device, PN53X_REG_CIU_TXMODE, 0xff, 0x55)
        },
        0,
        "writing 0x55 should succeed on {}",
        device_name(opened.device)
    );
    assert_eq!(
        unsafe {
            proximate_sys::pn53x_read_register(opened.device, PN53X_REG_CIU_TXMODE, &mut value)
        },
        0,
        "reading back 0x55 should succeed on {}",
        device_name(opened.device)
    );
    assert_eq!(value, 0x55);
}

#[test]
#[ignore = "requires a real PN53x-compatible device"]
fn invalid_register_status_matches_legacy_expectation() {
    let Some(opened) = open_device() else {
        return;
    };

    let mut value = 0u8;
    let status = unsafe { proximate_sys::pn53x_read_register(opened.device, 0xf0ff, &mut value) };
    if !require_pn53x_support(
        opened.device,
        status,
        "pn53x_read_register(valid xram register)",
    ) {
        return;
    }

    assert_eq!(
        unsafe { proximate_sys::pn53x_read_register(opened.device, 0xfff0, &mut value) },
        -1,
        "invalid SFR reads should keep returning -1 on {}",
        device_name(opened.device)
    );
}
