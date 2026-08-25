use super::external_driver::{DriverSnapshot, ExternalDriver};
use crate::c_boundary::status::NFC_ESOFT;
use crate::lifecycle::nfc_driver;
use libc::c_int;
use proximate_driver as rt;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Default)]
struct RegisteredDrivers {
    entries: Vec<Arc<DriverSnapshot>>,
}

static DRIVER_REGISTRY: OnceLock<Mutex<RegisteredDrivers>> = OnceLock::new();

fn registry() -> &'static Mutex<RegisteredDrivers> {
    DRIVER_REGISTRY.get_or_init(|| Mutex::new(RegisteredDrivers::default()))
}

fn with_registry<R>(operation: impl FnOnce(&mut RegisteredDrivers) -> R) -> Result<R, ()> {
    registry()
        .lock()
        .map(|mut registry| operation(&mut registry))
        .map_err(|_| ())
}

pub(crate) unsafe fn push_driver(driver: *const nfc_driver) -> c_int {
    let snapshot = match unsafe { DriverSnapshot::from_raw(driver) } {
        Ok(snapshot) => Arc::new(snapshot),
        Err(status) => return status,
    };
    match with_registry(|registry| {
        registry.entries.try_reserve(1)?;
        registry.entries.push(snapshot);
        Ok::<(), std::collections::TryReserveError>(())
    }) {
        Ok(Ok(())) => 0,
        Ok(Err(_)) | Err(()) => NFC_ESOFT,
    }
}

pub(crate) fn register_external_drivers(registry: &mut rt::DriverRegistry) {
    let snapshots = with_registry(|registered| registered.entries.clone()).unwrap_or_default();
    for snapshot in snapshots {
        registry.register_driver(Box::new(ExternalDriver::new(snapshot)));
    }
}

#[cfg(test)]
pub(crate) fn registry_snapshot() -> Vec<Arc<DriverSnapshot>> {
    with_registry(|registered| registered.entries.clone()).unwrap_or_default()
}

pub(crate) fn clear_registry() {
    let _ = with_registry(|registered| registered.entries.clear());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_registration_is_rejected() {
        assert_eq!(
            unsafe { push_driver(std::ptr::null()) },
            crate::c_boundary::status::NFC_EINVARG
        );
    }
}
