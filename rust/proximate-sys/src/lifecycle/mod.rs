mod abi;
pub(crate) mod alloc;
mod logging;
#[cfg(test)]
mod tests;

pub use abi::{
    DEVICE_NAME_LENGTH, MAX_USER_DEFINED_DEVICES, NFC_DRIVER_NAME_MAX, nfc_connstring, nfc_context,
    nfc_device, nfc_driver, nfc_user_defined_device, scan_type_enum,
};

#[cfg(test)]
pub(crate) use alloc::{nfc_context_alloc_defaults, nfc_device_free};
pub(crate) use alloc::{nfc_context_free, nfc_context_new, nfc_device_new, runtime_context_from_c};
#[cfg(test)]
pub(crate) use logging::{reset_lifecycle_test_state, snapshot_lifecycle_test_state};
