// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Rust-owned PN71xx driver backed by the external libnfc-nci userspace
// library.

#![allow(non_camel_case_types, non_snake_case)]

use crate::c_api_impl::{LOG_PRIORITY_DEBUG, LOG_PRIORITY_ERROR, NFC_BUFSIZE_CONNSTRING};
use crate::ffi_support::{as_mut, copy_bytes_to_c_buffer};
use crate::ffi_types::{
    nfc_baud_rate, nfc_felica_info, nfc_iso14443a_info, nfc_iso14443b_info, nfc_iso14443b2ct_info,
    nfc_iso14443b2sr_info, nfc_iso14443bi_info, nfc_jewel_info, nfc_mode, nfc_modulation,
    nfc_modulation_type, nfc_property, nfc_target,
};
use crate::lifecycle::{
    DEVICE_NAME_LENGTH, nfc_connstring, nfc_context, nfc_device, nfc_device_free, nfc_device_new,
    nfc_driver, scan_type_enum,
};
use crate::{
    emit_log_message, log_debug, log_error, release_allocated_ptr, reset_last_error,
    set_last_error_message,
};
use libc::{c_char, c_int, c_uchar, c_uint, c_void, size_t};
use std::ffi::{CStr, CString};
use std::mem::MaybeUninit;
use std::ptr;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

const NFC_SUCCESS: c_int = 0;
const NFC_EIO: c_int = -1;
const NFC_EINVARG: c_int = -2;
const NFC_ESOFT: c_int = -80;

const LOG_GROUP_DRIVER: u8 = 4;
const LOG_CATEGORY: *const c_char = b"libnfc.driver.pn71xx\0" as *const u8 as *const c_char;

const PN71XX_DRIVER_NAME: &[u8] = b"pn71xx";
const PN71XX_DEVICE_NAME: &[u8] = b"pn71xx-device";
const PN71XX_INFO: &[u8] = b"PN71XX nfc driver using libnfc-nci userspace library";
const DESFIRE_ATS: [u8; 4] = [0x75, 0x77, 0x81, 0x02];
const DEFAULT_NFA_TECH_MASK: c_int = 0x07;
const NFC_SETTLE_DELAY: Duration = Duration::from_secs(1);
const POLL_PERIOD_FACTOR_MICROS: u64 = 150_000;
const PN71XX_DRIVER_NAME_CSTR: *const c_char = b"pn71xx\0" as *const u8 as *const c_char;

#[allow(dead_code)]
const TARGET_TYPE_UNKNOWN: c_uint = 0x00;
const TARGET_TYPE_ISO14443_3A: c_uint = 0x01;
const TARGET_TYPE_ISO14443_3B: c_uint = 0x02;
const TARGET_TYPE_FELICA: c_uint = 0x03;
#[allow(dead_code)]
const TARGET_TYPE_ISO15693: c_uint = 0x04;
#[allow(dead_code)]
const TARGET_TYPE_NDEF: c_uint = 0x05;
#[allow(dead_code)]
const TARGET_TYPE_NDEF_FORMATABLE: c_uint = 0x06;
const TARGET_TYPE_MIFARE_CLASSIC: c_uint = 0x08;
const TARGET_TYPE_MIFARE_UL: c_uint = 0x09;
#[allow(dead_code)]
const TARGET_TYPE_KOVIO_BARCODE: c_uint = 0x0A;
const TARGET_TYPE_ISO14443_4: c_uint = 0x20;
#[allow(dead_code)]
const TARGET_TYPE_ISO14443_3A_3B: c_uint = 0x80;

