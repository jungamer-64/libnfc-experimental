use crate::nci::TagInfo;
use std::sync::{Mutex, OnceLock};

use super::backend::backend;

#[derive(Clone, Debug, Default)]
pub(super) struct Pn71xxRuntime {
    pub(super) initialized: bool,
    pub(super) callbacks_registered: bool,
    pub(super) discovery_enabled: bool,
    pub(super) active_device: Option<u64>,
    pub(super) next_device_id: u64,
}

fn runtime() -> &'static Mutex<Pn71xxRuntime> {
    static RUNTIME: OnceLock<Mutex<Pn71xxRuntime>> = OnceLock::new();
    RUNTIME.get_or_init(|| Mutex::new(Pn71xxRuntime::default()))
}

pub(super) fn clear_runtime_state() {
    let mut state = runtime().lock().unwrap();
    *state = Pn71xxRuntime::default();
}

pub(super) fn callbacks_registered() -> bool {
    runtime().lock().unwrap().callbacks_registered
}

pub(super) fn active_device() -> Option<u64> {
    runtime().lock().unwrap().active_device
}

pub(super) fn activate_device() -> u64 {
    let mut state = runtime().lock().unwrap();
    let device_id = state.next_device_id;
    state.next_device_id = state.next_device_id.wrapping_add(1);
    state.initialized = true;
    state.callbacks_registered = true;
    state.discovery_enabled = true;
    state.active_device = Some(device_id);
    device_id
}

pub(super) fn current_tag_snapshot() -> Option<TagInfo> {
    backend().current_tag_snapshot()
}

pub(super) fn normalize_inactive_runtime() {
    let snapshot = runtime().lock().unwrap().clone();
    if snapshot.active_device.is_some() {
        return;
    }

    if snapshot.discovery_enabled {
        backend().disable_discovery();
    }
    if snapshot.callbacks_registered {
        backend().deregister_callbacks();
    }
    if snapshot.initialized {
        backend().deinitialize();
    }

    clear_runtime_state();
}

pub(super) fn close_active_device(device_id: u64) {
    let snapshot = runtime().lock().unwrap().clone();
    if snapshot.active_device != Some(device_id) {
        return;
    }

    if snapshot.discovery_enabled {
        backend().disable_discovery();
    }
    if snapshot.callbacks_registered {
        backend().deregister_callbacks();
    }

    {
        let mut state = runtime().lock().unwrap();
        state.discovery_enabled = false;
        state.callbacks_registered = false;
        state.active_device = None;
    }

    if snapshot.initialized {
        backend().deinitialize();
    }

    runtime().lock().unwrap().initialized = false;
}

#[cfg(test)]
pub(super) fn runtime_snapshot() -> Pn71xxRuntime {
    runtime().lock().unwrap().clone()
}

#[cfg(test)]
pub(super) fn replace_runtime_state(state: Pn71xxRuntime) {
    *runtime().lock().unwrap() = state;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::pn71xx::fake::{backend_state_snapshot, reset_test_world};

    #[test]
    fn normalize_inactive_runtime_does_not_teardown_while_device_is_active() {
        reset_test_world();
        replace_runtime_state(Pn71xxRuntime {
            initialized: true,
            callbacks_registered: true,
            discovery_enabled: true,
            active_device: Some(7),
            next_device_id: 8,
        });

        normalize_inactive_runtime();

        let backend = backend_state_snapshot();
        assert_eq!(backend.disable_calls, 0);
        assert_eq!(backend.deregister_calls, 0);
        assert_eq!(backend.deinitialize_calls, 0);
        assert_eq!(runtime_snapshot().active_device, Some(7));
    }
}
