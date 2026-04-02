mod caps;
mod connstring;
mod error;
mod metadata;
mod types;

pub const NFC_BUFSIZE_CONNSTRING: usize = 1024;

pub use caps::{DeviceCaps, DriverCaps};
pub use connstring::{
    ConnectionString, DecodedConnectionString, build_connstring, decode_connstring,
    decode_connstring_segments_bytes, extract_param_value_bytes, parse_connstring,
};
pub use error::{Error, PublicError};
pub use metadata::{device_error_message, version};
pub use types::{
    BaudRate, DepInfo, DepMode, Mode, Modulation, ModulationType, Property, ScanType, Target,
    TargetInfo,
};
