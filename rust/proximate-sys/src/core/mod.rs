// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Ported from libnfc/nfc.c.

use crate::c_api_impl::{LOG_GROUP_GENERAL, LOG_PRIORITY_DEBUG, LOG_PRIORITY_ERROR};
use crate::emit_log_message;
use libc::c_char;
use std::ffi::CString;

pub(crate) mod context;
pub(crate) mod driver_registration;
pub(crate) mod runtime;
#[cfg(test)]
mod tests;

const LOG_PRIORITY_INFO: u8 = 2;
const GENERAL_LOG_CATEGORY: *const c_char = b"libnfc.general\0" as *const u8 as *const c_char;

fn log_general_message(priority: u8, message: &str) {
    if let Ok(c_msg) = CString::new(message) {
        unsafe {
            emit_log_message(
                LOG_GROUP_GENERAL,
                GENERAL_LOG_CATEGORY,
                priority,
                c_msg.as_ptr(),
            );
        }
    }
}

pub(super) fn log_general_debug(message: &str) {
    log_general_message(LOG_PRIORITY_DEBUG, message);
}

pub(super) fn log_general_error(message: &str) {
    log_general_message(LOG_PRIORITY_ERROR, message);
}

pub(super) fn log_general_info(message: &str) {
    log_general_message(LOG_PRIORITY_INFO, message);
}
