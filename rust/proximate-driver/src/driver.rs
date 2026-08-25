use crate::{ConnectionString, Context, ContextConfig, Device, DeviceHandle, Error, ScanType};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeviceOrigin {
    UserDefined,
    Driver(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredDevice {
    pub display_name: String,
    pub connstring: ConnectionString,
    pub scan_type: ScanType,
    pub exclusive: bool,
    pub origin: DeviceOrigin,
}

/// Result produced by one driver without losing backend availability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DriverScan {
    /// The driver completed its scan, including the valid empty result.
    Complete(Vec<DiscoveredDevice>),
    /// The driver's optional backend is currently unavailable.
    Unavailable(Error),
}

/// An unavailable backend observed while other drivers continued scanning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnavailableDriver {
    /// Name of the driver whose backend was unavailable.
    pub driver: String,
    /// Machine-readable cause reported by the backend.
    pub cause: Error,
}

/// Complete registry scan result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScanOutcome {
    /// Devices discovered by available drivers and user configuration.
    pub devices: Vec<DiscoveredDevice>,
    /// Optional driver backends that could not participate in this scan.
    pub unavailable_drivers: Vec<UnavailableDriver>,
}

pub trait Driver: Send + Sync {
    fn name(&self) -> &str;
    fn scan_type(&self) -> ScanType;
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
    ) -> DiscoveredDevice {
        DiscoveredDevice {
            display_name,
            connstring,
            scan_type: self.scan_type(),
            exclusive: self.exclusive(),
            origin: self.origin(),
        }
    }
    /// Scans this driver's backend without conflating a valid empty result
    /// with temporary backend unavailability.
    ///
    /// # Errors
    ///
    /// Returns an operational failure that should abort the aggregate scan.
    fn scan(&self, context: &Context) -> Result<DriverScan, Error>;
    fn open(
        &self,
        context: &Context,
        connstring: &ConnectionString,
    ) -> Result<Box<dyn DeviceHandle>, Error>;
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

    /// Scans every enabled driver while preserving optional backend availability.
    ///
    /// # Errors
    ///
    /// Returns the first operational scan failure. Backend unavailability is
    /// represented in [`ScanOutcome`] and does not stop other drivers.
    pub fn scan(&self, context: &Context) -> Result<ScanOutcome, Error> {
        let mut devices = Vec::new();
        let mut unavailable_drivers = Vec::new();

        for configured in &context.config.user_defined_devices {
            if configured.optional && self.open(context, Some(&configured.connstring)).is_err() {
                continue;
            }
            devices.push(DiscoveredDevice {
                display_name: configured.name.clone(),
                connstring: configured.connstring.clone(),
                scan_type: ScanType::NotAvailable,
                exclusive: false,
                origin: DeviceOrigin::UserDefined,
            });
        }

        if context.config.allow_autoscan {
            for driver in self.drivers.iter().rev() {
                if !scan_allowed_for_driver(&context.config, driver.as_ref()) {
                    continue;
                }

                match driver.scan(context)? {
                    DriverScan::Complete(mut scanned) => devices.append(&mut scanned),
                    DriverScan::Unavailable(cause) => {
                        unavailable_drivers.push(UnavailableDriver {
                            driver: driver.name().to_string(),
                            cause,
                        });
                    }
                }
            }
        }

        Ok(ScanOutcome {
            devices,
            unavailable_drivers,
        })
    }

    fn first_available_device(&self, context: &Context) -> Result<Option<DiscoveredDevice>, Error> {
        for configured in &context.config.user_defined_devices {
            if configured.optional && self.open(context, Some(&configured.connstring)).is_err() {
                continue;
            }
            return Ok(Some(DiscoveredDevice {
                display_name: configured.name.clone(),
                connstring: configured.connstring.clone(),
                scan_type: ScanType::NotAvailable,
                exclusive: false,
                origin: DeviceOrigin::UserDefined,
            }));
        }

        if !context.config.allow_autoscan {
            return Ok(None);
        }

        for driver in self.drivers.iter().rev() {
            if !scan_allowed_for_driver(&context.config, driver.as_ref()) {
                continue;
            }

            match driver.scan(context)? {
                DriverScan::Complete(devices) => {
                    if let Some(device) = devices.into_iter().next() {
                        return Ok(Some(device));
                    }
                }
                DriverScan::Unavailable(_) => continue,
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
