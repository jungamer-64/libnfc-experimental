mod facade;

pub use facade::{Config, Context, ContextBuilder, DeviceDescriptor, Selector};
pub use proximate_driver::{
    ContextLoadError, DepOps, Device, DeviceOrigin, InfoOps, InitiatorIoOps, PassiveScanOps,
    Pn53xOps, PropertyOps, SessionOps, TargetIoOps, UserDefinedDevice,
};
pub use proximate_types::{
    BaudRate, DepInfo, DepMode, DeviceCaps, DriverCaps, Error, Modulation, ModulationType,
    Property, ScanType, Target, TargetInfo, version,
};
