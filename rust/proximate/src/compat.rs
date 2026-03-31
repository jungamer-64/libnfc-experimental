// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Public C-ABI compatibility helpers that remain part of libnfc's
// installed surface even after the core implementation moved to Rust.

use crate::ffi_support::{as_ref, bounded_strlen};
use crate::ffi_types::{nfc_baud_rate, nfc_modulation_type, nfc_target};
use crate::lifecycle::nfc_device;
use crate::{
    ffi_catch_unwind_int, ffi_catch_unwind_ptr, ffi_catch_unwind_void, release_allocated_ptr,
};
use libc::{c_char, c_int, c_void, size_t};
use std::ffi::CString;
use std::sync::OnceLock;

#[cfg(any(test, not(libnfc_external_bridges)))]
use crate::ffi_support::copy_bytes_to_c_buffer;
#[cfg(any(test, not(libnfc_external_bridges)))]
use std::ptr;
#[cfg(any(test, not(libnfc_external_bridges)))]
use std::slice;

const NFC_EINVARG: c_int = -2;
const NFC_ESOFT: c_int = -80;
const TARGET_RENDER_BUFFER_SIZE: usize = 4096;

const UNKNOWN_LABEL: *const c_char = c"???".as_ptr();
const UNDEFINED_BAUD_RATE_LABEL: *const c_char = c"undefined baud rate".as_ptr();
const BAUD_RATE_106_LABEL: *const c_char = c"106 kbps".as_ptr();
const BAUD_RATE_212_LABEL: *const c_char = c"212 kbps".as_ptr();
const BAUD_RATE_424_LABEL: *const c_char = c"424 kbps".as_ptr();
const BAUD_RATE_847_LABEL: *const c_char = c"847 kbps".as_ptr();
const MODULATION_ISO14443A_LABEL: *const c_char = c"ISO/IEC 14443A".as_ptr();
const MODULATION_ISO14443B_LABEL: *const c_char = c"ISO/IEC 14443-4B".as_ptr();
const MODULATION_ISO14443BI_LABEL: *const c_char = c"ISO/IEC 14443-4B'".as_ptr();
const MODULATION_ISO14443BICLASS_LABEL: *const c_char =
    c"ISO/IEC 14443-2B-3B iClass (Picopass)".as_ptr();
const MODULATION_ISO14443B2CT_LABEL: *const c_char = c"ISO/IEC 14443-2B ASK CTx".as_ptr();
const MODULATION_ISO14443B2SR_LABEL: *const c_char = c"ISO/IEC 14443-2B ST SRx".as_ptr();
const MODULATION_FELICA_LABEL: *const c_char = c"FeliCa".as_ptr();
const MODULATION_JEWEL_LABEL: *const c_char = c"Innovision Jewel".as_ptr();
const MODULATION_BARCODE_LABEL: *const c_char = c"Thinfilm NFC Barcode".as_ptr();
const MODULATION_DEP_LABEL: *const c_char = c"D.E.P.".as_ptr();

static VERSION_STRING: OnceLock<CString> = OnceLock::new();

#[cfg(all(not(test), libnfc_external_bridges))]
unsafe extern "C" {
    fn snprint_nfc_target(dst: *mut c_char, size: size_t, pnt: *const nfc_target, verbose: bool);
}

fn version_value() -> &'static str {
    option_env!("PROXIMATE_GIT_REVISION")
        .filter(|value| !value.is_empty())
        .unwrap_or(option_env!("PROXIMATE_PACKAGE_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")))
}

fn version_c_string() -> &'static CString {
    VERSION_STRING
        .get_or_init(|| CString::new(version_value()).expect("version string must not contain NUL"))
}

fn modulation_label(value: nfc_modulation_type) -> *const c_char {
    match value {
        nfc_modulation_type::NMT_ISO14443A => MODULATION_ISO14443A_LABEL,
        nfc_modulation_type::NMT_ISO14443B => MODULATION_ISO14443B_LABEL,
        nfc_modulation_type::NMT_ISO14443BI => MODULATION_ISO14443BI_LABEL,
        nfc_modulation_type::NMT_ISO14443BICLASS => MODULATION_ISO14443BICLASS_LABEL,
        nfc_modulation_type::NMT_ISO14443B2CT => MODULATION_ISO14443B2CT_LABEL,
        nfc_modulation_type::NMT_ISO14443B2SR => MODULATION_ISO14443B2SR_LABEL,
        nfc_modulation_type::NMT_FELICA => MODULATION_FELICA_LABEL,
        nfc_modulation_type::NMT_JEWEL => MODULATION_JEWEL_LABEL,
        nfc_modulation_type::NMT_BARCODE => MODULATION_BARCODE_LABEL,
        nfc_modulation_type::NMT_DEP => MODULATION_DEP_LABEL,
        _ => UNKNOWN_LABEL,
    }
}