const NFA_PROTOCOL_T1T: c_uchar = 0x01;

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
struct nfc_tag_info_t {
    technology: c_uint,
    handle: c_uint,
    uid: [u8; 32],
    uid_length: c_uint,
    protocol: c_uchar,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct nfcTagCallback_t {
    onTagArrival: Option<unsafe extern "C" fn(*mut nfc_tag_info_t)>,
    onTagDeparture: Option<unsafe extern "C" fn()>,
}

#[derive(Clone, Debug, Default)]
struct Pn71xxRuntime {
    initialized: bool,
    callbacks_registered: bool,
    discovery_enabled: bool,
    active_device: Option<usize>,
    current_tag: Option<nfc_tag_info_t>,
}

#[cfg(test)]
#[derive(Clone, Debug, Default)]
struct BackendTestState {
    init_result: c_int,
    initialize_calls: usize,
    deinitialize_calls: usize,
    register_calls: usize,
    deregister_calls: usize,
    enable_calls: usize,
    disable_calls: usize,
    last_discovery_args: Option<(c_int, c_int, c_int, c_int)>,
    callbacks: Option<nfcTagCallback_t>,
    transceive_result: c_int,
    transceive_response: Vec<u8>,
    last_transceive_handle: Option<c_uint>,
    last_transceive_tx: Vec<u8>,
    last_transceive_timeout: Option<c_int>,
}

static PN71XX_SUPPORTED_MODULATION_AS_TARGET: [nfc_modulation_type; 9] = [
    nfc_modulation_type::NMT_ISO14443A,
    nfc_modulation_type::NMT_FELICA,
    nfc_modulation_type::NMT_ISO14443B,
    nfc_modulation_type::NMT_ISO14443BI,
    nfc_modulation_type::NMT_ISO14443B2SR,
    nfc_modulation_type::NMT_ISO14443B2CT,
    nfc_modulation_type::NMT_JEWEL,
    nfc_modulation_type::NMT_DEP,
    nfc_modulation_type::NMT_UNDEFINED,
];

static PN71XX_SUPPORTED_MODULATION_AS_INITIATOR: [nfc_modulation_type; 9] = [
    nfc_modulation_type::NMT_ISO14443A,
    nfc_modulation_type::NMT_FELICA,
    nfc_modulation_type::NMT_ISO14443B,
    nfc_modulation_type::NMT_ISO14443BI,
    nfc_modulation_type::NMT_ISO14443B2SR,
    nfc_modulation_type::NMT_ISO14443B2CT,
    nfc_modulation_type::NMT_JEWEL,
    nfc_modulation_type::NMT_DEP,
    nfc_modulation_type::NMT_UNDEFINED,
];

static PN71XX_ISO14443A_SUPPORTED_BAUD_RATES: [nfc_baud_rate; 5] = [
    nfc_baud_rate::NBR_847,
    nfc_baud_rate::NBR_424,
    nfc_baud_rate::NBR_212,
    nfc_baud_rate::NBR_106,
    nfc_baud_rate::NBR_UNDEFINED,
];

static PN71XX_FELICA_SUPPORTED_BAUD_RATES: [nfc_baud_rate; 3] = [
    nfc_baud_rate::NBR_424,
    nfc_baud_rate::NBR_212,
    nfc_baud_rate::NBR_UNDEFINED,
];

static PN71XX_DEP_SUPPORTED_BAUD_RATES: [nfc_baud_rate; 4] = [
    nfc_baud_rate::NBR_424,
    nfc_baud_rate::NBR_212,
    nfc_baud_rate::NBR_106,
    nfc_baud_rate::NBR_UNDEFINED,
];

static PN71XX_JEWEL_SUPPORTED_BAUD_RATES: [nfc_baud_rate; 5] = [
    nfc_baud_rate::NBR_847,
    nfc_baud_rate::NBR_424,
    nfc_baud_rate::NBR_212,
    nfc_baud_rate::NBR_106,
    nfc_baud_rate::NBR_UNDEFINED,
];

static PN71XX_ISO14443B_SUPPORTED_BAUD_RATES: [nfc_baud_rate; 5] = [
    nfc_baud_rate::NBR_847,
    nfc_baud_rate::NBR_424,
    nfc_baud_rate::NBR_212,
    nfc_baud_rate::NBR_106,
    nfc_baud_rate::NBR_UNDEFINED,
];

static PN71XX_DRIVER: nfc_driver = nfc_driver {
    name: PN71XX_DRIVER_NAME_CSTR,
    scan_type: scan_type_enum::NOT_INTRUSIVE,
    scan: Some(pn71xx_scan),
    open: Some(pn71xx_open),
    close: Some(pn71xx_close),
    strerror: None,
    initiator_init: Some(pn71xx_initiator_init),
    initiator_init_secure_element: None,
    initiator_select_passive_target: Some(pn71xx_initiator_select_passive_target),
    initiator_poll_target: Some(pn71xx_initiator_poll_target),
    initiator_select_dep_target: None,
    initiator_deselect_target: Some(pn71xx_initiator_deselect_target),
    initiator_transceive_bytes: Some(pn71xx_initiator_transceive_bytes),
    initiator_transceive_bits: None,
    initiator_transceive_bytes_timed: None,
    initiator_transceive_bits_timed: None,
    initiator_target_is_present: Some(pn71xx_initiator_target_is_present),
    target_init: None,
    target_send_bytes: None,
    target_receive_bytes: None,
    target_send_bits: None,
    target_receive_bits: None,
    device_set_property_bool: Some(pn71xx_set_property_bool),
    device_set_property_int: Some(pn71xx_set_property_int),
    get_supported_modulation: Some(pn71xx_get_supported_modulation),
    get_supported_baud_rate: Some(pn71xx_get_supported_baud_rate),
    device_get_information_about: Some(pn71xx_get_information_about),
    abort_command: Some(pn71xx_abort_command),
    idle: Some(pn71xx_idle),
    powerdown: Some(pn71xx_powerdown),
};

static PN71XX_TAG_CALLBACK: nfcTagCallback_t = nfcTagCallback_t {
    onTagArrival: Some(pn71xx_on_tag_arrival),
    onTagDeparture: Some(pn71xx_on_tag_departure),
};

#[cfg(not(test))]
unsafe extern "C" {
    fn nfcManager_doInitialize() -> c_int;
    fn nfcManager_doDeinitialize();
    fn nfcManager_registerTagCallback(callback: *mut nfcTagCallback_t);
    fn nfcManager_deregisterTagCallback();
    fn nfcManager_enableDiscovery(
        technologies_mask: c_int,
        reader_mode: c_int,
        enable_host_routing: c_int,
        restart: c_int,
    );
    fn nfcManager_disableDiscovery();
    fn nfcTag_transceive(
        handle: c_uint,
        tx_buffer: *mut c_uchar,
        tx_len: c_int,
        rx_buffer: *mut c_uchar,
        rx_len: c_int,
        timeout: c_int,
    ) -> c_int;
}

fn runtime() -> &'static Mutex<Pn71xxRuntime> {
    static RUNTIME: OnceLock<Mutex<Pn71xxRuntime>> = OnceLock::new();
    RUNTIME.get_or_init(|| Mutex::new(Pn71xxRuntime::default()))
}

#[cfg(test)]
fn backend_state() -> &'static Mutex<BackendTestState> {
    static STATE: OnceLock<Mutex<BackendTestState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(BackendTestState::default()))
}

fn log_driver_message(priority: u8, message: &str) {
    if let Ok(c_message) = CString::new(message) {
        unsafe {
            emit_log_message(LOG_GROUP_DRIVER, LOG_CATEGORY, priority, c_message.as_ptr());
        }
    }
}

fn log_driver_debug(message: &str) {
    log_driver_message(LOG_PRIORITY_DEBUG, message);
}

fn log_driver_error(message: &str) {
    log_driver_message(LOG_PRIORITY_ERROR, message);
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut rendered = String::with_capacity(bytes.len().saturating_mul(3));
    for (index, byte) in bytes.iter().enumerate() {
        if index > 0 {
            rendered.push(' ');
        }
        rendered.push_str(&format!("{byte:02X}"));
    }
    rendered
}

fn modulation_type(nm: nfc_modulation) -> nfc_modulation_type {
    unsafe { ptr::addr_of!(nm.nmt).read_unaligned() }
}

fn pn71xx_driver_ptr() -> *const nfc_driver {
    ptr::addr_of!(PN71XX_DRIVER)
}

#[cfg(libnfc_driver_pn71xx)]
pub(crate) fn builtin_driver_ptr() -> *const nfc_driver {
    pn71xx_driver_ptr()
}

fn copy_connstring_entry(connstrings: *mut nfc_connstring, index: usize, value: &[u8]) -> bool {
    unsafe {
        copy_bytes_to_c_buffer(
            connstrings.add(index).cast::<c_char>(),
            NFC_BUFSIZE_CONNSTRING,
            value,
        )
    }
}

