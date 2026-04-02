// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Pure Rust API surface for libnfc experiments.

mod native;
pub mod rust_api;

pub(crate) const NFC_BUFSIZE_CONNSTRING: usize = 1024;

pub use rust_api::{
    BaudRate, ConnectionString, Context, ContextConfig, DecodedConnectionString, DepInfo, DepMode,
    Device, DeviceCaps, Driver, DriverCaps, DriverRegistry, Error, Logger, Mode, Modulation,
    ModulationType, OpenedDevice, Property, ScanType, Target, TargetInfo, UserDefinedDevice,
    build_connstring, decode_connstring, parse_connstring, version,
};
