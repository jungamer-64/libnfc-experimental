// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Ported from libnfc/nfc.c.

use crate::c_boundary::{LOG_GROUP_GENERAL, LOG_PRIORITY_DEBUG};
use crate::emit_log_message;
use libc::c_char;
use std::ffi::CString;

pub(crate) mod accessors;
mod driver_dispatch;
pub(crate) mod emulation;
pub(crate) mod operations;
mod runtime;
#[cfg(test)]
mod tests;

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
