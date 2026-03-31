// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Ported from libnfc/conf.c.

use crate::emit_log_message;
use crate::lifecycle::{DEVICE_NAME_LENGTH, MAX_USER_DEFINED_DEVICES, nfc_context};
use crate::{LOG_PRIORITY_DEBUG, LOG_PRIORITY_ERROR, NFC_BUFSIZE_CONNSTRING};
use libc::c_char;
use std::ffi::CString;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::ptr;

const LOG_GROUP_CONFIG: u8 = 2;
const LOG_PRIORITY_INFO: u8 = 2;
const LOG_CATEGORY: *const c_char = b"libnfc.config\0" as *const u8 as *const c_char;
const LIBNFC_CONFFILE_NAME: &str = "libnfc.conf";
const LIBNFC_DEVICECONFDIR_NAME: &str = "devices.d";
const MAX_CONFIG_LINE_BYTES: usize = 8192;
const CONFIG_MAX_DEVICES_MESSAGE: &str = "Configuration exceeded maximum user-defined devices.";
#[cfg(not(test))]
const DEFAULT_NON_WINDOWS_CONFDIR: &str = "/usr/local/etc/nfc";
#[cfg(not(test))]
const DEFAULT_WINDOWS_CONFDIR: &str = "./config";

#[cfg(test)]
thread_local! {
    static TEST_CONF_ROOT: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn set_test_conf_root(root: Option<PathBuf>) {
    TEST_CONF_ROOT.with(|cell| {
        *cell.borrow_mut() = root;
    });
}

fn log_config_message(priority: u8, message: &str) {
    if let Ok(c_msg) = CString::new(message) {
        unsafe {
            emit_log_message(LOG_GROUP_CONFIG, LOG_CATEGORY, priority, c_msg.as_ptr());
        }
    }
}

fn log_config_debug(message: &str) {
    log_config_message(LOG_PRIORITY_DEBUG, message);
}

fn log_config_error(message: &str) {
    log_config_message(LOG_PRIORITY_ERROR, message);
}

fn log_config_info(message: &str) {
    log_config_message(LOG_PRIORITY_INFO, message);
}

fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

fn bytes_to_lossy_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn apply_boolean_bytes(value: &[u8], target: &mut bool) {
    if !(*target) {
        if value == b"yes" || value == b"true" || value == b"1" {
            *target = true;
        }
    } else if value == b"no" || value == b"false" || value == b"0" {
        *target = false;
    }
}

fn atoi_bytes(value: &[u8]) -> u32 {
    let Ok(c_string) = CString::new(value) else {
        return 0;
    };

    unsafe { libc::atoi(c_string.as_ptr()) as u32 }
}

fn parse_line(line: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let mut index = 0usize;

    while index < line.len() && is_space(line[index]) {
        index += 1;
    }

    if index >= line.len() || line[index] == b'\n' {
        return None;
    }

    let key_start = index;
    while index < line.len()
        && (line[index].is_ascii_alphanumeric() || matches!(line[index], b'_' | b'.'))
    {
        index += 1;
    }

    if index == key_start || index >= line.len() || line[index] == b'\n' {
        return None;
    }

    let key = line[key_start..index].to_vec();

    while index < line.len() && is_space(line[index]) {
        index += 1;
    }

    if index >= line.len() || line[index] != b'=' {
        return None;
    }
    index += 1;

    if index >= line.len() || line[index] == b'\n' {
        return None;
    }

    while index < line.len() && is_space(line[index]) {
        index += 1;
    }

    if index >= line.len() || line[index] == b'\n' {
        return None;
    }

    if line[index] == b'"' {
        index += 1;
        if index >= line.len() || line[index] == b'\n' {
            return None;
        }

        let value_start = index;
        while index < line.len() && line[index] != b'"' {
            index += 1;
        }

        if index >= line.len() || line[index] != b'"' {
            return None;
        }

        let value = line[value_start..index].to_vec();
        index += 1;

        while index < line.len() && is_space(line[index]) {
            index += 1;
        }

        if index < line.len() && line[index] != b'\n' {
            return None;
        }

        return Some((key, value));
    }

    let value_start = index;
    while index < line.len() && !is_space(line[index]) {
        index += 1;
    }

    let value = line[value_start..index].to_vec();

    if index < line.len() {
        index += 1;
        while index < line.len() && is_space(line[index]) {
            index += 1;
        }

        if index < line.len() {
            return None;
        }
    }

    Some((key, value))
}

unsafe fn copy_bytes_with_truncation(dst: *mut c_char, dst_size: usize, value: &[u8]) {
    if dst.is_null() || dst_size == 0 {
        return;
    }

    let copy_len = value.len().min(dst_size - 1);
    unsafe {
        if copy_len > 0 {
            ptr::copy_nonoverlapping(value.as_ptr() as *const c_char, dst, copy_len);
        }
        *dst.add(copy_len) = 0;
    }
}

unsafe fn last_device_name_empty(context: *mut nfc_context) -> bool {
    let last = unsafe { (*context).user_defined_device_count as usize - 1 };
    unsafe { (*context).user_defined_devices[last].name[0] == 0 }
}

unsafe fn last_device_connstring_empty(context: *mut nfc_context) -> bool {
    let last = unsafe { (*context).user_defined_device_count as usize - 1 };
    unsafe { (*context).user_defined_devices[last].connstring[0] == 0 }
}

unsafe fn last_device_optional(context: *mut nfc_context) -> bool {
    let last = unsafe { (*context).user_defined_device_count as usize - 1 };
    unsafe { (*context).user_defined_devices[last].optional }
}

unsafe fn push_user_defined_device_slot(context: *mut nfc_context) -> Option<usize> {
    if unsafe { (*context).user_defined_device_count as usize } >= MAX_USER_DEFINED_DEVICES {
        log_config_error(CONFIG_MAX_DEVICES_MESSAGE);
        return None;
    }

    unsafe {
        (*context).user_defined_device_count += 1;
        Some((*context).user_defined_device_count as usize - 1)
    }
}

unsafe fn current_device_index_for_name(context: *mut nfc_context) -> Option<usize> {
    if unsafe { (*context).user_defined_device_count == 0 } || unsafe { !last_device_name_empty(context) } {
        return unsafe { push_user_defined_device_slot(context) };
    }

    Some(unsafe { (*context).user_defined_device_count as usize - 1 })
}

unsafe fn current_device_index_for_connstring(context: *mut nfc_context) -> Option<usize> {
    if unsafe { (*context).user_defined_device_count == 0 }
        || unsafe { !last_device_connstring_empty(context) }
    {
        return unsafe { push_user_defined_device_slot(context) };
    }

    Some(unsafe { (*context).user_defined_device_count as usize - 1 })
}

unsafe fn current_device_index_for_optional(context: *mut nfc_context) -> Option<usize> {
    if unsafe { (*context).user_defined_device_count == 0 } || unsafe { last_device_optional(context) } {
        return unsafe { push_user_defined_device_slot(context) };
    }

    Some(unsafe { (*context).user_defined_device_count as usize - 1 })
}

unsafe fn conf_keyvalue_context(context: *mut nfc_context, key: &[u8], value: &[u8]) {
    log_config_debug(&format!(
        "key: [{}], value: [{}]",
        bytes_to_lossy_string(key),
        bytes_to_lossy_string(value)
    ));

    if key == b"allow_autoscan" {
        unsafe { apply_boolean_bytes(value, &mut (*context).allow_autoscan) };
        return;
    }

    if key == b"allow_intrusive_scan" {
        unsafe { apply_boolean_bytes(value, &mut (*context).allow_intrusive_scan) };
        return;
    }

    if key == b"log_level" {
        unsafe {
            (*context).log_level = atoi_bytes(value);
        }
        return;
    }

    if key == b"device.name" {
        let Some(index) = (unsafe { current_device_index_for_name(context) }) else {
            return;
        };

        unsafe {
            copy_bytes_with_truncation(
                (*context).user_defined_devices[index].name.as_mut_ptr(),
                DEVICE_NAME_LENGTH,
                value,
            );
        }
        return;
    }

    if key == b"device.connstring" {
        let Some(index) = (unsafe { current_device_index_for_connstring(context) }) else {
            return;
        };

        unsafe {
            copy_bytes_with_truncation(
                (*context).user_defined_devices[index].connstring.as_mut_ptr(),
                NFC_BUFSIZE_CONNSTRING,
                value,
            );
        }
        return;
    }

    if key == b"device.optional" {
        let Some(index) = (unsafe { current_device_index_for_optional(context) }) else {
            return;
        };

        if value == b"true" || value == b"True" || value == b"1" {
            unsafe {
                (*context).user_defined_devices[index].optional = true;
            }
        }
        return;
    }

    log_config_info(&format!(
        "Unknown key in config line: {} = {}",
        bytes_to_lossy_string(key),
        bytes_to_lossy_string(value)
    ));
}

unsafe fn conf_keyvalue_device(context: *mut nfc_context, key: &[u8], value: &[u8]) {
    let mut prefixed = b"device.".to_vec();
    prefixed.extend_from_slice(key);
    unsafe { conf_keyvalue_context(context, &prefixed, value) };
}

unsafe fn conf_parse_file(
    filename: &Path,
    conf_keyvalue: unsafe fn(*mut nfc_context, &[u8], &[u8]),
    context: *mut nfc_context,
) {
    let Ok(file) = File::open(filename) else {
        log_config_info(&format!("Unable to open file: {}", filename.display()));
        return;
    };

    let mut reader = BufReader::new(file);
    let mut line = Vec::with_capacity(256);
    let mut line_number = 0usize;

    loop {
        line.clear();
        let Ok(read_len) = reader.read_until(b'\n', &mut line) else {
            break;
        };

        if read_len == 0 {
            break;
        }

        line_number += 1;

        if line.first().copied().is_some_and(|byte| matches!(byte, b'#' | b'\n')) {
            continue;
        }

        if line.len() > MAX_CONFIG_LINE_BYTES {
            log_config_debug(&format!(
                "Parse error on line #{}: {}",
                line_number,
                bytes_to_lossy_string(&line)
            ));
            continue;
        }

        if let Some((key, value)) = parse_line(&line) {
            unsafe { conf_keyvalue(context, &key, &value) };
        } else {
            log_config_debug(&format!(
                "Parse error on line #{}: {}",
                line_number,
                bytes_to_lossy_string(&line)
            ));
        }
    }
}

unsafe fn conf_devices_load(dirname: &Path, context: *mut nfc_context) {
    let Ok(entries) = fs::read_dir(dirname) else {
        log_config_debug(&format!("Unable to open directory: {}", dirname.display()));
        return;
    };

    for entry_result in entries {
        let Ok(entry) = entry_result else {
            continue;
        };

        let file_name = entry.file_name();
        let file_name_bytes = file_name.to_string_lossy();
        if file_name_bytes.starts_with('.') || !file_name_bytes.ends_with(".conf") {
            continue;
        }

        let Ok(metadata) = entry.metadata() else {
            unsafe {
                libc::perror(b"stat\0".as_ptr() as *const c_char);
            }
            continue;
        };

        if metadata.is_file() {
            unsafe { conf_parse_file(&entry.path(), conf_keyvalue_device, context) };
        }
    }
}

#[cfg(not(test))]
fn compiled_conf_root() -> PathBuf {
    if let Some(path) = option_env!("LIBNFC_RS_CONFDIR") {
        PathBuf::from(path)
    } else if cfg!(windows) {
        PathBuf::from(DEFAULT_WINDOWS_CONFDIR)
    } else {
        PathBuf::from(DEFAULT_NON_WINDOWS_CONFDIR)
    }
}

fn configured_conf_root() -> Option<PathBuf> {
    #[cfg(test)]
    {
        return TEST_CONF_ROOT.with(|cell| cell.borrow().clone());
    }

    #[cfg(not(test))]
    {
        Some(compiled_conf_root())
    }
}

pub(crate) unsafe fn load_context_config(context: *mut nfc_context) {
    if context.is_null() {
        return;
    }

    let Some(root) = configured_conf_root() else {
        return;
    };

    unsafe { conf_parse_file(&root.join(LIBNFC_CONFFILE_NAME), conf_keyvalue_context, context) };
    unsafe { conf_devices_load(&root.join(LIBNFC_DEVICECONFDIR_NAME), context) };
}
