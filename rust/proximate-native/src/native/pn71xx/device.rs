use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

use crate::command_abort::AtomicCommandAbort;
use proximate_driver::{
    BaudRate, CommandAbort, CommandAbortHandle, ConnectionString, DeviceMeta, Error, InfoBackend,
    InitiatorBackend, Mode, Modulation, ModulationType, OperationTimeout, Pn53xBackend,
    PollIterations, PollPeriod, Property, PropertyBackend, Target, TargetBackend,
};

use super::backend::backend;
use super::consts::{
    FELICA_SUPPORTED_BAUD_RATES, ISO14443A_SUPPORTED_BAUD_RATES, ISO14443B_SUPPORTED_BAUD_RATES,
    JEWEL_SUPPORTED_BAUD_RATES, NFC_EINVARG, NFC_EIO, NFC_SUCCESS, PN71XX_DEVICE_NAME, PN71XX_INFO,
    POLL_PERIOD_FACTOR_MICROS, SUPPORTED_MODULATIONS,
};
use super::device_error;
use super::runtime::{
    close_active_device, current_tag_snapshot, ensure_discovery, idle_active_device,
    powerdown_active_device, restart_discovery,
};
use super::target::build_target;

struct Pn71xxCommandAbort {
    request: Arc<AtomicCommandAbort>,
    device_id: u64,
    command_active: AtomicBool,
}

impl Pn71xxCommandAbort {
    fn new(device_id: u64) -> Arc<Self> {
        Arc::new(Self {
            request: AtomicCommandAbort::new(),
            device_id,
            command_active: AtomicBool::new(false),
        })
    }

    fn begin_command(&self) {
        self.request.begin_command();
        self.command_active.store(true, Ordering::Release);
    }

    fn finish_command(&self) {
        self.command_active.store(false, Ordering::Release);
    }

    fn take_requested(&self) -> bool {
        self.request.take_requested()
    }

    fn revoke(&self) {
        self.command_active.store(false, Ordering::Release);
        self.request.revoke();
    }
}

impl CommandAbort for Pn71xxCommandAbort {
    fn abort(&self) -> Result<(), Error> {
        self.request.abort()?;
        if self.command_active.load(Ordering::Acquire) {
            idle_active_device(self.device_id)?;
        }
        Ok(())
    }
}

pub(super) struct Pn71xxDevice {
    device_id: u64,
    connstring: ConnectionString,
    last_error: i32,
    timeout_communication_ms: u32,
    infinite_select: bool,
    command_abort: Arc<Pn71xxCommandAbort>,
}

impl Pn71xxDevice {
    pub(super) fn new(device_id: u64, connstring: ConnectionString) -> Self {
        Self {
            device_id,
            connstring,
            last_error: NFC_SUCCESS,
            timeout_communication_ms: 500,
            infinite_select: false,
            command_abort: Pn71xxCommandAbort::new(device_id),
        }
    }

    fn succeed<T>(&mut self, value: T) -> Result<T, Error> {
        self.last_error = NFC_SUCCESS;
        Ok(value)
    }

    fn fail<T>(&mut self, operation: &'static str, code: i32) -> Result<T, Error> {
        self.last_error = code;
        Err(device_error(operation, code))
    }

    fn aborted(&self, operation: &'static str) -> Error {
        let operation = Error::Aborted(operation);
        match idle_active_device(self.device_id) {
            Ok(()) => operation,
            Err(recovery) => Error::RecoveryFailed {
                operation: Box::new(operation),
                recovery: Box::new(recovery),
            },
        }
    }
}

impl Drop for Pn71xxDevice {
    fn drop(&mut self) {
        self.command_abort.revoke();
        close_active_device(self.device_id);
    }
}

impl DeviceMeta for Pn71xxDevice {
    fn name(&self) -> &str {
        PN71XX_DEVICE_NAME
    }

    fn connstring(&self) -> &ConnectionString {
        &self.connstring
    }

    fn last_error(&self) -> i32 {
        self.last_error
    }
}

impl InfoBackend for Pn71xxDevice {
    fn information_about(&mut self) -> Result<String, Error> {
        self.succeed(PN71XX_INFO.to_string())
    }
}

