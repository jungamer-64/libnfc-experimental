pub(crate) mod alloc;
mod logging;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use alloc::{nfc_context_alloc_defaults, nfc_device_free};
pub(crate) use alloc::{nfc_context_free, nfc_context_new, nfc_device_new, runtime_context_from_c};
#[cfg(test)]
pub(crate) use logging::{reset_lifecycle_test_state, snapshot_lifecycle_test_state};
