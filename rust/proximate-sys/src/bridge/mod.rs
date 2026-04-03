#[path = "c_to_rust.rs"]
mod decode;
mod driver_shim;
#[path = "rust_to_c.rs"]
mod encode;
mod external_registry;
mod status;

pub(crate) use decode::{
    InputBytes, OutputBytes, ParityMarker, ParityMarkerMut, baud_rate_from_c, context_from_c,
    decode_connstring_ptr, decode_modulations, decode_optional_dep_info, decode_optional_target,
    dep_info_from_c, dep_mode_from_c, modulation_from_c, modulation_type_from_c, property_from_c,
    target_from_c,
};
pub(crate) use driver_shim::{
    attach_rust_device, borrowed_device, free_rust_device, is_rust_shim_device,
    rust_device_state_mut,
};
pub(crate) use encode::{
    CStringOut, ConnstringsOut, CyclesOut, SupportedBaudRatesOut, SupportedModulationsOut,
    TargetInOut, TargetOut, TargetSliceOut, baud_rate_to_c, dep_info_to_c, dep_mode_to_c,
    mode_to_c, modulation_to_c, modulation_type_to_c, property_to_c, target_to_c,
    write_context_to_c, write_target_to_c,
};
#[cfg(test)]
pub(crate) use external_registry::registry_snapshot;
pub(crate) use external_registry::{clear_registry, push_driver, register_external_drivers};
pub(crate) use status::{
    NFC_EDEVNOTSUPP, NFC_EINVARG, NFC_ENOTIMPL, NFC_ENOTSUCHDEV, NFC_EOVFLOW, NFC_ESOFT,
    device_last_error, error_message_ptr, error_to_status, invalid_argument_status,
    reset_device_last_error, runtime_result_status, set_device_last_error, soft_error_status,
    unsupported_driver_operation,
};
