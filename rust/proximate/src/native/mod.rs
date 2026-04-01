mod pn53x;

use crate::rust_api::DriverRegistry;

pub(crate) fn register_builtin_drivers(_registry: &mut DriverRegistry) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registration_is_safe_when_no_native_drivers_are_enabled() {
        let mut registry = DriverRegistry::new();
        register_builtin_drivers(&mut registry);
        assert!(registry.is_empty());
    }
}