fn baud_rate_label(value: nfc_baud_rate) -> *const c_char {
    match value {
        nfc_baud_rate::NBR_UNDEFINED => UNDEFINED_BAUD_RATE_LABEL,
        nfc_baud_rate::NBR_106 => BAUD_RATE_106_LABEL,
        nfc_baud_rate::NBR_212 => BAUD_RATE_212_LABEL,
        nfc_baud_rate::NBR_424 => BAUD_RATE_424_LABEL,
        nfc_baud_rate::NBR_847 => BAUD_RATE_847_LABEL,
    }
}

#[cfg(all(not(test), libnfc_external_bridges))]
unsafe fn bridge_snprint_nfc_target(
    dst: *mut c_char,
    size: size_t,
    target: *const nfc_target,
    verbose: bool,
) {
    unsafe { snprint_nfc_target(dst, size, target, verbose) };
}

#[cfg(any(test, not(libnfc_external_bridges)))]
unsafe fn bridge_snprint_nfc_target(
    dst: *mut c_char,
    size: size_t,
    target: *const nfc_target,
    verbose: bool,
) {
    let _ = verbose;

    if dst.is_null() || size == 0 {
        return;
    }

    let Some(target_ref) = (unsafe { as_ref(target) }) else {
        unsafe {
            *dst = 0;
        }
        return;
    };

    let modulation_type = unsafe { ptr::addr_of!(target_ref.nm.nmt).read_unaligned() };
    let baud_rate = unsafe { ptr::addr_of!(target_ref.nm.nbr).read_unaligned() };
    let modulation =
        unsafe { slice::from_raw_parts(modulation_label(modulation_type).cast::<u8>(), 128) };
    let modulation_len = modulation.iter().position(|&byte| byte == 0).unwrap_or(0);
    let baud = unsafe { slice::from_raw_parts(baud_rate_label(baud_rate).cast::<u8>(), 64) };
    let baud_len = baud.iter().position(|&byte| byte == 0).unwrap_or(0);

    let mut rendered = format!(
        "{} ({}) target:\n",
        String::from_utf8_lossy(&modulation[..modulation_len]),
        String::from_utf8_lossy(&baud[..baud_len])
    );

    if modulation_type == nfc_modulation_type::NMT_ISO14443A {
        let iso14443a = unsafe { ptr::addr_of!(target_ref.nti.nai).read_unaligned() };
        let uid_len = iso14443a.szUidLen.min(iso14443a.abtUid.len());
        if uid_len > 0 {
            rendered.push_str("UID (NFCID1):");
            for byte in &iso14443a.abtUid[..uid_len] {
                rendered.push(' ');
                rendered.push_str(&format!("{:02x}", byte));
            }
            rendered.push('\n');
        }
    }

    let _ = unsafe { copy_bytes_to_c_buffer(dst, size, rendered.as_bytes()) };
}

pub unsafe fn nfc_close(device: *mut nfc_device) {
    ffi_catch_unwind_void("nfc_close", || unsafe {
        let Some(device_ref) = as_ref(device) else {
            return;
        };
        let Some(driver_ref) = as_ref(device_ref.driver) else {
            return;
        };
        if let Some(close) = driver_ref.close {
            close(device);
        }
    });
}

pub unsafe fn nfc_free(ptr: *mut c_void) {
    ffi_catch_unwind_void("nfc_free", || unsafe {
        release_allocated_ptr(ptr);
    });
}

pub unsafe fn nfc_version() -> *const c_char {
    ffi_catch_unwind_ptr("nfc_version", || version_c_string().as_ptr().cast_mut()) as *const c_char
}

pub unsafe fn str_nfc_baud_rate(value: nfc_baud_rate) -> *const c_char {
    ffi_catch_unwind_ptr("str_nfc_baud_rate", || baud_rate_label(value).cast_mut()) as *const c_char
}

