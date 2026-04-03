use crate::nci::Backend;
#[cfg(not(test))]
use crate::nci::SystemBackend;

#[cfg(test)]
use super::fake::backend_instance;

#[cfg(not(test))]
pub(super) fn backend() -> &'static dyn Backend {
    static BACKEND: SystemBackend = SystemBackend;
    &BACKEND
}

#[cfg(test)]
pub(super) fn backend() -> &'static dyn Backend {
    backend_instance()
}
