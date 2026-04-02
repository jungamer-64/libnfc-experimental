#[cfg(any(
    test,
    all(feature = "pcsc_helper", libnfc_driver_acr122_pcsc),
    all(feature = "usb_helper", libnfc_driver_acr122_usb),
    libnfc_driver_acr122s
))]
mod acr122;
#[cfg(all(feature = "pcsc_helper", libnfc_driver_acr122_pcsc))]
mod acr122_pcsc;
#[cfg(all(feature = "usb_helper", libnfc_driver_acr122_usb))]
mod acr122_usb;
#[cfg(any(test, all(target_os = "linux", libnfc_driver_acr122s)))]
mod acr122s;
#[cfg(any(test, all(target_os = "linux", libnfc_driver_arygon)))]
mod arygon;
#[cfg(any(
    test,
    all(feature = "pcsc_helper", libnfc_driver_pcsc),
    all(feature = "pcsc_helper", libnfc_driver_acr122_pcsc),
    all(target_os = "linux", libnfc_driver_acr122s),
    all(target_os = "linux", libnfc_driver_arygon),
    all(feature = "usb_helper", libnfc_driver_acr122_usb),
    all(
        target_os = "linux",
        any(
            libnfc_driver_pn532_i2c,
            libnfc_driver_pn532_spi,
            libnfc_driver_pn532_uart
        )
    ),
    all(feature = "usb_helper", libnfc_driver_pn53x_usb)
))]
mod connstring;
#[cfg(all(target_os = "linux", libnfc_driver_pn532_i2c))]
mod i2c;
#[cfg(all(feature = "pcsc_helper", libnfc_driver_pcsc))]
mod pcsc;
mod pn53x;
#[cfg(any(test, all(feature = "nci_helper", libnfc_driver_pn71xx)))]
mod pn71xx;
#[cfg(all(target_os = "linux", libnfc_driver_pn532_spi))]
mod spi;
#[cfg(any(
    test,
    all(
        target_os = "linux",
        any(libnfc_driver_acr122s, libnfc_driver_arygon, libnfc_driver_pn532_uart)
    )
))]
mod uart;
#[cfg(all(feature = "usb_helper", libnfc_driver_pn53x_usb))]
mod usb;

use proximate_driver::DriverRegistry;

pub fn register_builtin_drivers(_registry: &mut DriverRegistry) {
    // Keep libnfc_orig's init order here. DriverRegistry walks in reverse,
    // which preserves the original effective driver precedence.
    #[cfg(all(feature = "usb_helper", libnfc_driver_pn53x_usb))]
    _registry.register_driver(Box::new(usb::Pn53xUsbDriver::new()));
    #[cfg(all(feature = "pcsc_helper", libnfc_driver_pcsc))]
    _registry.register_driver(Box::new(pcsc::PcscDriver::new()));
    #[cfg(all(feature = "pcsc_helper", libnfc_driver_acr122_pcsc))]
    _registry.register_driver(Box::new(acr122_pcsc::Acr122PcscDriver::new()));
    #[cfg(all(feature = "usb_helper", libnfc_driver_acr122_usb))]
    _registry.register_driver(Box::new(acr122_usb::Acr122UsbDriver::new()));
    #[cfg(all(target_os = "linux", libnfc_driver_acr122s))]
    _registry.register_driver(Box::new(acr122s::Acr122sDriver::new()));
    #[cfg(all(target_os = "linux", libnfc_driver_pn532_uart))]
    _registry.register_driver(Box::new(uart::Pn532UartDriver::new()));
    #[cfg(all(target_os = "linux", libnfc_driver_pn532_spi))]
    _registry.register_driver(Box::new(spi::Pn532SpiDriver::new()));
    #[cfg(all(target_os = "linux", libnfc_driver_pn532_i2c))]
    _registry.register_driver(Box::new(i2c::Pn532I2cDriver::new()));
    #[cfg(all(target_os = "linux", libnfc_driver_arygon))]
    _registry.register_driver(Box::new(arygon::ArygonDriver::new()));
    #[cfg(all(feature = "nci_helper", libnfc_driver_pn71xx))]
    _registry.register_driver(Box::new(pn71xx::Pn71xxDriver::new()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registration_is_safe_when_no_native_drivers_are_enabled() {
        let mut registry = DriverRegistry::new();
        register_builtin_drivers(&mut registry);
        #[cfg(any(
            all(feature = "pcsc_helper", libnfc_driver_pcsc),
            all(feature = "pcsc_helper", libnfc_driver_acr122_pcsc),
            all(target_os = "linux", libnfc_driver_acr122s),
            all(target_os = "linux", libnfc_driver_arygon),
            all(feature = "usb_helper", libnfc_driver_acr122_usb),
            all(feature = "nci_helper", libnfc_driver_pn71xx),
            all(target_os = "linux", libnfc_driver_pn532_uart),
            all(target_os = "linux", libnfc_driver_pn532_spi),
            all(target_os = "linux", libnfc_driver_pn532_i2c),
            all(feature = "usb_helper", libnfc_driver_pn53x_usb)
        ))]
        assert!(!registry.is_empty());
        #[cfg(not(any(
            all(feature = "pcsc_helper", libnfc_driver_pcsc),
            all(feature = "pcsc_helper", libnfc_driver_acr122_pcsc),
            all(target_os = "linux", libnfc_driver_acr122s),
            all(target_os = "linux", libnfc_driver_arygon),
            all(feature = "usb_helper", libnfc_driver_acr122_usb),
            all(feature = "nci_helper", libnfc_driver_pn71xx),
            all(target_os = "linux", libnfc_driver_pn532_uart),
            all(target_os = "linux", libnfc_driver_pn532_spi),
            all(target_os = "linux", libnfc_driver_pn532_i2c),
            all(feature = "usb_helper", libnfc_driver_pn53x_usb)
        )))]
        assert!(registry.is_empty());
    }

    #[test]
    fn builtin_registration_order_matches_libnfc_orig_init_sequence() {
        let mut registry = DriverRegistry::new();
        register_builtin_drivers(&mut registry);

        let expected: Vec<&str> = {
            #[allow(unused_mut)]
            let mut expected = Vec::new();
            #[cfg(all(feature = "usb_helper", libnfc_driver_pn53x_usb))]
            expected.push("pn53x_usb");
            #[cfg(all(feature = "pcsc_helper", libnfc_driver_pcsc))]
            expected.push("pcsc");
            #[cfg(all(feature = "pcsc_helper", libnfc_driver_acr122_pcsc))]
            expected.push("acr122_pcsc");
            #[cfg(all(feature = "usb_helper", libnfc_driver_acr122_usb))]
            expected.push("acr122_usb");
            #[cfg(all(target_os = "linux", libnfc_driver_acr122s))]
            expected.push("acr122s");
            #[cfg(all(target_os = "linux", libnfc_driver_pn532_uart))]
            expected.push("pn532_uart");
            #[cfg(all(target_os = "linux", libnfc_driver_pn532_spi))]
            expected.push("pn532_spi");
            #[cfg(all(target_os = "linux", libnfc_driver_pn532_i2c))]
            expected.push("pn532_i2c");
            #[cfg(all(target_os = "linux", libnfc_driver_arygon))]
            expected.push("arygon");
            #[cfg(all(feature = "nci_helper", libnfc_driver_pn71xx))]
            expected.push("pn71xx");
            expected
        };

        assert_eq!(registry.registered_driver_names(), expected);
    }
}
