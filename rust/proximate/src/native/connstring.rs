use crate::rust_api::{ConnectionString, Error};

const USB_DRIVER_NAME: &str = "pn53x_usb";
const USB_BUS_NAME: &str = "usb";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PathSpeedDescriptor {
    pub path: String,
    pub speed: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PathDescriptor {
    pub path: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UsbSelector {
    pub bus: Option<u8>,
    pub device: Option<u8>,
}

pub(crate) fn decode_path_speed_descriptor(
    connstring: &ConnectionString,
    driver_name: &str,
    default_speed: u32,
) -> Result<PathSpeedDescriptor, Error> {
    let decoded = crate::decode_connstring(connstring, driver_name, driver_name)?;
    if decoded.match_depth < 2 {
        return Err(Error::InvalidConnectionString(format!(
            "{driver_name} connstring requires a path"
        )));
    }

    let path = decoded
        .param1
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            Error::InvalidConnectionString(format!("{driver_name} connstring path is empty"))
        })?;
    let speed = match decoded.param2 {
        Some(value) if !value.is_empty() => value
            .parse::<u32>()
            .map_err(|_| Error::InvalidConnectionString(format!("invalid speed '{value}'")))?,
        _ => default_speed,
    };

    Ok(PathSpeedDescriptor { path, speed })
}

pub(crate) fn decode_path_descriptor(
    connstring: &ConnectionString,
    driver_name: &str,
) -> Result<PathDescriptor, Error> {
    let decoded = crate::decode_connstring(connstring, driver_name, driver_name)?;
    if decoded.match_depth < 2 {
        return Err(Error::InvalidConnectionString(format!(
            "{driver_name} connstring requires a path"
        )));
    }

    let path = decoded
        .param1
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            Error::InvalidConnectionString(format!("{driver_name} connstring path is empty"))
        })?;

    Ok(PathDescriptor { path })
}

pub(crate) fn decode_usb_selector(connstring: &ConnectionString) -> Result<UsbSelector, Error> {
    let decoded = crate::decode_connstring(connstring, USB_DRIVER_NAME, USB_BUS_NAME)?;
    match decoded.match_depth {
        0 => Err(Error::InvalidConnectionString(format!(
            "connstring '{}' does not match pn53x_usb",
            connstring
        ))),
        1 => Ok(UsbSelector {
            bus: None,
            device: None,
        }),
        3 => {
            let bus_value = decoded
                .param1
                .as_deref()
                .ok_or_else(|| Error::InvalidConnectionString("missing USB bus".into()))?;
            let device_value = decoded
                .param2
                .as_deref()
                .ok_or_else(|| Error::InvalidConnectionString("missing USB device".into()))?;
            Ok(UsbSelector {
                bus: Some(parse_usb_number("bus", bus_value)?),
                device: Some(parse_usb_number("device", device_value)?),
            })
        }
        _ => Err(Error::InvalidConnectionString(format!(
            "invalid pn53x_usb connstring '{}'",
            connstring
        ))),
    }
}

pub(crate) fn build_path_speed_connstring(
    driver_name: &str,
    path: &str,
    speed: u32,
) -> Result<ConnectionString, Error> {
    ConnectionString::new(format!("{driver_name}:{path}:{speed}"))
}

pub(crate) fn build_path_connstring(
    driver_name: &str,
    path: &str,
) -> Result<ConnectionString, Error> {
    ConnectionString::new(format!("{driver_name}:{path}"))
}

pub(crate) fn build_usb_connstring(bus: u8, device: u8) -> Result<ConnectionString, Error> {
    ConnectionString::new(format!("{USB_DRIVER_NAME}:{bus:03}:{device:03}"))
}

fn parse_usb_number(kind: &str, value: &str) -> Result<u8, Error> {
    value
        .parse::<u8>()
        .map_err(|_| Error::InvalidConnectionString(format!("invalid USB {kind} number '{value}'")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_path_speed_descriptor() {
        let connstring = ConnectionString::new("pn532_uart:/dev/ttyUSB0:230400").unwrap();
        let decoded = decode_path_speed_descriptor(&connstring, "pn532_uart", 115_200).unwrap();
        assert_eq!(decoded.path, "/dev/ttyUSB0");
        assert_eq!(decoded.speed, 230_400);
    }

    #[test]
    fn path_speed_descriptor_uses_default_speed() {
        let connstring = ConnectionString::new("pn532_spi:/dev/spidev0.0").unwrap();
        let decoded = decode_path_speed_descriptor(&connstring, "pn532_spi", 1_000_000).unwrap();
        assert_eq!(decoded.path, "/dev/spidev0.0");
        assert_eq!(decoded.speed, 1_000_000);
    }

    #[test]
    fn decodes_usb_selector_for_implicit_first_device() {
        let connstring = ConnectionString::new("usb").unwrap();
        let decoded = decode_usb_selector(&connstring).unwrap();
        assert_eq!(
            decoded,
            UsbSelector {
                bus: None,
                device: None,
            }
        );
    }

    #[test]
    fn decodes_usb_selector_for_specific_device() {
        let connstring = ConnectionString::new("pn53x_usb:001:002").unwrap();
        let decoded = decode_usb_selector(&connstring).unwrap();
        assert_eq!(
            decoded,
            UsbSelector {
                bus: Some(1),
                device: Some(2),
            }
        );
    }

    #[test]
    fn decodes_path_descriptor() {
        let connstring = ConnectionString::new("pn532_i2c:/dev/i2c-1").unwrap();
        let decoded = decode_path_descriptor(&connstring, "pn532_i2c").unwrap();
        assert_eq!(decoded.path, "/dev/i2c-1");
    }

    #[test]
    fn build_helpers_preserve_expected_formats() {
        assert_eq!(
            build_path_speed_connstring("pn532_uart", "/dev/ttyUSB0", 115_200)
                .unwrap()
                .as_str(),
            "pn532_uart:/dev/ttyUSB0:115200"
        );
        assert_eq!(
            build_path_connstring("pn532_spi", "/dev/spidev0.0")
                .unwrap()
                .as_str(),
            "pn532_spi:/dev/spidev0.0"
        );
        assert_eq!(
            build_usb_connstring(1, 2).unwrap().as_str(),
            "pn53x_usb:001:002"
        );
    }
}
