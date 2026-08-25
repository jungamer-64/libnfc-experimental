mod facade;

pub use facade::{Config, Context, ContextBuilder, DeviceDescriptor, ScanOutcome, Selector};
pub use proximate_driver::{
    ContextLoadError, DepOps, Device, DeviceHandle, DeviceOrigin, Driver, InfoOps, InitiatorIoOps,
    PassiveScanOps, Pn53xOps, PropertyOps, SessionOps, TargetIoOps, UnavailableDriver,
    UserDefinedDevice,
};
pub use proximate_types::{
    BaudRate, DepInfo, DepMode, Error, Modulation, ModulationType, Property, ScanType, Target,
    TargetInfo, version,
};
