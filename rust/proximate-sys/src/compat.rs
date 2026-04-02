// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Public C-ABI compatibility helpers that remain part of libnfc's
// installed surface even after the core implementation moved to Rust.

use crate::ffi_strings::{baud_rate_label_cstr, modulation_label_cstr, version_cstr};
use crate::ffi_support::{as_ref, bounded_strlen};
use crate::ffi_types::{nfc_baud_rate, nfc_modulation_type, nfc_target};
use crate::lifecycle::nfc_device;
use crate::runtime_bridge::{baud_rate_from_c, modulation_type_from_c};
use crate::{
    ffi_catch_unwind_int, ffi_catch_unwind_ptr, ffi_catch_unwind_void, release_allocated_ptr,
};
use libc::{c_char, c_int, c_void, size_t};

#[cfg(test)]
use crate::c_api_impl::NFC_BUFSIZE_CONNSTRING;
use crate::ffi_support::copy_bytes_to_c_buffer;
use std::ptr;
use std::slice;

const NFC_EINVARG: c_int = -2;
const NFC_ESOFT: c_int = -80;
const TARGET_RENDER_BUFFER_SIZE: usize = 4096;

fn modulation_label(value: nfc_modulation_type) -> *const c_char {
    modulation_label_cstr(modulation_type_from_c(value)).as_ptr()
}

fn baud_rate_label(value: nfc_baud_rate) -> *const c_char {
    baud_rate_label_cstr(baud_rate_from_c(value)).as_ptr()
}

fn iso14443a_crc_bytes(data: &[u8]) -> [u8; 2] {
    let mut crc = 0x6363u16;
    for byte in data {
        let mut bt = *byte ^ (crc as u8);
        bt ^= bt << 4;
        crc = (crc >> 8) ^ (u16::from(bt) << 8) ^ (u16::from(bt) << 3) ^ (u16::from(bt) >> 4);
    }
    [(crc & 0xff) as u8, (crc >> 8) as u8]
}

fn iso14443b_crc_bytes(data: &[u8]) -> [u8; 2] {
    let mut crc = 0xffffu16;
    for byte in data {
        let mut bt = *byte ^ (crc as u8);
        bt ^= bt << 4;
        crc = (crc >> 8) ^ (u16::from(bt) << 8) ^ (u16::from(bt) << 3) ^ (u16::from(bt) >> 4);
    }
    crc = !crc;
    [(crc & 0xff) as u8, (crc >> 8) as u8]
}

fn locate_historical_bytes_offset(ats: &[u8]) -> Option<usize> {
    let t0 = *ats.first()?;
    let mut offset = 1usize;
    if t0 & 0x10 != 0 {
        offset += 1;
    }
    if t0 & 0x20 != 0 {
        offset += 1;
    }
    if t0 & 0x40 != 0 {
        offset += 1;
    }
    (offset < ats.len()).then_some(offset)
}

unsafe fn render_nfc_target(
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
    ffi_catch_unwind_ptr("nfc_version", || version_cstr().as_ptr().cast_mut()) as *const c_char
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

        render_nfc_target(rendered, TARGET_RENDER_BUFFER_SIZE, target, verbose);
        bounded_strlen(rendered, TARGET_RENDER_BUFFER_SIZE) as c_int
    })
}

pub unsafe fn iso14443a_crc(data: *mut u8, len: size_t, crc: *mut u8) {
    ffi_catch_unwind_void("iso14443a_crc", || unsafe {
        if data.is_null() || crc.is_null() {
            return;
        }
        let bytes = slice::from_raw_parts(data.cast_const(), len);
        let out = iso14443a_crc_bytes(bytes);
        *crc = out[0];
        *crc.add(1) = out[1];
    });
}

pub unsafe fn iso14443a_crc_append(data: *mut u8, len: size_t) {
    ffi_catch_unwind_void("iso14443a_crc_append", || unsafe {
        if data.is_null() {
            return;
        }
        let bytes = slice::from_raw_parts(data.cast_const(), len);
        let out = iso14443a_crc_bytes(bytes);
        *data.add(len) = out[0];
        *data.add(len + 1) = out[1];
    });
}

pub unsafe fn iso14443b_crc(data: *mut u8, len: size_t, crc: *mut u8) {
    ffi_catch_unwind_void("iso14443b_crc", || unsafe {
        if data.is_null() || crc.is_null() {
            return;
        }
        let bytes = slice::from_raw_parts(data.cast_const(), len);
        let out = iso14443b_crc_bytes(bytes);
        *crc = out[0];
        *crc.add(1) = out[1];
    });
}

pub unsafe fn iso14443b_crc_append(data: *mut u8, len: size_t) {
    ffi_catch_unwind_void("iso14443b_crc_append", || unsafe {
        if data.is_null() {
            return;
        }
        let bytes = slice::from_raw_parts(data.cast_const(), len);
        let out = iso14443b_crc_bytes(bytes);
        *data.add(len) = out[0];
        *data.add(len + 1) = out[1];
    });
}

pub unsafe fn iso14443a_locate_historical_bytes(
    ats: *mut u8,
    ats_len: size_t,
    tk_len: *mut size_t,
) -> *mut u8 {
    ffi_catch_unwind_ptr("iso14443a_locate_historical_bytes", || unsafe {
        if !tk_len.is_null() {
            *tk_len = 0;
        }
        if ats.is_null() {
            return ptr::null_mut();
        }

        let ats_slice = slice::from_raw_parts(ats.cast_const(), ats_len);
        let Some(offset) = locate_historical_bytes_offset(ats_slice) else {
            return ptr::null_mut();
        };
        if !tk_len.is_null() {
            *tk_len = ats_len - offset;
        }
        ats.add(offset).cast()
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
            connstring: [0; NFC_BUFSIZE_CONNSTRING],
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

    #[test]
    fn iso14443_crc_helpers_match_known_values() {
        let mut atqa = [0x26u8];
        let mut a_crc = [0u8; 2];
        unsafe { iso14443a_crc(atqa.as_mut_ptr(), atqa.len(), a_crc.as_mut_ptr()) };
        assert_eq!(a_crc, [0xca, 0x15]);

        let mut atqb = [0x05u8, 0x00, 0x08];
        let mut b_crc = [0u8; 2];
        unsafe { iso14443b_crc(atqb.as_mut_ptr(), atqb.len(), b_crc.as_mut_ptr()) };
        assert_eq!(b_crc, iso14443b_crc_bytes(&atqb));

        let mut appended = [0x26u8, 0x00, 0x00];
        unsafe { iso14443a_crc_append(appended.as_mut_ptr(), 1) };
        assert_eq!(appended[1..], a_crc);
    }

    #[test]
    fn locate_historical_bytes_matches_existing_ats_layout() {
        let mut ats = [0x75u8, 0x77, 0x81, 0x02, 0x80, 0x80];
        let mut tk_len = 0usize;
        let ptr = unsafe {
            iso14443a_locate_historical_bytes(
                ats.as_mut_ptr(),
                ats.len(),
                ptr::addr_of_mut!(tk_len),
            )
        };
        assert_eq!(tk_len, 2);
        assert_eq!(unsafe { slice::from_raw_parts(ptr, tk_len) }, [0x80, 0x80]);
    }
}
