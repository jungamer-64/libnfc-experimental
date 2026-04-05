use std::thread;

use proximate_driver::{ConnectionString, Context, DeviceHandle, Driver, Error, ScanType};

use super::backend::backend;
use super::consts::{DEFAULT_NFA_TECH_MASK, NFC_SETTLE_DELAY, PN71XX_DRIVER_NAME};
use super::device::Pn71xxDevice;
use super::runtime::{activate_device, active_device, normalize_inactive_runtime};

pub(crate) struct Pn71xxDriver;

impl Pn71xxDriver {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Driver for Pn71xxDriver {
    fn name(&self) -> &str {
        PN71XX_DRIVER_NAME
    }

    fn exclusive(&self) -> bool {
        true
    }

    fn scan_type(&self) -> ScanType {
        ScanType::NotIntrusive
    }

    fn scan(&self, _context: &Context) -> Result<Vec<proximate_driver::DiscoveredDevice>, Error> {
        normalize_inactive_runtime();

        if active_device().is_some() {
            return Ok(vec![self.describe_discovered(
                PN71XX_DRIVER_NAME.to_string(),
                ConnectionString::new(PN71XX_DRIVER_NAME).unwrap(),
                Some(Pn71xxDevice::scan_caps()),
            )]);
        }

        if backend().initialize() != 0 {
            return Ok(Vec::new());
        }
        backend().deinitialize();

        Ok(vec![self.describe_discovered(
            PN71XX_DRIVER_NAME.to_string(),
            ConnectionString::new(PN71XX_DRIVER_NAME).unwrap(),
            Some(Pn71xxDevice::scan_caps()),
        )])
    }

    fn open(
        &self,
        _context: &Context,
        connstring: &ConnectionString,
    ) -> Result<Box<dyn DeviceHandle>, Error> {
        normalize_inactive_runtime();

        if active_device().is_some() {
            return Err(Error::DriverOpenFailed(
                "pn71xx only supports one active device at a time".to_string(),
            ));
        }

        let rc = backend().initialize();
        if rc != 0 {
            return Err(Error::DriverOpenFailed(format!(
                "pn71xx backend initialization failed with rc={rc}"
            )));
        }

        backend().register_callbacks();
        backend().enable_discovery(DEFAULT_NFA_TECH_MASK, 1, 0, 0);
        thread::sleep(NFC_SETTLE_DELAY);

        Ok(Box::new(Pn71xxDevice::new(
            activate_device(),
            connstring.clone(),
        )))
    }
}
