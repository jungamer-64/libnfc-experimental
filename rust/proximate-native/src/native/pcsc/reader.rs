use super::super::acr122;
use super::{NFC_ENOTSUCHDEV, PcscBackend, device_error, invalid_connection};
use proximate_driver::{ConnectionString, Error, decode_connstring};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReaderFilter {
    Generic,
    Acr122,
}

fn reader_matches(filter: ReaderFilter, reader: &str) -> bool {
    match filter {
        ReaderFilter::Generic => !acr122::is_pcsc_reader_name(reader),
        ReaderFilter::Acr122 => acr122::is_pcsc_reader_name(reader),
    }
}

pub(super) fn scan_matching_readers(
    backend: &dyn PcscBackend,
    driver_name: &str,
    filter: ReaderFilter,
) -> Result<Vec<ConnectionString>, Error> {
    let readers = match backend.list_readers_owned() {
        Ok(readers) => readers,
        Err(_) => return Ok(Vec::new()),
    };
    readers
        .into_iter()
        .filter(|reader| reader_matches(filter, reader))
        .map(|reader| ConnectionString::new(format!("{driver_name}:{reader}")))
        .collect()
}

fn parse_reader_index(value: &str) -> Option<usize> {
    if value.is_empty() || value.len() > 4 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse::<usize>().ok()
}

pub(super) fn resolve_reader(
    backend: &dyn PcscBackend,
    connstring: &ConnectionString,
    driver_name: &str,
    filter: ReaderFilter,
) -> Result<(String, ConnectionString), Error> {
    let decoded = decode_connstring(connstring, driver_name, "pcsc")?;
    if decoded.match_depth < 1 {
        return Err(invalid_connection(format!(
            "{driver_name} connection string does not match"
        )));
    }

    if decoded.match_depth == 1 {
        let devices = scan_matching_readers(backend, driver_name, filter)?;
        let Some(resolved) = devices.into_iter().next() else {
            return Err(device_error("pcsc_scan", NFC_ENOTSUCHDEV));
        };
        let resolved_decoded = decode_connstring(&resolved, driver_name, "pcsc")?;
        let reader = resolved_decoded
            .param1
            .ok_or_else(|| invalid_connection("resolved reader name is missing"))?;
        return Ok((reader, resolved));
    }

    let requested = decoded
        .param1
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid_connection("reader name is missing"))?;
    if let Some(index) = parse_reader_index(&requested) {
        let devices = scan_matching_readers(backend, driver_name, filter)?;
        let Some(resolved) = devices.into_iter().nth(index) else {
            return Err(device_error("pcsc_scan", NFC_ENOTSUCHDEV));
        };
        let resolved_decoded = decode_connstring(&resolved, driver_name, "pcsc")?;
        let reader = resolved_decoded
            .param1
            .ok_or_else(|| invalid_connection("resolved reader name is missing"))?;
        return Ok((reader, resolved));
    }

    if !reader_matches(filter, &requested) {
        return Err(device_error("pcsc_open", NFC_ENOTSUCHDEV));
    }

    Ok((
        requested.clone(),
        ConnectionString::new(format!("{driver_name}:{requested}"))?,
    ))
}
