use proximate_driver::{
    BaudRate, ConnectionString, Context, DeviceCaps, DeviceHandle, DeviceMeta, Driver, Error,
    InfoBackend, InitiatorBackend, Mode, Modulation, ModulationType, Pn53xBackend, Property,
    PropertyBackend, ScanType, Target, TargetBackend, TargetInfo, device_error_message,
};
use std::fmt;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

mod apdu;
mod backend;
mod device;
mod driver;
#[cfg(test)]
mod fake;
mod reader;
#[cfg(test)]
mod tests;
mod types;

use self::apdu::{
    attr_to_string, command_response_data, icc_type_matches, is_feitian_reader,
    iso14443a_atr_valid, iso14443a_uid_length_valid, iso14443b_atr_valid,
    iso14443b_uid_length_valid,
};
pub(crate) use self::backend::SystemPcscBackend;
use self::backend::stringify_pcsc_error;
pub(crate) use self::device::PcscDevice;
pub(crate) use self::driver::PcscDriver;
#[cfg(test)]
pub(crate) use self::fake::{FakeCardState, FakePcscBackend, FakePcscCard};
pub(crate) use self::reader::{ReaderFilter, resolve_reader, scan_matching_readers};
use self::types::{PcscAttribute, PcscDisposition};
pub(crate) use self::types::{
    PcscBackend, PcscCard, PcscCardStatus, PcscProtocol, PcscProtocols, PcscShareMode,
};

const NFC_SUCCESS: i32 = 0;
const NFC_EIO: i32 = -1;
const NFC_EINVARG: i32 = -2;
const NFC_EDEVNOTSUPP: i32 = -3;
const NFC_ENOTSUCHDEV: i32 = -4;
const NFC_ESOFT: i32 = -80;
const NFC_ECHIP: i32 = -90;

const PCSC_DRIVER_NAME: &str = "pcsc";

const ICC_TYPE_UNKNOWN: u8 = 0;
const ICC_TYPE_14443A: u8 = 5;
const ICC_TYPE_14443B: u8 = 6;

const PCSC_SUPPORTED_BAUD_RATES: &[BaudRate] = &[BaudRate::Br106, BaudRate::Br424];
const PCSC_SUPPORTED_MODULATIONS: &[ModulationType] =
    &[ModulationType::Iso14443A, ModulationType::Iso14443B];

fn device_error(operation: &'static str, code: i32) -> Error {
    Error::DeviceOperationFailed { operation, code }
}

fn invalid_connection(message: impl Into<String>) -> Error {
    Error::InvalidConnectionString(message.into())
}