fn alloc_c_string(bytes: &[u8]) -> *mut c_char {
    let size = bytes.len().saturating_add(1);
    let buffer = unsafe { libc::malloc(size) as *mut c_char };
    if buffer.is_null() {
        log_driver_error("Failed to allocate info buffer");
        set_last_error_message("Failed to allocate info buffer");
        return ptr::null_mut();
    }

    if unsafe { copy_bytes_to_c_buffer(buffer, size, bytes) } {
        buffer
    } else {
        unsafe { release_allocated_ptr(buffer.cast::<c_void>()) };
        log_driver_error("Failed to copy info buffer");
        set_last_error_message("Failed to copy info buffer");
        ptr::null_mut()
    }
}

fn clear_runtime_state() {
    let mut state = runtime()
        .lock()
        .expect("pn71xx runtime mutex should not be poisoned");
    *state = Pn71xxRuntime::default();
}

fn normalize_inactive_runtime() {
    let snapshot = runtime()
        .lock()
        .expect("pn71xx runtime mutex should not be poisoned")
        .clone();
    if snapshot.active_device.is_some() {
        return;
    }

    if snapshot.discovery_enabled {
        backend_disable_discovery();
    }
    if snapshot.callbacks_registered {
        backend_deregister_callbacks();
    }
    if snapshot.initialized {
        backend_deinitialize();
    }

    clear_runtime_state();
}

fn tag_info_snapshot() -> Option<nfc_tag_info_t> {
    runtime()
        .lock()
        .expect("pn71xx runtime mutex should not be poisoned")
        .current_tag
}

fn technology_matches(tag: &nfc_tag_info_t, modulation: nfc_modulation_type) -> bool {
    match modulation {
        nfc_modulation_type::NMT_ISO14443A => matches!(
            tag.technology,
            TARGET_TYPE_ISO14443_4
                | TARGET_TYPE_ISO14443_3A
                | TARGET_TYPE_MIFARE_CLASSIC
                | TARGET_TYPE_MIFARE_UL
        ),
        nfc_modulation_type::NMT_ISO14443B
        | nfc_modulation_type::NMT_ISO14443BI
        | nfc_modulation_type::NMT_ISO14443B2SR
        | nfc_modulation_type::NMT_ISO14443B2CT => tag.technology == TARGET_TYPE_ISO14443_3B,
        nfc_modulation_type::NMT_FELICA => tag.technology == TARGET_TYPE_FELICA,
        nfc_modulation_type::NMT_JEWEL => {
            tag.technology == TARGET_TYPE_ISO14443_3A && tag.protocol == NFA_PROTOCOL_T1T
        }
        _ => false,
    }
}

fn build_target(tag: &nfc_tag_info_t, nm: nfc_modulation) -> Option<nfc_target> {
    let modulation = modulation_type(nm);
    if !technology_matches(tag, modulation) {
        return None;
    }

    let uid_len = (tag.uid_length as usize).min(tag.uid.len());
    if uid_len == 0 {
        return None;
    }

    let mut target = MaybeUninit::<nfc_target>::zeroed();
    let target_ptr = target.as_mut_ptr();
    unsafe {
        ptr::addr_of_mut!((*target_ptr).nm).write_unaligned(nm);
    }

    match modulation {
        nfc_modulation_type::NMT_ISO14443A => {
            let mut info = unsafe { std::mem::zeroed::<nfc_iso14443a_info>() };
            let copy_len = uid_len.min(info.abtUid.len());
            info.abtUid[..copy_len].copy_from_slice(&tag.uid[..copy_len]);
            info.szUidLen = copy_len;
            if tag.technology == TARGET_TYPE_MIFARE_CLASSIC {
                info.btSak = 0x08;
            } else {
                info.btSak = 0x20;
                info.szAtsLen = 5;
                info.abtAts[..DESFIRE_ATS.len()].copy_from_slice(&DESFIRE_ATS);
            }
            unsafe {
                ptr::addr_of_mut!((*target_ptr).nti.nai).write_unaligned(info);
            }
        }
        nfc_modulation_type::NMT_ISO14443B => {
            let mut info = unsafe { std::mem::zeroed::<nfc_iso14443b_info>() };
            let copy_len = uid_len.min(info.abtPupi.len());
            info.abtPupi[..copy_len].copy_from_slice(&tag.uid[..copy_len]);
            unsafe {
                ptr::addr_of_mut!((*target_ptr).nti.nbi).write_unaligned(info);
            }
        }
        nfc_modulation_type::NMT_ISO14443BI => {
            let mut info = unsafe { std::mem::zeroed::<nfc_iso14443bi_info>() };
            let copy_len = uid_len.min(info.abtDIV.len());
            info.abtDIV[..copy_len].copy_from_slice(&tag.uid[..copy_len]);
            unsafe {
                ptr::addr_of_mut!((*target_ptr).nti.nii).write_unaligned(info);
            }
        }
        nfc_modulation_type::NMT_ISO14443B2SR => {
            let mut info = unsafe { std::mem::zeroed::<nfc_iso14443b2sr_info>() };
            let copy_len = uid_len.min(info.abtUID.len());
            info.abtUID[..copy_len].copy_from_slice(&tag.uid[..copy_len]);
            unsafe {
                ptr::addr_of_mut!((*target_ptr).nti.nsi).write_unaligned(info);
            }
        }
        nfc_modulation_type::NMT_ISO14443B2CT => {
            let mut info = unsafe { std::mem::zeroed::<nfc_iso14443b2ct_info>() };
            let copy_len = uid_len.min(info.abtUID.len());
            info.abtUID[..copy_len].copy_from_slice(&tag.uid[..copy_len]);
            unsafe {
                ptr::addr_of_mut!((*target_ptr).nti.nci).write_unaligned(info);
            }
        }
        nfc_modulation_type::NMT_FELICA => {
            let mut info = unsafe { std::mem::zeroed::<nfc_felica_info>() };
            let copy_len = uid_len.min(info.abtId.len());
            info.abtId[..copy_len].copy_from_slice(&tag.uid[..copy_len]);
            unsafe {
                ptr::addr_of_mut!((*target_ptr).nti.nfi).write_unaligned(info);
            }
        }
        nfc_modulation_type::NMT_JEWEL => {
            let mut info = unsafe { std::mem::zeroed::<nfc_jewel_info>() };
            let copy_len = uid_len.min(info.btId.len());
            info.btId[..copy_len].copy_from_slice(&tag.uid[..copy_len]);
            unsafe {
                ptr::addr_of_mut!((*target_ptr).nti.nji).write_unaligned(info);
            }
        }
        _ => return None,
    }

    Some(unsafe { target.assume_init() })
}

