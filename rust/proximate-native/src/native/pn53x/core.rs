use super::*;

pub(crate) struct Pn53xCore {
    pub(super) chip_type: Pn53xType,
    pub(super) firmware: Option<Pn53xFirmwareVersion>,
    pub(super) power_mode: Pn53xPowerMode,
    pub(super) last_command: Option<u8>,
    pub(super) last_status_byte: u8,
    pub(super) tx_bits: u8,
    pub(super) timer_prescaler: u16,
    pub(super) timeout_command_ms: i32,
    pub(super) timeout_atr_ms: i32,
    pub(super) timeout_communication_ms: i32,
    pub(super) properties: PropertyState,
    pub(super) current_target: Option<Target>,
}

impl Default for Pn53xCore {
    fn default() -> Self {
        Self {
            chip_type: Pn53xType::Unknown,
            firmware: None,
            power_mode: Pn53xPowerMode::LowVbat,
            last_command: None,
            last_status_byte: 0,
            tx_bits: 0,
            timer_prescaler: 0,
            timeout_command_ms: 500,
            timeout_atr_ms: 103,
            timeout_communication_ms: 52,
            properties: PropertyState::default(),
            current_target: None,
        }
    }
}

impl Pn53xCore {
    fn exchange_prepared_command<T: Pn53xTransport>(
        &mut self,
        transport: &mut T,
        command: u8,
        payload: &[u8],
        timeout_ms: i32,
    ) -> Result<Vec<u8>, Error> {
        let mut command_payload = Vec::with_capacity(payload.len() + 1);
        command_payload.push(command);
        command_payload.extend_from_slice(payload);

        let frame = build_frame(&command_payload)?;
        transport.send(&frame, timeout_ms)?;

        let mut ack = [0u8; PN53X_ACK_FRAME.len()];
        let ack_len = transport.receive(&mut ack, timeout_ms)?;
        if !is_ack_frame(&ack[..ack_len]) {
            return Err(status_error("pn53x_wait_for_ack", NFC_EIO));
        }

        let mut response = [0u8; PN532_BUFFER_LEN];
        let response_len = transport.receive(&mut response, timeout_ms)?;
        let payload = parse_response_frame(&response[..response_len], command)?;
        self.last_command = Some(command);
        Ok(payload)
    }

    fn ensure_ready<T: Pn53xTransport>(
        &mut self,
        profile: Pn53xProfile,
        transport: &mut T,
        timeout_ms: i32,
    ) -> Result<(), Error> {
        if self.power_mode == Pn53xPowerMode::Normal {
            return Ok(());
        }

        let previous_mode = self.power_mode;
        transport.wake_up()?;
        self.power_mode = Pn53xPowerMode::Normal;

        if previous_mode == Pn53xPowerMode::LowVbat
            && let Some(mode) = profile.sam_mode_on_low_vbat
        {
            let payload = match mode {
                Pn532SamMode::Normal => [mode as u8, 0x00],
                Pn532SamMode::WiredCard => [mode as u8, 0x00],
                Pn532SamMode::VirtualCard => [mode as u8, 0x00],
                Pn532SamMode::DualCard => [mode as u8, 0x00],
            };
            let _ = self.exchange_prepared_command(
                transport,
                PN532_SAM_CONFIGURATION,
                &payload,
                timeout_ms,
            )?;
        }

        Ok(())
    }

    pub(crate) fn chip_type(&self) -> Pn53xType {
        self.chip_type
    }

    pub(crate) fn firmware(&self) -> Option<&Pn53xFirmwareVersion> {
        self.firmware.as_ref()
    }

    pub(crate) fn power_mode(&self) -> Pn53xPowerMode {
        self.power_mode
    }

    pub(crate) fn last_command(&self) -> Option<u8> {
        self.last_command
    }

    pub(crate) fn property_bool_state(&self, property: Property) -> Option<bool> {
        self.properties.get(property)
    }

    pub(crate) fn current_target(&self) -> Option<&Target> {
        self.current_target.as_ref()
    }

    pub(super) fn remember_target(&mut self, target: Target) {
        self.current_target = Some(target);
    }

    pub(super) fn clear_target(&mut self) {
        self.current_target = None;
    }

    pub(crate) fn set_property_bool(
        &mut self,
        property: Property,
        enable: bool,
    ) -> Result<(), Error> {
        self.properties.set(property, enable)
    }

    pub(crate) fn set_property_int(&mut self, property: Property, value: i32) -> Result<(), Error> {
        match property {
            Property::TimeoutCommand => self.timeout_command_ms = value,
            Property::TimeoutAtr => self.timeout_atr_ms = value,
            Property::TimeoutCom => self.timeout_communication_ms = value,
            _ => return Err(Error::InvalidArgument("property")),
        }
        Ok(())
    }

    pub(crate) fn exchange_command<T: Pn53xTransport>(
        &mut self,
        profile: Pn53xProfile,
        transport: &mut T,
        command: u8,
        payload: &[u8],
        timeout_ms: i32,
    ) -> Result<Vec<u8>, Error> {
        self.ensure_ready(profile, transport, timeout_ms)?;
        self.exchange_prepared_command(transport, command, payload, timeout_ms)
    }

    pub(crate) fn get_firmware_version<T: Pn53xTransport>(
        &mut self,
        profile: Pn53xProfile,
        transport: &mut T,
        timeout_ms: i32,
    ) -> Result<Pn53xFirmwareVersion, Error> {
        let payload = self.exchange_command(
            profile,
            transport,
            PN53X_GET_FIRMWARE_VERSION,
            &[],
            timeout_ms,
        )?;
        if payload.len() < 4 {
            return Err(status_error("pn53x_get_firmware_version", NFC_EIO));
        }

        let firmware = Pn53xFirmwareVersion {
            ic: payload[0],
            version: payload[1],
            revision: payload[2],
            support: payload[3],
        };
        self.chip_type = firmware.chip_type();
        self.last_status_byte = payload.get(4).copied().unwrap_or(0);
        self.firmware = Some(firmware.clone());
        Ok(firmware)
    }
}
