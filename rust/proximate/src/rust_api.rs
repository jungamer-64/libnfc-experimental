mod backend;
mod caps;
mod connstring;
mod context;
mod device;
mod driver;
mod metadata;
mod types;

#[doc(hidden)]
pub use backend::{DeviceBackend, DriverBackend, wrap_device_backend, wrap_driver_backend};
pub use caps::{DeviceCaps, DriverCaps};
pub use connstring::{
    ConnectionString, DecodedConnectionString, build_connstring, decode_connstring,
    parse_connstring,
};
#[doc(hidden)]
pub use connstring::{decode_connstring_segments_bytes, extract_param_value_bytes};
#[doc(hidden)]
pub use context::ContextSources;
#[doc(hidden)]
pub use context::set_test_conf_root;
pub use context::{Context, ContextConfig, UserDefinedDevice};
#[doc(hidden)]
pub use context::{
    ContextDiagnostic, ContextDiagnosticCategory, ContextDiagnosticPriority, ContextLoadFailure,
    ContextLoadOutcome,
};
pub use device::{Device, Logger, OpenedDevice};
pub use driver::{Driver, DriverRegistry};
#[doc(hidden)]
pub use driver::{ListDevicesOutcome, RegisteredDriverSet};
#[doc(hidden)]
pub use metadata::device_error_message;
pub use metadata::version;
pub use types::{
    BaudRate, DepInfo, DepMode, Error, Mode, Modulation, ModulationType, Property, ScanType,
    Target, TargetInfo,
};

#[cfg(test)]
mod tests;
