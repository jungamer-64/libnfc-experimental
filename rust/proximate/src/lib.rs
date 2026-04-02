mod facade;

pub use facade::{
    Config, ConfiguredDevice, Context, ContextBuilder, Device, DeviceInfo, DeviceSelector,
    InitiatorDevice, Pn53xDevice, TargetDevice,
};
pub use proximate_driver::ContextLoadError;
pub use proximate_types::{
    BaudRate, DepInfo, DepMode, DeviceCaps, DriverCaps, Error, Modulation, ModulationType,
    Property, ScanType, Target, TargetInfo, version,
};
