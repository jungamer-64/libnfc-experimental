mod context;
mod device;
mod driver;

pub use proximate_types::{
    BaudRate, ConnectionString, DecodedConnectionString, DepInfo, DepMode, DeviceCaps, DriverCaps,
    Error, Mode, Modulation, ModulationType, NFC_BUFSIZE_CONNSTRING, Property, ScanType, Target,
    TargetInfo, build_connstring, decode_connstring, decode_connstring_segments_bytes,
    device_error_message, extract_param_value_bytes, parse_connstring, version,
};

pub use context::{Context, ContextConfig, ContextLoadError, UserDefinedDevice};
#[doc(hidden)]
pub mod diagnostics {
    pub use crate::context::{
        ContextDiagnostic, ContextDiagnosticCategory, ContextDiagnosticPriority,
        ContextLoadFailure, ContextLoadOutcome, load_context_from_dir_with_diagnostics,
        load_context_with_diagnostics,
    };
}
#[doc(hidden)]
pub use context::set_test_conf_root;
#[doc(hidden)]
pub use device::DeviceHandle;
pub use device::{
    Device, DeviceMeta, InfoBackend, InitiatorBackend, InitiatorDevice, Logger, Pn53xBackend,
    Pn53xDevice, PropertyBackend, TargetBackend, TargetDevice,
};
pub use driver::{Driver, DriverRegistry};

#[cfg(test)]
mod tests;
