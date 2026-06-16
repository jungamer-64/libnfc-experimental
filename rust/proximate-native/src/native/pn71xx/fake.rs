use std::sync::{Mutex, OnceLock};

use crate::nci::{Backend, TagInfo};

use super::runtime::{callbacks_registered, clear_runtime_state, replace_runtime_state};

#[derive(Clone, Debug, Default)]
pub(super) struct BackendTestState {
    pub(super) init_result: i32,
    pub(super) initialize_calls: usize,
    pub(super) deinitialize_calls: usize,
    pub(super) register_calls: usize,
    pub(super) deregister_calls: usize,
    pub(super) enable_calls: usize,
    pub(super) disable_calls: usize,
    pub(super) last_discovery_args: Option<(i32, i32, i32, i32)>,
    pub(super) transceive_result: i32,
    pub(super) transceive_response: Vec<u8>,
    pub(super) last_transceive_handle: Option<u32>,
    pub(super) last_transceive_tx: Vec<u8>,
    pub(super) last_transceive_timeout: Option<i32>,
    pub(super) current_tag: Option<TagInfo>,
}

#[derive(Default)]
pub(super) struct FakeNciBackend {
    state: Mutex<BackendTestState>,
}

impl Backend for FakeNciBackend {
    fn initialize(&self) -> i32 {
        let mut state = self.state.lock().unwrap();
        state.initialize_calls += 1;
        state.init_result
    }

    fn deinitialize(&self) {
        let mut state = self.state.lock().unwrap();
        state.deinitialize_calls += 1;
        state.current_tag = None;
    }

    fn register_callbacks(&self) {
        let mut state = self.state.lock().unwrap();
        state.register_calls += 1;
        state.current_tag = None;
    }

    fn deregister_callbacks(&self) {
        let mut state = self.state.lock().unwrap();
        state.deregister_calls += 1;
        state.current_tag = None;
    }

    fn enable_discovery(
        &self,
        technologies_mask: i32,
        reader_mode: i32,
        enable_host_routing: i32,
        restart: i32,
    ) {
        let mut state = self.state.lock().unwrap();
        state.enable_calls += 1;
        state.last_discovery_args =
            Some((technologies_mask, reader_mode, enable_host_routing, restart));
    }

    fn disable_discovery(&self) {
        self.state.lock().unwrap().disable_calls += 1;
    }

    fn transceive(&self, handle: u32, tx: &[u8], rx: &mut [u8], timeout: i32) -> i32 {
        let mut state = self.state.lock().unwrap();
        state.last_transceive_handle = Some(handle);
        state.last_transceive_timeout = Some(timeout);
        state.last_transceive_tx = tx.to_vec();
        if state.transceive_result <= 0 {
            return state.transceive_result;
        }

        let copy_len = state
            .transceive_response
            .len()
            .min(rx.len())
            .min(state.transceive_result as usize);
        rx[..copy_len].copy_from_slice(&state.transceive_response[..copy_len]);
        state.transceive_result
    }

    fn current_tag_snapshot(&self) -> Option<TagInfo> {
        self.state.lock().unwrap().current_tag
    }
}

pub(super) fn backend_instance() -> &'static FakeNciBackend {
    static BACKEND: OnceLock<FakeNciBackend> = OnceLock::new();
    BACKEND.get_or_init(FakeNciBackend::default)
}

pub(super) fn reset_test_world() {
    clear_runtime_state();
    *backend_instance().state.lock().unwrap() = BackendTestState::default();
}

pub(super) fn backend_state_snapshot() -> BackendTestState {
    backend_instance().state.lock().unwrap().clone()
}

pub(super) fn with_backend_state_mut<R>(f: impl FnOnce(&mut BackendTestState) -> R) -> R {
    let mut state = backend_instance().state.lock().unwrap();
    f(&mut state)
}

pub(super) fn emit_tag_arrival_for_tests(tag: TagInfo) {
    if callbacks_registered() {
        backend_instance().state.lock().unwrap().current_tag = Some(tag);
    }
}

pub(super) fn emit_tag_departure_for_tests() {
    if callbacks_registered() {
        backend_instance().state.lock().unwrap().current_tag = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::pn71xx::runtime::{Pn71xxRuntime, runtime_snapshot};

    #[test]
    fn callback_helpers_only_update_tag_state_when_callbacks_are_registered() {
        reset_test_world();
        let tag = TagInfo {
            technology: 1,
            handle: 2,
            uid: [0xAA; 32],
            uid_length: 4,
            protocol: 0,
        };

        emit_tag_arrival_for_tests(tag);
        assert_eq!(backend_state_snapshot().current_tag, None);

        replace_runtime_state(Pn71xxRuntime {
            callbacks_registered: true,
            ..Default::default()
        });
        emit_tag_arrival_for_tests(tag);
        assert_eq!(backend_state_snapshot().current_tag, Some(tag));

        emit_tag_departure_for_tests();
        assert_eq!(backend_state_snapshot().current_tag, None);
        assert!(runtime_snapshot().callbacks_registered);
    }
}
