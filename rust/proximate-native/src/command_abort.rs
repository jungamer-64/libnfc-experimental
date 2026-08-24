use proximate_driver::{CommandAbort, Error};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(crate) struct AtomicCommandAbort {
    requested: AtomicBool,
    active: AtomicBool,
}

impl AtomicCommandAbort {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            requested: AtomicBool::new(false),
            active: AtomicBool::new(true),
        })
    }

    pub(crate) fn begin_command(&self) {
        self.requested.store(false, Ordering::Release);
    }

    pub(crate) fn take_requested(&self) -> bool {
        self.requested.swap(false, Ordering::AcqRel)
    }

    pub(crate) fn revoke(&self) {
        self.active.store(false, Ordering::Release);
    }
}

impl CommandAbort for AtomicCommandAbort {
    fn abort(&self) -> Result<(), Error> {
        if !self.active.load(Ordering::Acquire) {
            return Err(Error::TargetReleased("abort_command"));
        }
        self.requested.store(true, Ordering::Release);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_abort_is_consumed_once_and_reset_at_command_start() {
        let abort = AtomicCommandAbort::new();
        abort.abort().unwrap();
        assert!(abort.take_requested());
        assert!(!abort.take_requested());

        abort.abort().unwrap();
        abort.begin_command();
        assert!(!abort.take_requested());
    }

    #[test]
    fn command_abort_is_revoked_with_its_device() {
        let abort = AtomicCommandAbort::new();
        abort.revoke();
        assert_eq!(abort.abort(), Err(Error::TargetReleased("abort_command")));
    }
}
