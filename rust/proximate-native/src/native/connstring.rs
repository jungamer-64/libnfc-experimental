use proximate_driver::{ConnectionString, Error, decode_connstring};

#[cfg(all(
    any(feature = "driver-pn53x-usb", feature = "driver-acr122-usb"),
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
pub(super) mod usb;

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
    }
}