unsafe extern "C" fn pn71xx_on_tag_arrival(tag: *mut nfc_tag_info_t) {
    if tag.is_null() {
        log_driver_error("tag callback received NULL tag info");
        return;
    }

    let tag = unsafe { ptr::read(tag) };
    log_driver_debug("tag found");
    runtime()
        .lock()
        .expect("pn71xx runtime mutex should not be poisoned")
        .current_tag = Some(tag);
}

unsafe extern "C" fn pn71xx_on_tag_departure() {
    log_driver_debug("tag lost");
    runtime()
        .lock()
        .expect("pn71xx runtime mutex should not be poisoned")
        .current_tag = None;
}

#[cfg(not(test))]
fn backend_initialize() -> c_int {
    unsafe { nfcManager_doInitialize() }
}

#[cfg(test)]
fn backend_initialize() -> c_int {
    let mut state = backend_state()
        .lock()
        .expect("pn71xx backend mutex should not be poisoned");
    state.initialize_calls += 1;
    state.init_result
}

#[cfg(not(test))]
fn backend_deinitialize() {
    unsafe { nfcManager_doDeinitialize() };
}

#[cfg(test)]
fn backend_deinitialize() {
    backend_state()
        .lock()
        .expect("pn71xx backend mutex should not be poisoned")
        .deinitialize_calls += 1;
}

#[cfg(not(test))]
fn backend_register_callbacks() {
    unsafe {
        nfcManager_registerTagCallback(ptr::addr_of!(PN71XX_TAG_CALLBACK).cast_mut());
    }
}

#[cfg(test)]
fn backend_register_callbacks() {
    let mut state = backend_state()
        .lock()
        .expect("pn71xx backend mutex should not be poisoned");
    state.register_calls += 1;
    state.callbacks = Some(PN71XX_TAG_CALLBACK);
}

#[cfg(not(test))]
fn backend_deregister_callbacks() {
    unsafe { nfcManager_deregisterTagCallback() };
}

#[cfg(test)]
fn backend_deregister_callbacks() {
    let mut state = backend_state()
        .lock()
        .expect("pn71xx backend mutex should not be poisoned");
    state.deregister_calls += 1;
    state.callbacks = None;
}

#[cfg(not(test))]
fn backend_enable_discovery() {
    unsafe {
        nfcManager_enableDiscovery(DEFAULT_NFA_TECH_MASK, 1, 0, 0);
    }
}

#[cfg(test)]
fn backend_enable_discovery() {
    let mut state = backend_state()
        .lock()
        .expect("pn71xx backend mutex should not be poisoned");
    state.enable_calls += 1;
    state.last_discovery_args = Some((DEFAULT_NFA_TECH_MASK, 1, 0, 0));
}

#[cfg(not(test))]
fn backend_disable_discovery() {
    unsafe { nfcManager_disableDiscovery() };
}

#[cfg(test)]
fn backend_disable_discovery() {
    backend_state()
        .lock()
        .expect("pn71xx backend mutex should not be poisoned")
        .disable_calls += 1;
}

#[cfg(not(test))]
fn backend_transceive(
    handle: c_uint,
    tx: *const u8,
    tx_len: size_t,
    rx: *mut u8,
    rx_len: size_t,
    timeout: c_int,
) -> c_int {
    unsafe {
        nfcTag_transceive(
            handle,
            tx.cast_mut(),
            tx_len as c_int,
            rx,
            rx_len as c_int,
            timeout,
        )
    }
}

#[cfg(test)]
fn backend_transceive(
    handle: c_uint,
    tx: *const u8,
    tx_len: size_t,
    rx: *mut u8,
    rx_len: size_t,
    timeout: c_int,
) -> c_int {
    let mut state = backend_state()
        .lock()
        .expect("pn71xx backend mutex should not be poisoned");
    state.last_transceive_handle = Some(handle);
    state.last_transceive_timeout = Some(timeout);
    state.last_transceive_tx = if tx.is_null() || tx_len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(tx, tx_len) }.to_vec()
    };
    if state.transceive_result <= 0 {
        return state.transceive_result;
    }

    if !rx.is_null() && rx_len > 0 {
        let copy_len = state
            .transceive_response
            .len()
            .min(rx_len)
            .min(state.transceive_result as usize);
        unsafe {
            ptr::copy_nonoverlapping(state.transceive_response.as_ptr(), rx, copy_len);
        }
    }

    state.transceive_result
}

unsafe extern "C" fn pn71xx_scan(
    _context: *const nfc_context,
    connstrings: *mut nfc_connstring,
    connstrings_len: size_t,
) -> size_t {
    if connstrings.is_null() || connstrings_len == 0 {
        return 0;
    }

    normalize_inactive_runtime();

    let runtime_snapshot = runtime()
        .lock()
        .expect("pn71xx runtime mutex should not be poisoned")
        .clone();
    if runtime_snapshot.active_device.is_some() {
        return if copy_connstring_entry(connstrings, 0, PN71XX_DRIVER_NAME) {
            1
        } else {
            0
        };
    }

    reset_last_error();
    let rc = backend_initialize();
    if rc != 0 {
        log_debug("pn71xx scan probe failed during backend initialization");
        return 0;
    }

    backend_deinitialize();

    if copy_connstring_entry(connstrings, 0, PN71XX_DRIVER_NAME) {
        1
    } else {
        log_driver_error("Failed to copy PN71xx connstring");
        0
    }
}

