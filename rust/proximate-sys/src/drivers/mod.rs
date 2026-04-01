// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Internal Rust-owned builtin drivers.

#[cfg(any(test, all(libnfc_driver_pn532_i2c, not(target_os = "linux"))))]
pub(crate) mod pn532_i2c;
#[cfg(any(test, all(libnfc_driver_pn532_spi, not(target_os = "linux"))))]
pub(crate) mod pn532_spi;
#[cfg(any(test, all(libnfc_driver_pn532_uart, unix, not(target_os = "linux"))))]
pub(crate) mod pn532_uart;
#[cfg(any(
    test,
    all(libnfc_driver_pn532_i2c, not(target_os = "linux")),
    all(libnfc_driver_pn532_spi, not(target_os = "linux")),
    all(libnfc_driver_pn532_uart, unix, not(target_os = "linux"))
))]
pub(crate) mod pn53x_native;
#[cfg(any(test, all(libnfc_driver_pn53x_usb, not(target_os = "linux"))))]
pub(crate) mod pn53x_usb;
#[cfg(any(test, libnfc_driver_pn71xx))]
pub(crate) mod pn71xx;
