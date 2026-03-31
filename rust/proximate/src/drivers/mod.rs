// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Internal Rust-owned builtin drivers.

#[cfg(any(test, libnfc_driver_pn71xx))]
pub(crate) mod pn71xx;