unsafe extern "C" fn pn71xx_open(
    context: *const nfc_context,
    connstring: *const c_char,
) -> *mut nfc_device {
    if connstring.is_null() {
        log_driver_error("pn71xx open received NULL connstring");
        set_last_error_message("pn71xx open received NULL connstring");
        return ptr::null_mut();
    }

    normalize_inactive_runtime();

    if runtime()
        .lock()
        .expect("pn71xx runtime mutex should not be poisoned")
        .active_device
        .is_some()
    {
        log_driver_error("pn71xx only supports one active device at a time");
        set_last_error_message("pn71xx only supports one active device at a time");
        return ptr::null_mut();
    }

    log_driver_debug(&format!(
        "open: {}",
        unsafe { CStr::from_ptr(connstring) }.to_string_lossy()
    ));

    let rc = backend_initialize();
    if rc != 0 {
        let message = format!("pn71xx backend initialization failed with rc={rc}");
        log_error(&message);
        set_last_error_message(message);
        return ptr::null_mut();
    }

    let device = unsafe { nfc_device_new(context, connstring) };
    if device.is_null() {
        backend_deinitialize();
        clear_runtime_state();
        return ptr::null_mut();
    }

    let Some(device_ref) = (unsafe { as_mut(device) }) else {
        backend_deinitialize();
        unsafe { nfc_device_free(device) };
        clear_runtime_state();
        return ptr::null_mut();
    };

    device_ref.driver = pn71xx_driver_ptr();
    if !unsafe {
        copy_bytes_to_c_buffer(
            device_ref.name.as_mut_ptr(),
            DEVICE_NAME_LENGTH,
            PN71XX_DEVICE_NAME,
        )
    } {
        backend_deinitialize();
        unsafe { nfc_device_free(device) };
        clear_runtime_state();
        log_driver_error("Failed to copy pn71xx device name");
        set_last_error_message("Failed to copy pn71xx device name");
        return ptr::null_mut();
    }

    backend_register_callbacks();
    backend_enable_discovery();
    thread::sleep(NFC_SETTLE_DELAY);

    {
        let mut state = runtime()
            .lock()
            .expect("pn71xx runtime mutex should not be poisoned");
        state.initialized = true;
        state.callbacks_registered = true;
        state.discovery_enabled = true;
        state.active_device = Some(device as usize);
        state.current_tag = None;
    }

    reset_last_error();
    device
}

unsafe extern "C" fn pn71xx_close(device: *mut nfc_device) {
    if device.is_null() {
        return;
    }

    let snapshot = runtime()
        .lock()
        .expect("pn71xx runtime mutex should not be poisoned")
        .clone();
    if snapshot.discovery_enabled {
        backend_disable_discovery();
    }
    if snapshot.callbacks_registered {
        backend_deregister_callbacks();
    }

    {
        let mut state = runtime()
            .lock()
            .expect("pn71xx runtime mutex should not be poisoned");
        state.discovery_enabled = false;
        state.callbacks_registered = false;
        state.current_tag = None;
        state.active_device = None;
    }

    if snapshot.initialized {
        backend_deinitialize();
    }

    {
        let mut state = runtime()
            .lock()
            .expect("pn71xx runtime mutex should not be poisoned");
        state.initialized = false;
    }

    unsafe { nfc_device_free(device) };
}

unsafe extern "C" fn pn71xx_initiator_init(device: *mut nfc_device) -> c_int {
    if device.is_null() {
        return NFC_EIO;
    }
    NFC_SUCCESS
}

unsafe extern "C" fn pn71xx_initiator_select_passive_target(
    device: *mut nfc_device,
    nm: nfc_modulation,
    _init_data: *const u8,
    _init_data_len: size_t,
    target: *mut nfc_target,
) -> c_int {
    if device.is_null() {
        return NFC_EIO;
    }

    log_driver_debug("select_passive_target");

    let Some(tag) = tag_info_snapshot() else {
        return 0;
    };
    let Some(mapped) = build_target(&tag, nm) else {
        return 0;
    };

    log_driver_debug("target found");
    if !target.is_null() {
        unsafe {
            ptr::write(target, mapped);
        }
    }
    1
}

unsafe extern "C" fn pn71xx_initiator_deselect_target(device: *mut nfc_device) -> c_int {
    if device.is_null() {
        return NFC_EIO;
    }
    log_driver_debug("deselect_passive_target");
    NFC_SUCCESS
}

unsafe extern "C" fn pn71xx_initiator_transceive_bytes(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: size_t,
    rx: *mut u8,
    rx_len: size_t,
    timeout: c_int,
) -> c_int {
    if device.is_null() {
        return NFC_EIO;
    }

    let Some(tag) = tag_info_snapshot() else {
        return NFC_EINVARG;
    };

    if (tx.is_null() && tx_len > 0) || (rx.is_null() && rx_len > 0) {
        return NFC_EINVARG;
    }

    log_driver_debug(&format!("transceive_bytes timeout={timeout}"));
    if !tx.is_null() && tx_len > 0 {
        let tx_bytes = unsafe { std::slice::from_raw_parts(tx, tx_len) };
        log_driver_debug(&format!("===> {}", encode_hex(tx_bytes)));
    }

    let received = backend_transceive(tag.handle, tx, tx_len, rx, rx_len, 500);
    if received <= 0 {
        return NFC_EIO;
    }

    if !rx.is_null() {
        let rx_bytes = unsafe { std::slice::from_raw_parts(rx, received as usize) };
        log_driver_debug(&format!("<=== {}", encode_hex(rx_bytes)));
    }

    received
}

unsafe extern "C" fn pn71xx_initiator_poll_target(
    device: *mut nfc_device,
    modulations: *const nfc_modulation,
    modulations_len: size_t,
    poll_nr: u8,
    period: u8,
    target: *mut nfc_target,
) -> c_int {
    if device.is_null() {
        return 0;
    }
    if modulations.is_null() && modulations_len > 0 {
        return NFC_EINVARG;
    }

    let sleep_duration = Duration::from_micros(period as u64 * POLL_PERIOD_FACTOR_MICROS);
    for _ in 0..poll_nr {
        for index in 0..modulations_len {
            let nm = unsafe { ptr::read(modulations.add(index)) };
            let result = unsafe {
                pn71xx_initiator_select_passive_target(device, nm, ptr::null(), 0, target)
            };
            if result > 0 {
                return result;
            }
        }
        if !sleep_duration.is_zero() {
            thread::sleep(sleep_duration);
        }
    }

    0
}

unsafe extern "C" fn pn71xx_initiator_target_is_present(
    device: *mut nfc_device,
    target: *const nfc_target,
) -> c_int {
    if device.is_null() || target.is_null() {
        return 1;
    }
    if tag_info_snapshot().is_some() { 0 } else { 1 }
}

unsafe extern "C" fn pn71xx_get_supported_modulation(
    device: *mut nfc_device,
    mode: nfc_mode,
    supported: *mut *const nfc_modulation_type,
) -> c_int {
    if device.is_null() {
        return NFC_EIO;
    }
    if supported.is_null() {
        return NFC_EINVARG;
    }

    unsafe {
        *supported = match mode {
            nfc_mode::N_TARGET => PN71XX_SUPPORTED_MODULATION_AS_TARGET.as_ptr(),
            nfc_mode::N_INITIATOR => PN71XX_SUPPORTED_MODULATION_AS_INITIATOR.as_ptr(),
        };
    }
    NFC_SUCCESS
}

