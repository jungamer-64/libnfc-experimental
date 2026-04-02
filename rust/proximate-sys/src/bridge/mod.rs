mod c_to_rust;
mod driver_shim;
mod external_registry;
mod rust_to_c;
mod status;

pub(crate) use c_to_rust::{
    baud_rate_from_c, context_from_c, dep_info_from_c, dep_mode_from_c, modulation_from_c,
    modulation_type_from_c, property_from_c, target_from_c,
};
pub(crate) use driver_shim::{attach_rust_device, borrowed_device, is_rust_shim_device};
#[cfg(test)]
pub(crate) use external_registry::registry_snapshot;
pub(crate) use external_registry::{clear_registry, push_driver, register_external_drivers};
pub(crate) use rust_to_c::{write_context_to_c, write_target_to_c};
pub(crate) use status::{error_to_status, set_device_last_error};