pub unsafe fn str_nfc_modulation_type(value: nfc_modulation_type) -> *const c_char {
    ffi_catch_unwind_ptr("str_nfc_modulation_type", || {
        modulation_label(value).cast_mut()
    }) as *const c_char
}

pub unsafe fn str_nfc_target(
    buf: *mut *mut c_char,
    target: *const nfc_target,
    verbose: bool,
) -> c_int {
    ffi_catch_unwind_int("str_nfc_target", NFC_ESOFT, || unsafe {
        if buf.is_null() || target.is_null() {
            return NFC_EINVARG;
        }

        let rendered = libc::malloc(TARGET_RENDER_BUFFER_SIZE) as *mut c_char;
        if rendered.is_null() {
            return NFC_ESOFT;
        }

        *buf = rendered;
        *rendered = 0;

        bridge_snprint_nfc_target(rendered, TARGET_RENDER_BUFFER_SIZE, target, verbose);
        bounded_strlen(rendered, TARGET_RENDER_BUFFER_SIZE) as c_int
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi_types::{nfc_target, nfc_target_info};
    use crate::lifecycle::nfc_driver;
    use std::ffi::CStr;

    unsafe extern "C" fn test_close(device: *mut nfc_device) {
        unsafe {
            (*device).last_error = 123;
        }
    }

    #[test]
    fn version_is_non_empty() {
        let version = unsafe { CStr::from_ptr(nfc_version()) }.to_str().unwrap();
        assert!(!version.is_empty());
    }

    #[test]
    fn close_dispatches_driver_callback() {
        let driver = nfc_driver {
            name: ptr::null(),
            scan_type: crate::lifecycle::scan_type_enum::NOT_AVAILABLE,
            scan: None,
            open: None,
            close: Some(test_close),
            strerror: None,
            initiator_init: None,
            initiator_init_secure_element: None,
            initiator_select_passive_target: None,
            initiator_poll_target: None,
            initiator_select_dep_target: None,
            initiator_deselect_target: None,
            initiator_transceive_bytes: None,
            initiator_transceive_bits: None,
            initiator_transceive_bytes_timed: None,
            initiator_transceive_bits_timed: None,
            initiator_target_is_present: None,
            target_init: None,
            target_send_bytes: None,
            target_receive_bytes: None,
            target_send_bits: None,
            target_receive_bits: None,
            device_set_property_bool: None,
            device_set_property_int: None,
            get_supported_modulation: None,
            get_supported_baud_rate: None,
            device_get_information_about: None,
            abort_command: None,
            idle: None,
            powerdown: None,
        };
        let mut device = nfc_device {
            context: ptr::null(),
            driver: ptr::addr_of!(driver),
            driver_data: ptr::null_mut(),
            chip_data: ptr::null_mut(),
            name: [0; crate::lifecycle::DEVICE_NAME_LENGTH],
            connstring: [0; crate::NFC_BUFSIZE_CONNSTRING],
            bCrc: false,
            bPar: false,
            bEasyFraming: false,
            bInfiniteSelect: false,
            bAutoIso14443_4: false,
            btSupportByte: 0,
            last_error: 0,
        };

        unsafe { nfc_close(ptr::addr_of_mut!(device)) };
        assert_eq!(device.last_error, 123);
    }

    #[test]
    fn target_renderer_returns_allocated_string() {
        let target = nfc_target {
            nm: crate::ffi_types::nfc_modulation {
                nmt: nfc_modulation_type::NMT_ISO14443A,
                nbr: nfc_baud_rate::NBR_106,
            },
            nti: nfc_target_info {
                nai: crate::ffi_types::nfc_iso14443a_info {
                    abtAtqa: [0; 2],
                    btSak: 0,
                    szUidLen: 4,
                    abtUid: [0x01, 0x02, 0x03, 0x04, 0, 0, 0, 0, 0, 0],
                    szAtsLen: 0,
                    abtAts: [0; 254],
                },
            },
        };
        let mut rendered = ptr::null_mut();

        let len =
            unsafe { str_nfc_target(ptr::addr_of_mut!(rendered), ptr::addr_of!(target), false) };
        assert!(len > 0);
        let text = unsafe { CStr::from_ptr(rendered) }
            .to_string_lossy()
            .into_owned();
        assert!(text.contains("ISO/IEC 14443A"));
        assert!(text.contains("106 kbps"));
        unsafe { nfc_free(rendered.cast()) };
    }
}
