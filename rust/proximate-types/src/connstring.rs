use crate::{Error, NFC_BUFSIZE_CONNSTRING};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionString(String);

impl ConnectionString {
    pub fn new(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        validate_connstring(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ConnectionString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedConnectionString {
    pub match_depth: i32,
    pub param1: Option<String>,
    pub param2: Option<String>,
}

pub fn parse_connstring(connstring: &str, prefix: &str, param_name: &str) -> Result<String, Error> {
    validate_text(connstring, "connstring")?;
    validate_text(prefix, "prefix")?;
    validate_text(param_name, "param_name")?;

    extract_param_value_bytes(
        connstring.as_bytes(),
        prefix.as_bytes(),
        param_name.as_bytes(),
    )
    .map(|value| String::from_utf8_lossy(value).into_owned())
    .map_err(Error::InvalidConnectionString)
}

pub fn build_connstring(
    driver_name: &str,
    param_name: &str,
    param_value: &str,
) -> Result<ConnectionString, Error> {
    validate_text(driver_name, "driver_name")?;
    validate_text(param_name, "param_name")?;
    validate_text(param_value, "param_value")?;

    let result = build_connstring_bytes(
        driver_name.as_bytes(),
        param_name.as_bytes(),
        param_value.as_bytes(),
    );
    if result.len() >= NFC_BUFSIZE_CONNSTRING {
        return Err(Error::BufferTooSmall {
            needed: result.len() + 1,
            available: NFC_BUFSIZE_CONNSTRING,
        });
    }

    ConnectionString::new(String::from_utf8_lossy(&result).into_owned())
}

pub fn decode_connstring(
    connstring: &ConnectionString,
    driver_name: &str,
    bus_name: &str,
) -> Result<DecodedConnectionString, Error> {
    validate_text(connstring.as_str(), "connstring")?;
    validate_text(driver_name, "driver_name")?;
    validate_text(bus_name, "bus_name")?;

    let (match_depth, param1, param2) = decode_connstring_segments(
        connstring.as_str().as_bytes(),
        driver_name.as_bytes(),
        bus_name.as_bytes(),
    )
    .map(|(depth, first, second)| {
        (
            depth,
            first.map(|value| String::from_utf8_lossy(value).into_owned()),
            second.map(|value| String::from_utf8_lossy(value).into_owned()),
        )
    })
    .unwrap_or((0, None, None));

    Ok(DecodedConnectionString {
        match_depth,
        param1,
        param2,
    })
}

fn validate_connstring(value: &str) -> Result<(), Error> {
    if value.is_empty() {
        return Err(Error::InvalidConnectionString(
            "connection string cannot be empty".to_string(),
        ));
    }
    if value.len() >= NFC_BUFSIZE_CONNSTRING {
        return Err(Error::BufferTooSmall {
            needed: value.len() + 1,
            available: NFC_BUFSIZE_CONNSTRING,
        });
    }
    if value.as_bytes().iter().any(|byte| byte.is_ascii_control()) {
        return Err(Error::InvalidConnectionString(
            "connection string contains control characters".to_string(),
        ));
    }
    Ok(())
}

fn validate_text(value: &str, context: &'static str) -> Result<(), Error> {
    if value.as_bytes().contains(&0) {
        return Err(Error::InvalidEncoding(context));
    }
    Ok(())
}

fn split_at_first(data: &[u8], delimiter: u8) -> (&[u8], Option<&[u8]>) {
    if let Some(position) = data.iter().position(|&b| b == delimiter) {
        (&data[..position], Some(&data[position + 1..]))
    } else {
        (data, None)
    }
}

#[doc(hidden)]
pub fn extract_param_value_bytes<'a>(
    conn_bytes: &'a [u8],
    prefix_bytes: &[u8],
    param_name_bytes: &[u8],
) -> Result<&'a [u8], String> {
    if conn_bytes.len() < prefix_bytes.len() || !conn_bytes.starts_with(prefix_bytes) {
        return Err(format!(
            "Connstring '{}' does not match prefix '{}'",
            String::from_utf8_lossy(conn_bytes),
            String::from_utf8_lossy(prefix_bytes)
        ));
    }

    let mut param_section = &conn_bytes[prefix_bytes.len()..];
    if param_section.first().copied() == Some(b':') {
        param_section = &param_section[1..];
    }

    let mut pattern = Vec::with_capacity(param_name_bytes.len() + 1);
    pattern.extend_from_slice(param_name_bytes);
    pattern.push(b'=');

    let value_start = param_section
        .windows(pattern.len())
        .position(|window| window == pattern.as_slice())
        .map(|index| index + pattern.len())
        .ok_or_else(|| {
            format!(
                "Parameter '{}' not found in connstring",
                String::from_utf8_lossy(param_name_bytes)
            )
        })?;

    let value_slice = &param_section[value_start..];
    let value_end = value_slice
        .iter()
        .position(|&byte| byte == b':')
        .unwrap_or(value_slice.len());
    Ok(&value_slice[..value_end])
}

fn build_connstring_bytes(driver_name: &[u8], param_name: &[u8], param_value: &[u8]) -> Vec<u8> {
    let mut result =
        Vec::with_capacity(driver_name.len() + 1 + param_name.len() + 1 + param_value.len());
    result.extend_from_slice(driver_name);
    result.push(b':');
    result.extend_from_slice(param_name);
    result.push(b'=');
    result.extend_from_slice(param_value);
    result
}

type DecodedConnstringSegments<'a> = (i32, Option<&'a [u8]>, Option<&'a [u8]>);

fn decode_connstring_segments<'a>(
    connstring: &'a [u8],
    driver_name: &[u8],
    bus_name: &[u8],
) -> Option<DecodedConnstringSegments<'a>> {
    let (first_segment, remainder) = split_at_first(connstring, b':');
    if first_segment != driver_name && first_segment != bus_name {
        return None;
    }

    let mut result = 1;
    let mut param1 = None;
    let mut param2 = None;

    if let Some(level1) = remainder {
        let (second, remainder2) = split_at_first(level1, b':');
        param1 = Some(second);
        result = 2;

        if let Some(level2) = remainder2 {
            let (third, _) = split_at_first(level2, b':');
            param2 = Some(third);
            result = 3;
        }
    }

    Some((result, param1, param2))
}

#[doc(hidden)]
pub fn decode_connstring_segments_bytes<'a>(
    connstring: &'a [u8],
    driver_name: &[u8],
    bus_name: &[u8],
) -> Option<DecodedConnstringSegments<'a>> {
    decode_connstring_segments(connstring, driver_name, bus_name)
}
