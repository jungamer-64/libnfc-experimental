#[path = "native_helpers/i2c.rs"]
pub mod i2c;
#[cfg(any(test, feature = "nci_helper"))]
pub mod nci;
#[cfg(feature = "pcsc_helper")]
pub mod pcsc;
#[path = "native_helpers/spi.rs"]
pub mod spi;
#[path = "native_helpers/uart.rs"]
pub mod uart;
#[cfg(feature = "usb_helper")]
#[path = "native_helpers/usb.rs"]
pub mod usb;

mod native;

pub use native::register_builtin_drivers;
