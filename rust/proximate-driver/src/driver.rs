use super::device::NamedOpenedDevice;
use crate::{
    ConnectionString, Context, ContextConfig, Device, DriverCaps, Error, OpenedDevice, ScanType,
};

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
    fn scan(&self, context: &Context) -> Result<Vec<ConnectionString>, Error>;
    fn open(
        &self,
        context: &Context,
        connstring: &ConnectionString,
    ) -> Result<Box<dyn OpenedDevice>, Error>;
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListDevicesOutcome {
    pub devices: Vec<ConnectionString>,
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
    pub fn list_devices_outcome(&self, context: &Context) -> Result<ListDevicesOutcome, Error> {
        let mut devices = Vec::new();

        for configured in &context.config.user_defined_devices {
            if configured.optional && self.open(context, Some(&configured.connstring)).is_err() {
                continue;
            }
            devices.push(configured.connstring.clone());
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

    pub fn list_devices(&self, context: &Context) -> Result<Vec<ConnectionString>, Error> {
        Ok(self.list_devices_outcome(context)?.devices)
    }

    fn first_available_connstring(
        &self,
        context: &Context,
    ) -> Result<Option<ConnectionString>, Error> {
        for configured in &context.config.user_defined_devices {
            if configured.optional && self.open(context, Some(&configured.connstring)).is_err() {
                continue;
            }
            return Ok(Some(configured.connstring.clone()));
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

            if let Some(connstring) = driver.scan(context)?.into_iter().next() {
                return Ok(Some(connstring));
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
            self.first_available_connstring(context)?
                .ok_or_else(|| Error::DriverNotFound("no device available".to_string()))?
        };

        let request_is_usb = requested.as_str().starts_with("usb");
        let override_name = user_defined_device_name(context, &requested).map(str::to_owned);
        let mut last_error = None;

        for driver in self.drivers.iter().rev() {
            if !driver.caps().contains(DriverCaps::OPEN) {
                continue;
            }
            if !driver_matches_connstring(driver.as_ref(), &requested) {
                continue;
            }

            match driver.open(context, &requested) {
                Ok(handle) => {
                    let handle = if let Some(name) = override_name.clone() {
                        Box::new(NamedOpenedDevice::new(name, handle)) as Box<dyn OpenedDevice>
                    } else {
                        handle
                    };
                    return Ok(Device::new(handle));
                }
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

fn driver_matches_connstring(driver: &dyn Driver, connstring: &ConnectionString) -> bool {
    let name = driver.name();
    connstring.as_str().starts_with(name)
        || (connstring.as_str().starts_with("usb") && name.ends_with("_usb"))
}
