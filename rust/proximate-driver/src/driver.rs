use crate::{
    ConnectionString, Context, ContextConfig, Device, DeviceCaps, DeviceHandle, DriverCaps, Error,
    ScanType,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeviceOrigin {
    UserDefined,
    Driver(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredDevice {
    pub display_name: String,
    pub connstring: ConnectionString,
    pub caps: Option<DeviceCaps>,
    pub scan_type: ScanType,
    pub exclusive: bool,
    pub origin: DeviceOrigin,
}

pub trait Driver: Send + Sync {
    fn name(&self) -> &str;
    fn scan_type(&self) -> ScanType;
    fn caps(&self) -> DriverCaps {
        let mut caps = DriverCaps::OPEN;
        if self.scan_type() != ScanType::NotAvailable {
            caps |= DriverCaps::SCAN;
        }
        caps
    }
    fn origin(&self) -> DeviceOrigin {
        DeviceOrigin::Driver(self.name().to_string())
    }
    fn exclusive(&self) -> bool {
        false
    }
    fn accepts_family(&self, family: &str) -> bool {
        family == self.name() || (family == "usb" && self.name().ends_with("_usb"))
    }
    fn describe_discovered(
        &self,
        display_name: String,
        connstring: ConnectionString,
        caps: Option<DeviceCaps>,
    ) -> DiscoveredDevice {
        DiscoveredDevice {
            display_name,
            connstring,
            caps,
            scan_type: self.scan_type(),
            exclusive: self.exclusive(),
            origin: self.origin(),
        }
    }
    fn scan(&self, context: &Context) -> Result<Vec<DiscoveredDevice>, Error>;
    fn open(
        &self,
        context: &Context,
        connstring: &ConnectionString,
    ) -> Result<Box<dyn DeviceHandle>, Error>;
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListDevicesOutcome {
    pub devices: Vec<DiscoveredDevice>,
    pub warn_manual_selection: bool,
}

#[derive(Default)]
pub struct DriverRegistry {
    drivers: Vec<Box<dyn Driver>>,
}

impl DriverRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_driver(&mut self, driver: Box<dyn Driver>) {
        self.drivers.push(driver);
    }

    pub fn is_empty(&self) -> bool {
        self.drivers.is_empty()
    }

    #[doc(hidden)]
    pub fn registered_driver_names(&self) -> Vec<&str> {
        self.drivers.iter().map(|driver| driver.name()).collect()
    }

    #[doc(hidden)]
    pub fn list_devices_outcome(&self, context: &Context) -> Result<ListDevicesOutcome, Error> {
        let mut devices = Vec::new();

        for configured in &context.config.user_defined_devices {
            if configured.optional && self.open(context, Some(&configured.connstring)).is_err() {
                continue;
            }
            devices.push(DiscoveredDevice {
                display_name: configured.name.clone(),
                connstring: configured.connstring.clone(),
                caps: None,
                scan_type: ScanType::NotAvailable,
                exclusive: false,
                origin: DeviceOrigin::UserDefined,
            });
        }

        if !context.config.allow_autoscan {
            return Ok(ListDevicesOutcome {
                devices,
                warn_manual_selection: context.config.user_defined_devices.is_empty(),
            });
        }

        for driver in self.drivers.iter().rev() {
            if !driver.caps().contains(DriverCaps::SCAN) {
                continue;
            }
            if !scan_allowed_for_driver(&context.config, driver.as_ref()) {
                continue;
            }

            let mut scanned = driver.scan(context)?;
            devices.append(&mut scanned);
        }

        Ok(ListDevicesOutcome {
            devices,
            warn_manual_selection: false,
        })
    }

    pub fn list_devices(&self, context: &Context) -> Result<Vec<DiscoveredDevice>, Error> {
        Ok(self.list_devices_outcome(context)?.devices)
    }

    fn first_available_device(&self, context: &Context) -> Result<Option<DiscoveredDevice>, Error> {
        for configured in &context.config.user_defined_devices {
            if configured.optional && self.open(context, Some(&configured.connstring)).is_err() {
                continue;
            }
            return Ok(Some(DiscoveredDevice {
                display_name: configured.name.clone(),
                connstring: configured.connstring.clone(),
                caps: None,
                scan_type: ScanType::NotAvailable,
                exclusive: false,
                origin: DeviceOrigin::UserDefined,
            }));
        }

        if !context.config.allow_autoscan {
            return Ok(None);
        }

        for driver in self.drivers.iter().rev() {
            if !driver.caps().contains(DriverCaps::SCAN) {
                continue;
            }
            if !scan_allowed_for_driver(&context.config, driver.as_ref()) {
                continue;
            }

            if let Some(device) = driver.scan(context)?.into_iter().next() {
                return Ok(Some(device));
            }
        }

        Ok(None)
    }

    pub fn open(
        &self,
        context: &Context,
        connstring: Option<&ConnectionString>,
    ) -> Result<Device, Error> {
        let requested = if let Some(connstring) = connstring {
            connstring.clone()
        } else {
            self.first_available_device(context)?
                .ok_or_else(|| Error::DriverNotFound("no device available".to_string()))?
                .connstring
        };

        let request_is_usb = requested.family() == "usb";
        let override_name = user_defined_device_name(context, &requested).map(str::to_owned);
        let mut last_error = None;
        let requested_family = requested.family().to_string();

        for driver in self.drivers.iter().rev() {
            if !driver.caps().contains(DriverCaps::OPEN) {
                continue;
            }
            if !driver.accepts_family(&requested_family) {
                continue;
            }

            match driver.open(context, &requested) {
                Ok(handle) => return Ok(Device::new(handle, override_name.clone())),
                Err(error) if request_is_usb => {
                    last_error = Some(error);
                }
                Err(error) => return Err(error),
            }
        }

        Err(last_error.unwrap_or_else(|| Error::DriverNotFound(requested.as_str().to_string())))
    }
}

fn user_defined_device_name<'a>(
    context: &'a Context,
    connstring: &ConnectionString,
) -> Option<&'a str> {
    context
        .config
        .user_defined_devices
        .iter()
        .find(|device| device.connstring == *connstring)
        .map(|device| device.name.as_str())
}

fn scan_allowed_for_driver(context: &ContextConfig, driver: &dyn Driver) -> bool {
    match driver.scan_type() {
        ScanType::NotIntrusive => true,
        ScanType::Intrusive => context.allow_intrusive_scan,
        ScanType::NotAvailable => false,
    }
}
