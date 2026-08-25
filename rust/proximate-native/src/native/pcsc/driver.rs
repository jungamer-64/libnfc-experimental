use super::*;

pub(crate) struct PcscDriver {
    driver_name: &'static str,
    filter: ReaderFilter,
    backend: Arc<dyn PcscBackend>,
}

impl PcscDriver {
    pub(crate) fn new() -> Self {
        Self {
            driver_name: PCSC_DRIVER_NAME,
            filter: ReaderFilter::Generic,
            backend: Arc::new(SystemPcscBackend),
        }
    }

    #[cfg(test)]
    pub(super) fn with_backend(backend: Arc<dyn PcscBackend>) -> Self {
        Self {
            driver_name: PCSC_DRIVER_NAME,
            filter: ReaderFilter::Generic,
            backend,
        }
    }
}

impl Driver for PcscDriver {
    fn name(&self) -> &str {
        self.driver_name
    }

    fn scan_type(&self) -> ScanType {
        ScanType::NotIntrusive
    }

    fn scan(&self, _context: &Context) -> Result<proximate_driver::DriverScan, Error> {
        let readers =
            match scan_matching_readers(self.backend.as_ref(), self.driver_name, self.filter)? {
                ReaderScan::Complete(readers) => readers,
                ReaderScan::Unavailable(cause) => {
                    return Ok(proximate_driver::DriverScan::Unavailable(cause));
                }
            };
        Ok(proximate_driver::DriverScan::Complete(
            readers
                .into_iter()
                .map(|connstring| {
                    let display_name = connstring.as_str().to_string();
                    self.describe_discovered(display_name, connstring)
                })
                .collect(),
        ))
    }

    fn open(
        &self,
        _context: &Context,
        connstring: &ConnectionString,
    ) -> Result<Box<dyn DeviceHandle>, Error> {
        let (reader_name, resolved_connstring) = resolve_reader(
            self.backend.as_ref(),
            connstring,
            self.driver_name,
            self.filter,
        )?;
        let card = self
            .backend
            .connect(&reader_name, PcscShareMode::Direct, PcscProtocols::T0)
            .map_err(|status| device_error("pcsc_connect", status))?;
        Ok(Box::new(PcscDevice::new(
            reader_name,
            resolved_connstring,
            card,
            PcscShareMode::Direct,
            PcscProtocols::T0,
        )))
    }
}
