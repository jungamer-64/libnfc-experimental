/*-
 * Free/Libre Near Field Communication (NFC) library
 *
 * Libnfc historical contributors:
 * Copyright (C) 2019      Frank Morgner
 * See AUTHORS file for a more comprehensive list of contributors.
 * Additional contributors of this file:
 * Copyright (C) 2020      Feitian Technologies
 * This program is free software: you can redistribute it and/or modify it
 * under the terms of the GNU Lesser General Public License as published by the
 * Free Software Foundation, either version 3 of the License, or (at your
 * option) any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
 * more details.
 *
 * You should have received a copy of the GNU Lesser General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>
 */

// Derived from libnfc's generic PC/SC driver. Reader adaptation and APDU
// handling are implemented here in Rust.

use proximate_driver::Error;
#[cfg(feature = "driver-pcsc")]
use proximate_driver::{
    BaudRate, ConnectionString, Context, DeviceHandle, DeviceMeta, Driver, InfoBackend,
    InitiatorBackend, Mode, Modulation, ModulationType, OperationTimeout, Pn53xBackend,
    PollIterations, PollPeriod, Property, PropertyBackend, ScanType, Target, TargetBackend,
    TargetInfo, TimeoutProperty, device_error_message,
};
#[cfg(feature = "driver-pcsc")]
use std::fmt;
#[cfg(feature = "driver-pcsc")]
use std::sync::Arc;
#[cfg(feature = "driver-pcsc")]
use std::thread;
#[cfg(feature = "driver-pcsc")]
use std::time::Duration;

#[cfg(feature = "driver-pcsc")]
mod apdu;
mod backend;
#[cfg(feature = "driver-pcsc")]
mod device;
#[cfg(feature = "driver-pcsc")]
mod driver;
#[cfg(all(test, feature = "driver-pcsc"))]
mod fake;
mod reader;
#[cfg(all(test, feature = "driver-pcsc"))]
mod tests;
mod types;

#[cfg(feature = "driver-pcsc")]
use self::apdu::{
    attr_to_string, command_response_data, icc_type_matches, is_feitian_reader,
    iso14443a_atr_valid, iso14443a_uid_length_valid, iso14443b_atr_valid,
    iso14443b_uid_length_valid,
};
pub(crate) use self::backend::SystemPcscBackend;
#[cfg(feature = "driver-pcsc")]
use self::backend::stringify_pcsc_error;
#[cfg(feature = "driver-pcsc")]
pub(crate) use self::device::PcscDevice;
#[cfg(feature = "driver-pcsc")]
pub(crate) use self::driver::PcscDriver;
pub(crate) use self::reader::{ReaderFilter, ReaderScan, resolve_reader, scan_matching_readers};
#[cfg(feature = "driver-pcsc")]
pub(crate) use self::types::PcscAttribute;
pub(crate) use self::types::{
    PcscBackend, PcscCard, PcscCardStatus, PcscProtocol, PcscProtocols, PcscShareMode,
};

#[cfg(feature = "driver-pcsc")]
const NFC_SUCCESS: i32 = 0;
#[cfg(feature = "driver-pcsc")]
const NFC_EIO: i32 = -1;
#[cfg(feature = "driver-pcsc")]
const NFC_EINVARG: i32 = -2;
#[cfg(feature = "driver-pcsc")]
const NFC_EDEVNOTSUPP: i32 = -3;
const NFC_ENOTSUCHDEV: i32 = -4;
#[cfg(feature = "driver-pcsc")]
const NFC_ESOFT: i32 = -80;
#[cfg(feature = "driver-pcsc")]
const NFC_ECHIP: i32 = -90;

#[cfg(feature = "driver-pcsc")]
const PCSC_DRIVER_NAME: &str = "pcsc";

#[cfg(feature = "driver-pcsc")]
const ICC_TYPE_UNKNOWN: u8 = 0;
#[cfg(feature = "driver-pcsc")]
const ICC_TYPE_14443A: u8 = 5;
#[cfg(feature = "driver-pcsc")]
const ICC_TYPE_14443B: u8 = 6;

#[cfg(feature = "driver-pcsc")]
const PCSC_SUPPORTED_BAUD_RATES: &[BaudRate] = &[BaudRate::Br106, BaudRate::Br424];
#[cfg(feature = "driver-pcsc")]
const PCSC_SUPPORTED_MODULATIONS: &[ModulationType] =
    &[ModulationType::Iso14443A, ModulationType::Iso14443B];

fn device_error(operation: &'static str, code: i32) -> Error {
    Error::DeviceOperationFailed { operation, code }
}

fn invalid_connection(message: impl Into<String>) -> Error {
    Error::InvalidConnectionString(message.into())
}
