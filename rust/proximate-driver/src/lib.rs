mod context;
mod device;
mod driver;

pub use proximate_types::{
    BaudRate, ConnectionString, DecodedConnectionString, DepInfo, DepMode, DeviceCaps, DriverCaps,
    Error, Mode, Modulation, ModulationType, NFC_BUFSIZE_CONNSTRING, Property, ScanType, Target,
    TargetInfo, build_connstring, decode_connstring, decode_connstring_segments_bytes,
    device_error_message, extract_param_value_bytes, parse_connstring, version,
};

pub use context::{Context, ContextConfig, UserDefinedDevice};
#[doc(hidden)]
pub use context::{
    ContextDiagnostic, ContextDiagnosticCategory, ContextDiagnosticPriority, ContextLoadFailure,
    ContextLoadOutcome, ContextSources, set_test_conf_root,
};
pub use device::{
    ChipDebugOps, Device, DeviceBackend, DeviceMeta, InfoBackend, InitiatorBackend,
    InitiatorDevice, InitiatorOps, Logger, OpenedDevice, Pn53xBackend, Pn53xDevice,
    PropertyBackend, PropertyOps, TargetBackend, TargetDevice, TargetOps,
};
pub use driver::{Driver, DriverRegistry};

#[cfg(test)]
mod tests;
