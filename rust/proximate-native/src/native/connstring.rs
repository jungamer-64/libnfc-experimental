use proximate_driver::{ConnectionString, Error, decode_connstring};

use crate::usb::UsbDeviceInfo;
#[cfg(target_os = "windows")]
use crate::usb::WindowsUsbInstanceId;

const USB_BUS_NAME: &str = "usb";

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PathSpeedDescriptor {
    pub path: String,
    pub speed: u32,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PathDescriptor {
    pub path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum UsbSelector {
    Any,
    Numeric {
        bus: u8,
        address: u8,
    },
    #[cfg(target_os = "windows")]
    Instance(WindowsUsbInstanceId),
}

impl UsbSelector {
    fn matches(&self, device: &UsbDeviceInfo) -> bool {
        match self {
            Self::Any => true,
            Self::Numeric { bus, address } => device.numeric_locator() == (*bus, *address),
            #[cfg(target_os = "windows")]
            Self::Instance(instance_id) => device.instance_id() == instance_id,
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn decode_path_speed_descriptor(
    connstring: &ConnectionString,
    driver_name: &str,
    default_speed: u32,
) -> Result<PathSpeedDescriptor, Error> {
    let decoded = decode_connstring(connstring, driver_name, driver_name)?;
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

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn decode_path_descriptor(
    connstring: &ConnectionString,
    driver_name: &str,
) -> Result<PathDescriptor, Error> {
    let decoded = decode_connstring(connstring, driver_name, driver_name)?;
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

pub(crate) fn decode_usb_selector_for(
    connstring: &ConnectionString,
    driver_name: &str,
) -> Result<UsbSelector, Error> {
    let decoded = decode_connstring(connstring, driver_name, USB_BUS_NAME)?;
    match decoded.match_depth {
        0 => Err(Error::InvalidConnectionString(format!(
            "connstring '{}' does not match {driver_name}",
            connstring,
        ))),
        1 => Ok(UsbSelector::Any),
        3 => {
            let bus_value = decoded
                .param1
                .as_deref()
                .ok_or_else(|| Error::InvalidConnectionString("missing USB bus".into()))?;
            let device_value = decoded
                .param2
                .as_deref()
                .ok_or_else(|| Error::InvalidConnectionString("missing USB device".into()))?;
            #[cfg(target_os = "windows")]
            if bus_value.eq_ignore_ascii_case("instance") {
                return Ok(UsbSelector::Instance(decode_windows_instance_id(
                    device_value,
                )?));
            }

            Ok(UsbSelector::Numeric {
                bus: parse_usb_number("bus", bus_value)?,
                address: parse_usb_number("device", device_value)?,
            })
        }
        _ => Err(Error::InvalidConnectionString(format!(
            "invalid {driver_name} connstring '{}'",
            connstring,
        ))),
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn decode_usb_selector(connstring: &ConnectionString) -> Result<UsbSelector, Error> {
    decode_usb_selector_for(connstring, "pn53x_usb")
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn build_path_speed_connstring(
    driver_name: &str,
    path: &str,
    speed: u32,
) -> Result<ConnectionString, Error> {
    ConnectionString::new(format!("{driver_name}:{path}:{speed}"))
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn build_path_connstring(
    driver_name: &str,
    path: &str,
) -> Result<ConnectionString, Error> {
    ConnectionString::new(format!("{driver_name}:{path}"))
}

pub(crate) fn build_usb_connstring_for(
    driver_name: &str,
    device: &UsbDeviceInfo,
) -> Result<ConnectionString, Error> {
    #[cfg(not(target_os = "windows"))]
    {
        let (bus, address) = device.numeric_locator();
        build_numeric_usb_connstring_for(driver_name, bus, address)
    }

    #[cfg(target_os = "windows")]
    {
        let payload = encode_windows_instance_id(device.instance_id());
        ConnectionString::new(format!("{driver_name}:instance:{payload}"))
    }
}

#[cfg(any(not(target_os = "windows"), test))]
fn build_numeric_usb_connstring_for(
    driver_name: &str,
    bus: u8,
    address: u8,
) -> Result<ConnectionString, Error> {
    ConnectionString::new(format!("{driver_name}:{bus:03}:{address:03}"))
}

pub(crate) fn select_usb_candidate<T>(
    driver_name: &'static str,
    selector: &UsbSelector,
    candidates: impl IntoIterator<Item = (UsbDeviceInfo, T)>,
) -> Result<(UsbDeviceInfo, T), Error> {
    let mut matches = candidates
        .into_iter()
        .filter(|(device, _)| selector.matches(device));

    #[cfg(target_os = "windows")]
    let selected = match selector {
        UsbSelector::Any => matches.next(),
        UsbSelector::Numeric { .. } | UsbSelector::Instance(_) => {
            select_unique_usb_match(driver_name, matches)?
        }
    };
    #[cfg(not(target_os = "windows"))]
    let selected = matches.next();

    let Some(selected) = selected else {
        return Err(Error::DriverOpenFailed(format!(
            "no supported {driver_name} device is available"
        )));
    };
    Ok(selected)
}

#[cfg(target_os = "windows")]
fn select_unique_usb_match<T>(
    driver_name: &'static str,
    mut matches: impl Iterator<Item = T>,
) -> Result<Option<T>, Error> {
    let Some(selected) = matches.next() else {
        return Ok(None);
    };
    let match_count = 1 + matches.count();
    if match_count > 1 {
        return Err(Error::AmbiguousDeviceSelection {
            driver: driver_name.to_string(),
            matches: match_count,
        });
    }
    Ok(Some(selected))
}

fn parse_usb_number(kind: &str, value: &str) -> Result<u8, Error> {
    value
        .parse::<u8>()
        .map_err(|_| Error::InvalidConnectionString(format!("invalid USB {kind} number '{value}'")))
}

#[cfg(target_os = "windows")]
fn decode_windows_instance_id(payload: &str) -> Result<WindowsUsbInstanceId, Error> {
    if payload.is_empty() || !payload.len().is_multiple_of(4) {
        return Err(Error::InvalidConnectionString(
            "USB instance payload must contain fixed-width UTF-16 hexadecimal units".into(),
        ));
    }

    let mut units = Vec::with_capacity(payload.len() / 4);
    for offset in (0..payload.len()).step_by(4) {
        let unit = u16::from_str_radix(&payload[offset..offset + 4], 16).map_err(|_| {
            Error::InvalidConnectionString(
                "USB instance payload contains non-hexadecimal data".into(),
            )
        })?;
        if unit == 0 {
            return Err(Error::InvalidConnectionString(
                "USB instance payload contains a NUL unit".into(),
            ));
        }
        units.push(unit);
    }
    Ok(WindowsUsbInstanceId::from_units(units))
}

#[cfg(target_os = "windows")]
fn encode_windows_instance_id(instance_id: &WindowsUsbInstanceId) -> String {
    use std::fmt::Write;

    let mut payload = String::with_capacity(instance_id.units().len() * 4);
    for unit in instance_id.units() {
        write!(&mut payload, "{unit:04X}").expect("writing to a String cannot fail");
    }
    payload
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
        assert_eq!(decoded, UsbSelector::Any);
    }

    #[test]
    fn decodes_usb_selector_for_specific_device() {
        let connstring = ConnectionString::new("pn53x_usb:001:002").unwrap();
        let decoded = decode_usb_selector(&connstring).unwrap();
        assert_eq!(decoded, UsbSelector::Numeric { bus: 1, address: 2 });
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
            build_numeric_usb_connstring_for("pn53x_usb", 1, 2)
                .unwrap()
                .as_str(),
            "pn53x_usb:001:002"
        );
        assert_eq!(
            build_numeric_usb_connstring_for("acr122_usb", 1, 2)
                .unwrap()
                .as_str(),
            "acr122_usb:001:002"
        );
    }

    #[test]
    fn decode_usb_selector_for_supports_non_default_driver_name() {
        let connstring = ConnectionString::new("acr122_usb:001:002").unwrap();
        let decoded = decode_usb_selector_for(&connstring, "acr122_usb").unwrap();
        assert_eq!(decoded, UsbSelector::Numeric { bus: 1, address: 2 });
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_instance_selector_normalizes_ascii_case_and_preserves_utf16() {
        let connstring =
            ConnectionString::new("pn53x_usb:INSTANCE:007500730062005C00E900610062").unwrap();
        let decoded = decode_usb_selector(&connstring).unwrap();
        assert_eq!(
            decoded,
            UsbSelector::Instance(WindowsUsbInstanceId::from_units([
                u16::from(b'U'),
                u16::from(b'S'),
                u16::from(b'B'),
                u16::from(b'\\'),
                0x00E9,
                u16::from(b'A'),
                u16::from(b'B'),
            ]))
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_instance_payload_round_trips_scan_encoding_to_open_selector() {
        let instance_id = WindowsUsbInstanceId::from_units([
            u16::from(b'U'),
            u16::from(b'S'),
            u16::from(b'B'),
            u16::from(b'\\'),
            0x65E5,
            0x672C,
        ]);
        let payload = encode_windows_instance_id(&instance_id);
        assert_eq!(decode_windows_instance_id(&payload), Ok(instance_id));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_instance_selector_rejects_malformed_payloads() {
        for value in [
            "pn53x_usb:instance:",
            "pn53x_usb:instance:004",
            "pn53x_usb:instance:XXXX",
            "pn53x_usb:instance:0000",
        ] {
            let connstring = ConnectionString::new(value).unwrap();
            assert!(matches!(
                decode_usb_selector(&connstring),
                Err(Error::InvalidConnectionString(_))
            ));
        }

        assert!(matches!(
            ConnectionString::new(format!("pn53x_usb:instance:{}", "0041".repeat(256))),
            Err(Error::BufferTooSmall { .. })
        ));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_explicit_usb_selection_requires_one_match() {
        assert_eq!(
            select_unique_usb_match("pn53x_usb", [7].into_iter()),
            Ok(Some(7))
        );
        assert_eq!(
            select_unique_usb_match("pn53x_usb", [7, 8, 9].into_iter()),
            Err(Error::AmbiguousDeviceSelection {
                driver: "pn53x_usb".into(),
                matches: 3,
            })
        );
    }
}
