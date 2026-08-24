#[cfg(any(
    test,
    feature = "driver-acr122-pcsc",
    feature = "driver-acr122-usb",
    feature = "driver-acr122s"
))]
mod acr122;
#[cfg(feature = "driver-acr122-pcsc")]
mod acr122_pcsc;
#[cfg(all(
    feature = "driver-acr122-usb",
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
mod acr122_usb;
#[cfg(any(test, feature = "driver-acr122s"))]
mod acr122s;
#[cfg(any(test, feature = "driver-arygon"))]
mod arygon;
#[cfg(any(
    test,
    feature = "driver-pcsc",
    feature = "driver-acr122-pcsc",
    feature = "driver-acr122s",
    feature = "driver-arygon",
    feature = "driver-acr122-usb",
    feature = "driver-pn532-uart",
    feature = "driver-pn532-spi",
    feature = "driver-pn532-i2c",
    feature = "driver-pn53x-usb"
))]
mod connstring;
#[cfg(all(target_os = "linux", feature = "driver-pn532-i2c"))]
mod i2c;
#[cfg(any(feature = "driver-pcsc", feature = "driver-acr122-pcsc"))]
mod pcsc;
mod pn53x;
#[cfg(any(test, all(target_os = "linux", feature = "driver-pn71xx")))]
mod pn71xx;
#[cfg(all(target_os = "linux", feature = "driver-pn532-spi"))]
mod spi;
#[cfg(any(
    test,
    feature = "driver-acr122s",
    feature = "driver-arygon",
    feature = "driver-pn532-uart"
))]
mod uart;
#[cfg(all(
    any(feature = "driver-pn53x-usb", feature = "driver-acr122-usb"),
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
mod usb;

use proximate_driver::DriverRegistry;

pub fn register_builtin_drivers(_registry: &mut DriverRegistry) {
    // DriverRegistry walks in reverse, preserving libnfc 1.8.0's effective
    // driver precedence.
    #[cfg(all(
        feature = "driver-pn53x-usb",
        any(target_os = "linux", target_os = "macos", target_os = "windows")
    ))]
    _registry.register_driver(Box::new(usb::Pn53xUsbDriver::new()));
    #[cfg(feature = "driver-pcsc")]
    _registry.register_driver(Box::new(pcsc::PcscDriver::new()));
    #[cfg(feature = "driver-acr122-pcsc")]
    _registry.register_driver(Box::new(acr122_pcsc::Acr122PcscDriver::new()));
    #[cfg(all(
        feature = "driver-acr122-usb",
        any(target_os = "linux", target_os = "macos", target_os = "windows")
    ))]
    _registry.register_driver(Box::new(acr122_usb::Acr122UsbDriver::new()));
    #[cfg(feature = "driver-acr122s")]
    _registry.register_driver(Box::new(acr122s::Acr122sDriver::new()));
    #[cfg(feature = "driver-pn532-uart")]
    _registry.register_driver(Box::new(uart::Pn532UartDriver::new()));
    #[cfg(all(target_os = "linux", feature = "driver-pn532-spi"))]
    _registry.register_driver(Box::new(spi::Pn532SpiDriver::new()));
    #[cfg(all(target_os = "linux", feature = "driver-pn532-i2c"))]
    _registry.register_driver(Box::new(i2c::Pn532I2cDriver::new()));
    #[cfg(feature = "driver-arygon")]
    _registry.register_driver(Box::new(arygon::ArygonDriver::new()));
    #[cfg(all(target_os = "linux", feature = "driver-pn71xx"))]
    _registry.register_driver(Box::new(pn71xx::Pn71xxDriver::new()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registration_matches_enabled_driver_order() {
        let mut registry = DriverRegistry::new();
        register_builtin_drivers(&mut registry);

        let expected = [
            #[cfg(feature = "driver-pn53x-usb")]
            "pn53x_usb",
            #[cfg(feature = "driver-pcsc")]
            "pcsc",
            #[cfg(feature = "driver-acr122-pcsc")]
            "acr122_pcsc",
            #[cfg(feature = "driver-acr122-usb")]
            "acr122_usb",
            #[cfg(feature = "driver-acr122s")]
            "ACR122S",
            #[cfg(feature = "driver-pn532-uart")]
            "pn532_uart",
            #[cfg(all(target_os = "linux", feature = "driver-pn532-spi"))]
            "pn532_spi",
            #[cfg(all(target_os = "linux", feature = "driver-pn532-i2c"))]
            "pn532_i2c",
            #[cfg(feature = "driver-arygon")]
            "arygon",
            #[cfg(all(target_os = "linux", feature = "driver-pn71xx"))]
            "pn71xx",
        ];

        assert_eq!(registry.registered_driver_names(), expected);
    }
}