unsafe extern "C" fn pn71xx_get_supported_baud_rate(
    device: *mut nfc_device,
    _mode: nfc_mode,
    modulation: nfc_modulation_type,
    supported: *mut *const nfc_baud_rate,
) -> c_int {
    if device.is_null() {
        return NFC_EIO;
    }
    if supported.is_null() {
        return NFC_EINVARG;
    }

    unsafe {
        *supported = match modulation {
            nfc_modulation_type::NMT_FELICA => PN71XX_FELICA_SUPPORTED_BAUD_RATES.as_ptr(),
            nfc_modulation_type::NMT_ISO14443A => PN71XX_ISO14443A_SUPPORTED_BAUD_RATES.as_ptr(),
            nfc_modulation_type::NMT_ISO14443B
            | nfc_modulation_type::NMT_ISO14443BI
            | nfc_modulation_type::NMT_ISO14443B2SR
            | nfc_modulation_type::NMT_ISO14443B2CT => {
                PN71XX_ISO14443B_SUPPORTED_BAUD_RATES.as_ptr()
            }
            nfc_modulation_type::NMT_JEWEL => PN71XX_JEWEL_SUPPORTED_BAUD_RATES.as_ptr(),
            nfc_modulation_type::NMT_DEP => PN71XX_DEP_SUPPORTED_BAUD_RATES.as_ptr(),
            _ => return NFC_EINVARG,
        };
    }
    NFC_SUCCESS
}

unsafe extern "C" fn pn71xx_set_property_bool(
    device: *mut nfc_device,
    _property: nfc_property,
    _enabled: bool,
) -> c_int {
    if device.is_null() {
        return NFC_EIO;
    }
    NFC_SUCCESS
}

unsafe extern "C" fn pn71xx_set_property_int(
    device: *mut nfc_device,
    _property: nfc_property,
    _value: c_int,
) -> c_int {
    if device.is_null() {
        return NFC_EIO;
    }
    NFC_SUCCESS
}

unsafe extern "C" fn pn71xx_get_information_about(
    device: *mut nfc_device,
    buf: *mut *mut c_char,
) -> c_int {
    if device.is_null() {
        return NFC_EIO;
    }
    if buf.is_null() {
        return NFC_EINVARG;
    }

    let rendered = alloc_c_string(PN71XX_INFO);
    if rendered.is_null() {
        return NFC_ESOFT;
    }

    unsafe {
        *buf = rendered;
    }
    (PN71XX_INFO.len() + 1) as c_int
}

unsafe extern "C" fn pn71xx_abort_command(device: *mut nfc_device) -> c_int {
    if device.is_null() {
        return NFC_EIO;
    }
    log_driver_debug("abort_command");
    NFC_SUCCESS
}

unsafe extern "C" fn pn71xx_idle(device: *mut nfc_device) -> c_int {
    if device.is_null() {
        return NFC_EIO;
    }
    log_driver_debug("idle");
    NFC_SUCCESS
}

unsafe extern "C" fn pn71xx_powerdown(device: *mut nfc_device) -> c_int {
    if device.is_null() {
        return NFC_EIO;
    }
    log_driver_debug("PowerDown");
    NFC_SUCCESS
}

#[cfg(test)]
fn reset_test_world() {
    clear_runtime_state();
    *backend_state()
        .lock()
        .expect("pn71xx backend mutex should not be poisoned") = BackendTestState::default();
}

#[cfg(test)]
fn emit_tag_arrival_for_tests(tag: nfc_tag_info_t) {
    let callbacks = backend_state()
        .lock()
        .expect("pn71xx backend mutex should not be poisoned")
        .callbacks;
    if let Some(callbacks) = callbacks {
        if let Some(on_arrival) = callbacks.onTagArrival {
            let mut local = tag;
            unsafe { on_arrival(ptr::addr_of_mut!(local)) };
        }
    }
}