impl PropertyBackend for Pn71xxDevice {
    fn set_property_bool(&mut self, property: Property, enable: bool) -> Result<(), Error> {
        // libnfc-nci owns framing, CRC, parity, and ISO-DEP policy. Accept only
        // values matching those fixed backend semantics; mutable RF field and
        // selection policy are represented by actual Rust/NCI state changes.
        match property {
            Property::ActivateField if enable => ensure_discovery(self.device_id)?,
            Property::ActivateField => idle_active_device(self.device_id)?,
            Property::InfiniteSelect => self.infinite_select = enable,
            Property::HandleCrc
            | Property::HandleParity
            | Property::AutoIso14443_4
            | Property::EasyFraming
            | Property::ForceIso14443A
            | Property::ForceSpeed106
                if enable => {}
            Property::AcceptInvalidFrames | Property::AcceptMultipleFrames if !enable => {}
            _ => return Err(Error::UnsupportedOperation("pn71xx_property_bool")),
        }
        self.succeed(())
    }

    fn set_property_int(&mut self, property: Property, value: i32) -> Result<(), Error> {
        if property != Property::TimeoutCom {
            return Err(Error::UnsupportedOperation("pn71xx_property_int"));
        }
        let value = u32::try_from(value).map_err(|_| Error::InvalidArgument("timeout"))?;
        self.timeout_communication_ms = value;
        self.succeed(())
    }

    fn supported_modulations(&mut self, mode: Mode) -> Result<Vec<ModulationType>, Error> {
        if mode == Mode::Target {
            return Err(Error::UnsupportedOperation("pn71xx_target_mode"));
        }
        self.succeed(SUPPORTED_MODULATIONS.to_vec())
    }

    fn supported_baud_rates(
        &mut self,
        mode: Mode,
        modulation_type: ModulationType,
    ) -> Result<Vec<BaudRate>, Error> {
        if mode == Mode::Target {
            return Err(Error::UnsupportedOperation("pn71xx_target_mode"));
        }
        let values = match modulation_type {
            ModulationType::Felica => FELICA_SUPPORTED_BAUD_RATES,
            ModulationType::Iso14443A => ISO14443A_SUPPORTED_BAUD_RATES,
            ModulationType::Iso14443B
            | ModulationType::Iso14443Bi
            | ModulationType::Iso14443B2Sr
            | ModulationType::Iso14443B2Ct => ISO14443B_SUPPORTED_BAUD_RATES,
            ModulationType::Jewel => JEWEL_SUPPORTED_BAUD_RATES,
            _ => return self.fail("pn71xx_get_supported_baud_rate", NFC_EINVARG),
        };
        self.succeed(values.to_vec())
    }

    fn property_bool_state(&self, property: Property) -> Option<bool> {
        match property {
            Property::InfiniteSelect => Some(self.infinite_select),
            Property::HandleCrc
            | Property::HandleParity
            | Property::AutoIso14443_4
            | Property::EasyFraming
            | Property::ForceIso14443A
            | Property::ForceSpeed106 => Some(true),
            Property::AcceptInvalidFrames | Property::AcceptMultipleFrames => Some(false),
            _ => None,
        }
    }
}

impl InitiatorBackend for Pn71xxDevice {
    fn command_abort_handle(&self) -> Option<CommandAbortHandle> {
        Some(self.command_abort.clone())
    }

    fn initiator_init_driver(&mut self) -> Result<i32, Error> {
        ensure_discovery(self.device_id)?;
        self.succeed(0)
    }

    fn select_passive_target_driver(
        &mut self,
        modulation: Modulation,
        _init_data: &[u8],
    ) -> Result<Option<Target>, Error> {
        self.command_abort.begin_command();
        let result = self.select_passive_target(modulation);
        self.command_abort.finish_command();
        result
    }

    fn poll_target_driver(
        &mut self,
        modulations: &[Modulation],
        iterations: PollIterations,
        period: PollPeriod,
    ) -> Result<Option<Target>, Error> {
        self.command_abort.begin_command();
        let result = self.poll_target(
            modulations,
            iterations,
            Duration::from_micros(u64::from(period.get()) * POLL_PERIOD_FACTOR_MICROS),
        );
        self.command_abort.finish_command();
        result
    }

    fn deselect_target_driver(&mut self) -> Result<(), Error> {
        restart_discovery(self.device_id)?;
        self.succeed(())
    }

