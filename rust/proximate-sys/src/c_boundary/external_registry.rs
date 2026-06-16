use crate::bridge::driver_shim::ExternalDriver;
use crate::bridge::status::{NFC_EINVARG, NFC_ESOFT};
use crate::lifecycle::nfc_driver;
use libc::c_int;
use proximate_driver as rt;
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy)]
pub(crate) struct DriverHandle(pub(crate) *const nfc_driver);

unsafe impl Send for DriverHandle {}

pub(crate) struct RegisteredDriverSet<T> {
    drivers: Vec<T>,
}

impl<T> Default for RegisteredDriverSet<T> {
    fn default() -> Self {
        Self {
            drivers: Vec::new(),
        }
    }
}

impl<T> RegisteredDriverSet<T> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn register(&mut self, driver: T) -> Result<(), std::collections::TryReserveError> {
        self.drivers.try_reserve(1)?;
        self.drivers.push(driver);
        Ok(())
    }

    pub(crate) fn clear(&mut self) {
        self.drivers.clear();
    }
}

impl<T: Clone> RegisteredDriverSet<T> {
    pub(crate) fn snapshot(&self) -> Vec<T> {
        self.drivers.clone()
    }
}

static DRIVER_REGISTRY: OnceLock<Mutex<RegisteredDriverSet<DriverHandle>>> = OnceLock::new();

fn driver_registry() -> &'static Mutex<RegisteredDriverSet<DriverHandle>> {
    DRIVER_REGISTRY.get_or_init(|| Mutex::new(RegisteredDriverSet::new()))
}

fn with_registry<R>(f: impl FnOnce(&mut RegisteredDriverSet<DriverHandle>) -> R) -> R {
    let mut registry = driver_registry()
        .lock()
        .expect("driver registry mutex should not be poisoned");
    f(&mut registry)
}

pub(crate) unsafe fn push_driver(driver: *const nfc_driver) -> c_int {
    if driver.is_null() {
        return NFC_EINVARG;
    }

    with_registry(|registry| {
        if registry.register(DriverHandle(driver)).is_err() {
            return NFC_ESOFT;
        }
        0
    })
}

pub(crate) fn register_external_drivers(registry: &mut rt::DriverRegistry) {
    for handle in registry_snapshot() {
        registry.register_driver(Box::new(ExternalDriver::new(handle.0)));
    }
}

pub(crate) fn registry_snapshot() -> Vec<DriverHandle> {
    with_registry(|registry| registry.snapshot())
}

pub(crate) fn clear_registry() {
    with_registry(|registry| registry.clear());
}

#[cfg(test)]
mod tests {
    use super::RegisteredDriverSet;

    #[test]
    fn snapshot_preserves_insertion_order() {
        let mut registry = RegisteredDriverSet::new();
        registry.register("alpha").unwrap();
        registry.register("beta").unwrap();

        assert_eq!(registry.snapshot(), vec!["alpha", "beta"]);
    }
}
