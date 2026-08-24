use crate::nci::TagInfo;
use proximate_driver::Error;
use std::sync::{Mutex, OnceLock};

use super::backend::backend;
use super::consts::DEFAULT_NFA_TECH_MASK;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
/// Authoritative lifecycle of the process-global NCI stack and its sole device.
///
/// Each non-inactive variant implies exactly which external NCI resources are
/// live, so callback, discovery, and initialization state cannot diverge.
pub(super) enum Pn71xxSession {
    #[default]
    Inactive,
    Discovering {
        device_id: u64,
    },
    Idle {
        device_id: u64,
    },
    PoweredDown {
        device_id: u64,
    },
}

#[derive(Clone, Debug, Default)]
pub(super) struct Pn71xxRuntime {
    pub(super) session: Pn71xxSession,
    pub(super) next_device_id: u64,
}

fn runtime() -> &'static Mutex<Pn71xxRuntime> {
    static RUNTIME: OnceLock<Mutex<Pn71xxRuntime>> = OnceLock::new();
    RUNTIME.get_or_init(|| Mutex::new(Pn71xxRuntime::default()))
}

#[cfg(test)]
pub(super) fn clear_runtime_state() {
    let mut state = runtime().lock().unwrap();
    *state = Pn71xxRuntime::default();
}

#[cfg(test)]
pub(super) fn callbacks_registered() -> bool {
    matches!(
        runtime().lock().unwrap().session,
        Pn71xxSession::Discovering { .. } | Pn71xxSession::Idle { .. }
    )
}

pub(super) fn active_device() -> Option<u64> {
    match runtime().lock().unwrap().session {
        Pn71xxSession::Inactive => None,
        Pn71xxSession::Discovering { device_id }
        | Pn71xxSession::Idle { device_id }
        | Pn71xxSession::PoweredDown { device_id } => Some(device_id),
    }
}

pub(super) fn activate_device() -> u64 {
    let mut state = runtime().lock().unwrap();
    let device_id = state.next_device_id;
    state.next_device_id = state.next_device_id.wrapping_add(1);
    state.session = Pn71xxSession::Discovering { device_id };
    device_id
}

pub(super) fn current_tag_snapshot() -> Option<TagInfo> {
    matches!(
        runtime().lock().unwrap().session,
        Pn71xxSession::Discovering { .. }
    )
    .then(|| backend().current_tag_snapshot())
    .flatten()
}

fn ensure_device(session: Pn71xxSession, device_id: u64) -> Result<(), Error> {
    match session {
        Pn71xxSession::Discovering {
            device_id: active_id,
        }
        | Pn71xxSession::Idle {
            device_id: active_id,
        }
        | Pn71xxSession::PoweredDown {
            device_id: active_id,
        } if active_id == device_id => Ok(()),
        _ => Err(Error::TargetReleased("pn71xx_device")),
    }
}

pub(super) fn ensure_discovery(device_id: u64) -> Result<(), Error> {
    let mut state = runtime().lock().unwrap();
    ensure_device(state.session, device_id)?;

    match state.session {
        Pn71xxSession::Discovering { .. } => return Ok(()),
        Pn71xxSession::Idle { .. } => {}
        Pn71xxSession::PoweredDown { .. } => {
            let rc = backend().initialize();
            if rc != 0 {
                return Err(Error::DeviceOperationFailed {
                    operation: "pn71xx_reinitialize",
                    code: rc,
                });
            }
            backend().register_callbacks();
        }
        Pn71xxSession::Inactive => unreachable!("validated by ensure_device"),
    }

    backend().enable_discovery(DEFAULT_NFA_TECH_MASK, 1, 0, 0);
    state.session = Pn71xxSession::Discovering { device_id };
    Ok(())
}

pub(super) fn idle_active_device(device_id: u64) -> Result<(), Error> {
    let mut state = runtime().lock().unwrap();
    ensure_device(state.session, device_id)?;
    match state.session {
        Pn71xxSession::Discovering { .. } => {
            backend().disable_discovery();
            state.session = Pn71xxSession::Idle { device_id };
        }
        Pn71xxSession::Idle { .. } => {}
        Pn71xxSession::PoweredDown { .. } => {}
        Pn71xxSession::Inactive => unreachable!("validated by ensure_device"),
    }
    Ok(())
}

pub(super) fn restart_discovery(device_id: u64) -> Result<(), Error> {
    let mut state = runtime().lock().unwrap();
    ensure_device(state.session, device_id)?;
    match state.session {
        Pn71xxSession::Discovering { .. } => backend().disable_discovery(),
        Pn71xxSession::Idle { .. } => {}
        Pn71xxSession::PoweredDown { .. } => {
            let rc = backend().initialize();
            if rc != 0 {
                return Err(Error::DeviceOperationFailed {
                    operation: "pn71xx_reinitialize",
                    code: rc,
                });
            }
            backend().register_callbacks();
        }
        Pn71xxSession::Inactive => unreachable!("validated by ensure_device"),
    }
    backend().enable_discovery(DEFAULT_NFA_TECH_MASK, 1, 0, 1);
    state.session = Pn71xxSession::Discovering { device_id };
    Ok(())
}

pub(super) fn powerdown_active_device(device_id: u64) -> Result<(), Error> {
    let mut state = runtime().lock().unwrap();
    ensure_device(state.session, device_id)?;
    match state.session {
        Pn71xxSession::Discovering { .. } => {
            backend().disable_discovery();
            backend().deregister_callbacks();
            backend().deinitialize();
        }
        Pn71xxSession::Idle { .. } => {
            backend().deregister_callbacks();
            backend().deinitialize();
        }
        Pn71xxSession::PoweredDown { .. } => return Ok(()),
        Pn71xxSession::Inactive => unreachable!("validated by ensure_device"),
    }
    state.session = Pn71xxSession::PoweredDown { device_id };
    Ok(())
}

pub(super) fn close_active_device(device_id: u64) {
    let mut state = runtime().lock().unwrap();
    if ensure_device(state.session, device_id).is_err() {
        return;
    }

    match state.session {
        Pn71xxSession::Discovering { .. } => {
            backend().disable_discovery();
            backend().deregister_callbacks();
            backend().deinitialize();
        }
        Pn71xxSession::Idle { .. } => {
            backend().deregister_callbacks();
            backend().deinitialize();
        }
        Pn71xxSession::PoweredDown { .. } => {}
        Pn71xxSession::Inactive => unreachable!("validated by ensure_device"),
    }
    state.session = Pn71xxSession::Inactive;
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
    fn ensuring_an_active_discovery_is_idempotent() {
        reset_test_world();
        replace_runtime_state(Pn71xxRuntime {
            session: Pn71xxSession::Discovering { device_id: 7 },
            next_device_id: 8,
        });

        ensure_discovery(7).unwrap();

        let backend = backend_state_snapshot();
        assert_eq!(backend.disable_calls, 0);
        assert_eq!(backend.deregister_calls, 0);
        assert_eq!(backend.deinitialize_calls, 0);
        assert_eq!(active_device(), Some(7));
    }
}