    fn transceive_bytes_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: OperationTimeout,
    ) -> Result<usize, Error> {
        self.command_abort.begin_command();
        let result = self.transceive_bytes(tx, rx, timeout);
        self.command_abort.finish_command();
        result
    }

    fn target_is_present_driver(&mut self, _target: Option<&Target>) -> Result<bool, Error> {
        ensure_discovery(self.device_id)?;
        self.succeed(current_tag_snapshot().is_some())
    }

    fn abort_command_driver(&mut self) -> Result<(), Error> {
        let result = self.command_abort.abort();
        self.last_error = if result.is_ok() { NFC_SUCCESS } else { NFC_EIO };
        result
    }

    fn idle_driver(&mut self) -> Result<(), Error> {
        idle_active_device(self.device_id)?;
        self.succeed(())
    }

    fn powerdown_driver(&mut self) -> Result<(), Error> {
        powerdown_active_device(self.device_id)?;
        self.succeed(())
    }
}

impl Pn71xxDevice {
    fn poll_target(
        &mut self,
        modulations: &[Modulation],
        iterations: PollIterations,
        sleep_duration: Duration,
    ) -> Result<Option<Target>, Error> {
        ensure_discovery(self.device_id)?;
        let sleep_duration =
            sleep_duration.min(Duration::from_micros(POLL_PERIOD_FACTOR_MICROS * 15));
        let mut remaining = if iterations.is_continuous() {
            usize::MAX
        } else {
            usize::from(iterations.to_libnfc())
        };
        while remaining > 0 {
            if self.command_abort.take_requested() {
                return Err(self.aborted("pn71xx_poll_target"));
            }
            for modulation in modulations {
                if let Some(target) = self.select_passive_target_once(*modulation)? {
                    if self.command_abort.take_requested() {
                        return Err(self.aborted("pn71xx_poll_target"));
                    }
                    return self.succeed(Some(target));
                }
            }
            if !iterations.is_continuous() {
                remaining -= 1;
            }
            if !sleep_duration.is_zero() {
                self.wait_interruptibly(sleep_duration, "pn71xx_poll_target")?;
            }
        }

        self.succeed(None)
    }

    fn wait_interruptibly(&self, duration: Duration, operation: &'static str) -> Result<(), Error> {
        const ABORT_POLL_INTERVAL: Duration = Duration::from_millis(20);
        let mut remaining = duration;
        while !remaining.is_zero() {
            if self.command_abort.take_requested() {
                return Err(self.aborted(operation));
            }
            let interval = remaining.min(ABORT_POLL_INTERVAL);
            thread::sleep(interval);
            remaining = remaining.saturating_sub(interval);
        }
        Ok(())
    }

    fn select_passive_target(&mut self, modulation: Modulation) -> Result<Option<Target>, Error> {
        ensure_discovery(self.device_id)?;
        loop {
            if self.command_abort.take_requested() {
                return Err(self.aborted("pn71xx_select_passive_target"));
            }
            if let Some(target) = self.select_passive_target_once(modulation)? {
                if self.command_abort.take_requested() {
                    return Err(self.aborted("pn71xx_select_passive_target"));
                }
                return self.succeed(Some(target));
            }
            if !self.infinite_select {
                return self.succeed(None);
            }
            self.wait_interruptibly(Duration::from_millis(20), "pn71xx_select_passive_target")?;
        }
    }

    fn select_passive_target_once(&self, modulation: Modulation) -> Result<Option<Target>, Error> {
        current_tag_snapshot()
            .map(|tag| build_target(&tag, modulation))
            .transpose()
            .map(Option::flatten)
    }

    fn transceive_bytes(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: OperationTimeout,
    ) -> Result<usize, Error> {
        ensure_discovery(self.device_id)?;
        let Some(tag) = current_tag_snapshot() else {
            return self.fail("pn71xx_transceive_bytes", NFC_EINVARG);
        };

        let timeout = timeout.resolve_libnfc_millis(self.timeout_communication_ms)?;
        let received = backend().transceive(tag.handle, tx, rx, timeout);
        if self.command_abort.take_requested() {
            return Err(self.aborted("pn71xx_transceive_bytes"));
        }
        if received <= 0 {
            return self.fail("pn71xx_transceive_bytes", NFC_EIO);
        }

        self.succeed(received as usize)
    }
}

impl TargetBackend for Pn71xxDevice {}

impl Pn53xBackend for Pn71xxDevice {}