#[cfg(test)]
fn emit_tag_departure_for_tests() {
    let callbacks = backend_state()
        .lock()
        .expect("pn71xx backend mutex should not be poisoned")
        .callbacks;
    if let Some(callbacks) = callbacks {
        if let Some(on_departure) = callbacks.onTagDeparture {
            unsafe { on_departure() };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::nfc_free;
    use crate::ffi_support::fixed_c_buffer_to_string;
    use crate::lifecycle::nfc_context_alloc_defaults;
    use std::sync::{Mutex, OnceLock};

    fn test_guard() -> &'static Mutex<()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD.get_or_init(|| Mutex::new(()))
    }

    fn make_tag(technology: c_uint, uid: &[u8], protocol: c_uchar) -> nfc_tag_info_t {
        let mut tag = nfc_tag_info_t {
            technology,
            handle: 0x1234,
            protocol,
            ..Default::default()
        };
        let copy_len = uid.len().min(tag.uid.len());
        tag.uid[..copy_len].copy_from_slice(&uid[..copy_len]);
        tag.uid_length = copy_len as c_uint;
        tag
    }

    fn read_target_uid_len(target: &nfc_target) -> usize {
        unsafe { ptr::addr_of!(target.nti.nai.szUidLen).read_unaligned() }
    }

    fn read_target_ats_len(target: &nfc_target) -> usize {
        unsafe { ptr::addr_of!(target.nti.nai.szAtsLen).read_unaligned() }
    }

    fn target_iso14443a(target: &nfc_target) -> nfc_iso14443a_info {
        unsafe { ptr::addr_of!(target.nti.nai).read_unaligned() }
    }

    fn target_iso14443b(target: &nfc_target) -> nfc_iso14443b_info {
        unsafe { ptr::addr_of!(target.nti.nbi).read_unaligned() }
    }

    fn target_iso14443bi(target: &nfc_target) -> nfc_iso14443bi_info {
        unsafe { ptr::addr_of!(target.nti.nii).read_unaligned() }
    }

    fn target_iso14443b2sr(target: &nfc_target) -> nfc_iso14443b2sr_info {
        unsafe { ptr::addr_of!(target.nti.nsi).read_unaligned() }
    }

    fn target_iso14443b2ct(target: &nfc_target) -> nfc_iso14443b2ct_info {
        unsafe { ptr::addr_of!(target.nti.nci).read_unaligned() }
    }

    fn target_felica(target: &nfc_target) -> nfc_felica_info {
        unsafe { ptr::addr_of!(target.nti.nfi).read_unaligned() }
    }

    fn target_jewel(target: &nfc_target) -> nfc_jewel_info {
        unsafe { ptr::addr_of!(target.nti.nji).read_unaligned() }
    }

    fn open_device(connstring: &CString) -> *mut nfc_device {
        let context = unsafe { nfc_context_alloc_defaults() };
        let device = unsafe { pn71xx_open(context, connstring.as_ptr()) };
        assert!(!device.is_null());
        device
    }

    #[test]
    fn scan_reports_success_and_failure() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        let context = unsafe { nfc_context_alloc_defaults() };
        let mut connstrings = [[0 as c_char; NFC_BUFSIZE_CONNSTRING]; 1];

        {
            let mut state = backend_state().lock().unwrap();
            state.init_result = 0;
        }
        let found = unsafe { pn71xx_scan(context, connstrings.as_mut_ptr(), connstrings.len()) };
        assert_eq!(found, 1);
        assert_eq!(
            fixed_c_buffer_to_string(&connstrings[0]),
            "pn71xx".to_string()
        );
        let snapshot = backend_state().lock().unwrap().clone();
        assert_eq!(snapshot.initialize_calls, 1);
        assert_eq!(snapshot.deinitialize_calls, 1);

        reset_test_world();
        let context = unsafe { nfc_context_alloc_defaults() };
        let mut connstrings = [[0 as c_char; NFC_BUFSIZE_CONNSTRING]; 1];
        {
            let mut state = backend_state().lock().unwrap();
            state.init_result = -1;
        }
        let found = unsafe { pn71xx_scan(context, connstrings.as_mut_ptr(), connstrings.len()) };
        assert_eq!(found, 0);
        let snapshot = backend_state().lock().unwrap().clone();
        assert_eq!(snapshot.initialize_calls, 1);
        assert_eq!(snapshot.deinitialize_calls, 0);
    }

    #[test]
    fn open_works_without_prior_scan() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        let connstring = CString::new("pn71xx").unwrap();
        let context = unsafe { nfc_context_alloc_defaults() };
        let device = unsafe { pn71xx_open(context, connstring.as_ptr()) };
        assert!(!device.is_null());
        assert_eq!(
            fixed_c_buffer_to_string(unsafe { &(*device).name }),
            "pn71xx-device".to_string()
        );
        let snapshot = backend_state().lock().unwrap().clone();
        assert_eq!(snapshot.initialize_calls, 1);
        assert_eq!(snapshot.register_calls, 1);
        assert_eq!(snapshot.enable_calls, 1);

        unsafe { pn71xx_close(device) };
    }

    #[test]
    fn second_concurrent_open_is_rejected() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        let connstring = CString::new("pn71xx").unwrap();
        let first = open_device(&connstring);
        let context = unsafe { nfc_context_alloc_defaults() };
        let second = unsafe { pn71xx_open(context, connstring.as_ptr()) };
        assert!(second.is_null());

        unsafe { pn71xx_close(first) };
    }

    #[test]
    fn close_tears_down_runtime_and_backend() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        let connstring = CString::new("pn71xx").unwrap();
        let device = open_device(&connstring);
        emit_tag_arrival_for_tests(make_tag(TARGET_TYPE_ISO14443_3A, &[0x11, 0x22], 0));

        unsafe { pn71xx_close(device) };

        let runtime = runtime().lock().unwrap().clone();
        assert!(!runtime.initialized);
        assert!(!runtime.callbacks_registered);
        assert!(!runtime.discovery_enabled);
        assert!(runtime.active_device.is_none());
        assert!(runtime.current_tag.is_none());

        let backend = backend_state().lock().unwrap().clone();
        assert_eq!(backend.disable_calls, 1);
        assert_eq!(backend.deregister_calls, 1);
        assert_eq!(backend.deinitialize_calls, 1);
    }

    #[test]
    fn callbacks_update_cached_tag_state() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        let connstring = CString::new("pn71xx").unwrap();
        let device = open_device(&connstring);

        emit_tag_arrival_for_tests(make_tag(TARGET_TYPE_ISO14443_3A, &[0x44], 0));
        assert!(runtime().lock().unwrap().current_tag.is_some());

        emit_tag_departure_for_tests();
        assert!(runtime().lock().unwrap().current_tag.is_none());

        unsafe { pn71xx_close(device) };
    }

    #[test]
    fn select_passive_target_maps_supported_technology_families() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        let connstring = CString::new("pn71xx").unwrap();
        let device = open_device(&connstring);

        let cases = [
            (
                make_tag(TARGET_TYPE_MIFARE_CLASSIC, &[0x01, 0x02, 0x03, 0x04], 0),
                nfc_modulation {
                    nmt: nfc_modulation_type::NMT_ISO14443A,
                    nbr: nfc_baud_rate::NBR_106,
                },
            ),
            (
                make_tag(TARGET_TYPE_ISO14443_3A, &[0x10, 0x11, 0x12, 0x13], 0),
                nfc_modulation {
                    nmt: nfc_modulation_type::NMT_ISO14443A,
                    nbr: nfc_baud_rate::NBR_106,
                },
            ),
            (
                make_tag(TARGET_TYPE_ISO14443_3B, &[0x21, 0x22, 0x23, 0x24], 0),
                nfc_modulation {
                    nmt: nfc_modulation_type::NMT_ISO14443B,
                    nbr: nfc_baud_rate::NBR_106,
                },
            ),
            (
                make_tag(TARGET_TYPE_ISO14443_3B, &[0x31, 0x32, 0x33, 0x34], 0),
                nfc_modulation {
                    nmt: nfc_modulation_type::NMT_ISO14443BI,
                    nbr: nfc_baud_rate::NBR_106,
                },
            ),
            (
                make_tag(
                    TARGET_TYPE_ISO14443_3B,
                    &[0x41, 0x42, 0x43, 0x44, 0x45, 0x46],
                    0,
                ),
                nfc_modulation {
                    nmt: nfc_modulation_type::NMT_ISO14443B2SR,
                    nbr: nfc_baud_rate::NBR_106,
                },
            ),
            (
                make_tag(TARGET_TYPE_ISO14443_3B, &[0x51, 0x52, 0x53, 0x54], 0),
                nfc_modulation {
                    nmt: nfc_modulation_type::NMT_ISO14443B2CT,
                    nbr: nfc_baud_rate::NBR_106,
                },
            ),
            (
                make_tag(
                    TARGET_TYPE_FELICA,
                    &[0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68],
                    0,
                ),
                nfc_modulation {
                    nmt: nfc_modulation_type::NMT_FELICA,
                    nbr: nfc_baud_rate::NBR_212,
                },
            ),
            (
                make_tag(
                    TARGET_TYPE_ISO14443_3A,
                    &[0x71, 0x72, 0x73, 0x74],
                    NFA_PROTOCOL_T1T,
                ),
                nfc_modulation {
                    nmt: nfc_modulation_type::NMT_JEWEL,
                    nbr: nfc_baud_rate::NBR_106,
                },
            ),
        ];

        for (index, (tag, modulation)) in cases.iter().enumerate() {
            emit_tag_arrival_for_tests(*tag);
            let mut target = unsafe { std::mem::zeroed::<nfc_target>() };
            let rc = unsafe {
                pn71xx_initiator_select_passive_target(
                    device,
                    *modulation,
                    ptr::null(),
                    0,
                    ptr::addr_of_mut!(target),
                )
            };
            assert_eq!(rc, 1, "case {index} should succeed");

            match modulation.nmt {
                nfc_modulation_type::NMT_ISO14443A => {
                    let info = target_iso14443a(&target);
                    assert_eq!(
                        &info.abtUid[..tag.uid_length as usize],
                        &tag.uid[..tag.uid_length as usize]
                    );
                    if tag.technology == TARGET_TYPE_MIFARE_CLASSIC {
                        assert_eq!(info.btSak, 0x08);
                    } else {
                        assert_eq!(info.btSak, 0x20);
                        assert_eq!(read_target_ats_len(&target), 5);
                        assert_eq!(&info.abtAts[..DESFIRE_ATS.len()], &DESFIRE_ATS);
                    }
                    assert_eq!(read_target_uid_len(&target), tag.uid_length as usize);
                }
                nfc_modulation_type::NMT_ISO14443B => {
                    let info = target_iso14443b(&target);
                    assert_eq!(info.abtPupi, [0x21, 0x22, 0x23, 0x24]);
                }
                nfc_modulation_type::NMT_ISO14443BI => {
                    let info = target_iso14443bi(&target);
                    assert_eq!(info.abtDIV, [0x31, 0x32, 0x33, 0x34]);
                }
                nfc_modulation_type::NMT_ISO14443B2SR => {
                    let info = target_iso14443b2sr(&target);
                    assert_eq!(&info.abtUID[..6], &[0x41, 0x42, 0x43, 0x44, 0x45, 0x46]);
                }
                nfc_modulation_type::NMT_ISO14443B2CT => {
                    let info = target_iso14443b2ct(&target);
                    assert_eq!(info.abtUID, [0x51, 0x52, 0x53, 0x54]);
                }
                nfc_modulation_type::NMT_FELICA => {
                    let info = target_felica(&target);
                    assert_eq!(info.abtId, [0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68]);
                }
                nfc_modulation_type::NMT_JEWEL => {
                    let info = target_jewel(&target);
                    assert_eq!(info.btId, [0x71, 0x72, 0x73, 0x74]);
                }
                _ => unreachable!(),
            }
        }

        unsafe { pn71xx_close(device) };
    }

    #[test]
    fn poll_target_retries_until_tag_appears() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        let connstring = CString::new("pn71xx").unwrap();
        let device = open_device(&connstring);
        let worker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));
            emit_tag_arrival_for_tests(make_tag(TARGET_TYPE_ISO14443_3A, &[0xAA, 0xBB], 0));
        });

        let modulations = [nfc_modulation {
            nmt: nfc_modulation_type::NMT_ISO14443A,
            nbr: nfc_baud_rate::NBR_106,
        }];
        let mut target = unsafe { std::mem::zeroed::<nfc_target>() };
        let rc = unsafe {
            pn71xx_initiator_poll_target(
                device,
                modulations.as_ptr(),
                modulations.len(),
                2,
                1,
                ptr::addr_of_mut!(target),
            )
        };
        worker.join().unwrap();
        assert_eq!(rc, 1);
        let info = target_iso14443a(&target);
        assert_eq!(info.abtUid[0], 0xAA);
        assert_eq!(info.abtUid[1], 0xBB);

        unsafe { pn71xx_close(device) };
    }

    #[test]
    fn transceive_bytes_handles_missing_and_present_tags() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        let connstring = CString::new("pn71xx").unwrap();
        let device = open_device(&connstring);
        let tx = [0x30u8, 0x04];
        let mut rx = [0u8; 8];

        let missing = unsafe {
            pn71xx_initiator_transceive_bytes(
                device,
                tx.as_ptr(),
                tx.len(),
                rx.as_mut_ptr(),
                rx.len(),
                250,
            )
        };
        assert_eq!(missing, NFC_EINVARG);

        emit_tag_arrival_for_tests(make_tag(TARGET_TYPE_ISO14443_3A, &[0x01], 0));
        {
            let mut state = backend_state().lock().unwrap();
            state.transceive_result = 4;
            state.transceive_response = vec![0xDE, 0xAD, 0xBE, 0xEF];
        }

        let received = unsafe {
            pn71xx_initiator_transceive_bytes(
                device,
                tx.as_ptr(),
                tx.len(),
                rx.as_mut_ptr(),
                rx.len(),
                250,
            )
        };
        assert_eq!(received, 4);
        assert_eq!(&rx[..4], &[0xDE, 0xAD, 0xBE, 0xEF]);

        let state = backend_state().lock().unwrap().clone();
        assert_eq!(state.last_transceive_handle, Some(0x1234));
        assert_eq!(state.last_transceive_tx, tx);
        assert_eq!(state.last_transceive_timeout, Some(500));

        unsafe { pn71xx_close(device) };
    }

    #[test]
    fn device_get_information_about_returns_caller_free_buffer() {
        let _guard = test_guard().lock().unwrap();
        reset_test_world();

        let connstring = CString::new("pn71xx").unwrap();
        let device = open_device(&connstring);
        let mut info = ptr::null_mut();

        let len = unsafe { pn71xx_get_information_about(device, ptr::addr_of_mut!(info)) };
        assert_eq!(len as usize, PN71XX_INFO.len() + 1);
        assert!(!info.is_null());
        assert_eq!(
            unsafe { CStr::from_ptr(info) }.to_string_lossy(),
            "PN71XX nfc driver using libnfc-nci userspace library"
        );

        unsafe {
            nfc_free(info.cast::<c_void>());
            pn71xx_close(device);
        }
    }
}
