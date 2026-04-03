use std::thread;
use std::time::Duration;

use proximate_driver::{
    BaudRate, ConnectionString, DeviceCaps, DeviceMeta, Error, InfoBackend, InitiatorBackend, Mode,
    Modulation, ModulationType, Pn53xBackend, Property, PropertyBackend, Target, TargetBackend,
};

use super::backend::backend;
use super::consts::{
    DEP_SUPPORTED_BAUD_RATES, FELICA_SUPPORTED_BAUD_RATES, ISO14443A_SUPPORTED_BAUD_RATES,
    ISO14443B_SUPPORTED_BAUD_RATES, JEWEL_SUPPORTED_BAUD_RATES, NFC_EINVARG, NFC_EIO, NFC_SUCCESS,
    PN71XX_DEVICE_NAME, PN71XX_INFO, POLL_PERIOD_FACTOR_MICROS, SUPPORTED_MODULATIONS,
};
use super::device_error;
use super::runtime::{close_active_device, current_tag_snapshot};
use super::target::build_target;

pub(super) struct Pn71xxDevice {
    device_id: u64,
    connstring: ConnectionString,
    last_error: i32,
}

impl Pn71xxDevice {
    pub(super) fn new(device_id: u64, connstring: ConnectionString) -> Self {
        Self {
            device_id,
            connstring,
            last_error: NFC_SUCCESS,
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
}

impl Drop for Pn71xxDevice {
    fn drop(&mut self) {
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

    fn caps(&self) -> DeviceCaps {
        DeviceCaps::INFO
            | DeviceCaps::SET_PROPERTY_BOOL
            | DeviceCaps::SET_PROPERTY_INT
            | DeviceCaps::SUPPORTED_MODULATIONS
            | DeviceCaps::SUPPORTED_BAUD_RATES
            | DeviceCaps::INITIATOR_INIT
            | DeviceCaps::SELECT_PASSIVE_TARGET
            | DeviceCaps::POLL_TARGET
            | DeviceCaps::DESELECT_TARGET
            | DeviceCaps::TARGET_IS_PRESENT
            | DeviceCaps::TRANSCEIVE_BYTES
            | DeviceCaps::ABORT_COMMAND
            | DeviceCaps::IDLE
            | DeviceCaps::POWERDOWN
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
    fn set_property_bool(&mut self, _property: Property, _enable: bool) -> Result<(), Error> {
        self.succeed(())
    }

    fn set_property_int(&mut self, _property: Property, _value: i32) -> Result<(), Error> {
        self.succeed(())
    }

    fn supported_modulations(&mut self, _mode: Mode) -> Result<Vec<ModulationType>, Error> {
        self.succeed(SUPPORTED_MODULATIONS.to_vec())
    }

    fn supported_baud_rates(
        &mut self,
        _mode: Mode,
        modulation_type: ModulationType,
    ) -> Result<Vec<BaudRate>, Error> {
        let values = match modulation_type {
            ModulationType::Felica => FELICA_SUPPORTED_BAUD_RATES,
            ModulationType::Iso14443A => ISO14443A_SUPPORTED_BAUD_RATES,
            ModulationType::Iso14443B
            | ModulationType::Iso14443Bi
            | ModulationType::Iso14443B2Sr
            | ModulationType::Iso14443B2Ct => ISO14443B_SUPPORTED_BAUD_RATES,
            ModulationType::Jewel => JEWEL_SUPPORTED_BAUD_RATES,
            ModulationType::Dep => DEP_SUPPORTED_BAUD_RATES,
            _ => return self.fail("pn71xx_get_supported_baud_rate", NFC_EINVARG),
        };
        self.succeed(values.to_vec())
    }
}

impl InitiatorBackend for Pn71xxDevice {
    fn initiator_init_driver(&mut self) -> Result<i32, Error> {
        self.succeed(0)
    }

    fn select_passive_target_driver(
        &mut self,
        modulation: Modulation,
        _init_data: &[u8],
    ) -> Result<Option<Target>, Error> {
        self.succeed(current_tag_snapshot().and_then(|tag| build_target(&tag, modulation)))
    }

    fn poll_target_driver(
        &mut self,
        modulations: &[Modulation],
        poll_nr: u8,
        period: u8,
    ) -> Result<Option<Target>, Error> {
        let sleep_duration = Duration::from_micros(period as u64 * POLL_PERIOD_FACTOR_MICROS);
        for _ in 0..poll_nr {
            for modulation in modulations {
                if let Some(target) = self.select_passive_target_driver(*modulation, &[])? {
                    return self.succeed(Some(target));
                }
            }
            if !sleep_duration.is_zero() {
                thread::sleep(sleep_duration);
            }
        }

        self.succeed(None)
    }

    fn deselect_target_driver(&mut self) -> Result<(), Error> {
        self.succeed(())
    }

    fn transceive_bytes_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        _timeout: i32,
    ) -> Result<usize, Error> {
        let Some(tag) = current_tag_snapshot() else {
            return self.fail("pn71xx_transceive_bytes", NFC_EINVARG);
        };

        let received = backend().transceive(tag.handle, tx, rx, 500);
        if received <= 0 {
            return self.fail("pn71xx_transceive_bytes", NFC_EIO);
        }

        self.succeed(received as usize)
    }

    fn target_is_present_driver(&mut self, _target: Option<&Target>) -> Result<bool, Error> {
        self.succeed(current_tag_snapshot().is_some())
    }

    fn abort_command_driver(&mut self) -> Result<(), Error> {
        self.succeed(())
    }

    fn idle_driver(&mut self) -> Result<(), Error> {
        self.succeed(())
    }

    fn powerdown_driver(&mut self) -> Result<(), Error> {
        self.succeed(())
    }
}

impl TargetBackend for Pn71xxDevice {}

impl Pn53xBackend for Pn71xxDevice {}
