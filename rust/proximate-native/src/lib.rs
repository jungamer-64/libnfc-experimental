#[cfg(all(feature = "driver-pn532-i2c", not(target_os = "linux")))]
compile_error!("driver-pn532-i2c is supported only on Linux");
#[cfg(all(feature = "driver-pn532-spi", not(target_os = "linux")))]
compile_error!("driver-pn532-spi is supported only on Linux");
#[cfg(all(feature = "driver-pn71xx", not(target_os = "linux")))]
compile_error!("driver-pn71xx is supported only on Linux");
#[cfg(all(
    any(feature = "driver-pn53x-usb", feature = "driver-acr122-usb"),
    not(any(target_os = "linux", target_os = "macos", target_os = "windows"))
))]
compile_error!("USB drivers are supported only on Linux, macOS, and Windows");

#[cfg(any(
    test,
    feature = "driver-pn53x-usb",
    feature = "driver-acr122-usb",
    feature = "driver-pn532-i2c",
    feature = "driver-pn532-spi",
    feature = "driver-pn71xx"
))]
mod command_abort;
#[path = "native_helpers/i2c.rs"]
pub mod i2c;
#[cfg(any(test, feature = "driver-pn71xx"))]
pub mod nci;
#[cfg(any(feature = "driver-pcsc", feature = "driver-acr122-pcsc"))]
pub mod pcsc;
#[cfg(any(
    test,
    feature = "driver-acr122s",
    feature = "driver-arygon",
    feature = "driver-pn532-uart"
))]
mod serial;
#[path = "native_helpers/spi.rs"]
pub mod spi;
#[cfg(all(
    any(feature = "driver-pn53x-usb", feature = "driver-acr122-usb"),
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
#[path = "native_helpers/usb.rs"]
pub mod usb;

mod native;

pub use native::register_builtin_drivers;
