// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Internal Rust-owned builtin drivers.

#[cfg(any(test, libnfc_driver_pn532_i2c))]
pub(crate) mod pn532_i2c;
#[cfg(any(test, libnfc_driver_pn532_spi))]
pub(crate) mod pn532_spi;
#[cfg(any(test, libnfc_driver_pn532_uart))]
pub(crate) mod pn532_uart;
pub(crate) mod pn53x_native;
#[cfg(any(test, libnfc_driver_pn53x_usb))]
pub(crate) mod pn53x_usb;
#[cfg(any(test, libnfc_driver_pn71xx))]
pub(crate) mod pn71xx;
