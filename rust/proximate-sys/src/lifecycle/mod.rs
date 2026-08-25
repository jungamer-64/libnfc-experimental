mod abi;
pub(crate) mod alloc;
mod handles;
mod logging;
#[cfg(test)]
mod tests;

pub(crate) use abi::{
    DEVICE_NAME_LENGTH, MAX_USER_DEFINED_DEVICES, NFC_DRIVER_NAME_MAX, scan_type_enum,
};
pub use abi::{nfc_connstring, nfc_context, nfc_device, nfc_driver};
pub(crate) use handles::{
    AbiContext, AbiDevice, context_into_raw, context_ref, device_into_raw, device_ref,
    drop_context, drop_device,
};

#[cfg(test)]
pub(crate) use alloc::nfc_context_alloc_defaults;
pub(crate) use alloc::{attach_device, nfc_context_free, nfc_context_new, runtime_context_from_c};
#[cfg(test)]
pub(crate) use logging::{reset_lifecycle_test_state, snapshot_lifecycle_test_state};
