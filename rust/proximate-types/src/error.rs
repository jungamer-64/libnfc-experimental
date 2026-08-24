use thiserror::Error;

#[derive(Debug, Error, Clone, Eq, PartialEq)]
pub enum Error {
    #[error("invalid argument: {0}")]
    InvalidArgument(&'static str),
    #[error("invalid encoding in {0}")]
    InvalidEncoding(&'static str),
    #[error("buffer too small: needed {needed}, available {available}")]
    BufferTooSmall { needed: usize, available: usize },
    #[error("invalid connection string: {0}")]
    InvalidConnectionString(String),
    #[error("driver not found: {0}")]
    DriverNotFound(String),
    #[error("driver open failed: {0}")]
    DriverOpenFailed(String),
    #[error("missing capability: {0}")]
    MissingCapability(&'static str),
    #[error("unsupported operation: {0}")]
    UnsupportedOperation(&'static str),
    #[error("operation timed out: {0}")]
    Timeout(&'static str),
    #[error("operation was aborted: {0}")]
    Aborted(&'static str),
    #[error("target was released during operation: {0}")]
    TargetReleased(&'static str),
    #[error("RF transmission failed during operation: {0}")]
    RfTransmission(&'static str),
    #[error("authentication failed during operation: {0}")]
    Authentication(&'static str),
    #[error("device I/O failed during operation: {0}")]
    Io(&'static str),
    #[error("device chip rejected operation: {0}")]
    Chip(&'static str),
    #[error("device state is unknown after operation {operation}: {cause}")]
    OutcomeUnknown {
        operation: &'static str,
        cause: Box<Error>,
    },
    #[error("operation failed and recovery also failed")]
    RecoveryFailed {
        operation: Box<Error>,
        recovery: Box<Error>,
    },
    #[error("device operation {operation} failed with status {code}")]
    DeviceOperationFailed { operation: &'static str, code: i32 },
}

impl Error {
    pub fn device_code(&self) -> Option<i32> {
        match self {
            Self::DeviceOperationFailed { code, .. } => Some(*code),
            _ => None,
        }
    }
}

pub type PublicError = Error;
