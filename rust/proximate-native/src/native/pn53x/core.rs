/*-
 * Free/Libre Near Field Communication (NFC) library
 *
 * Libnfc historical contributors:
 * Copyright (C) 2009      Roel Verdult
 * Copyright (C) 2009-2013 Romuald Conty
 * Copyright (C) 2010-2012 Romain Tartière
 * Copyright (C) 2010-2013 Philippe Teuwen
 * Copyright (C) 2012-2013 Ludovic Rousseau
 * See AUTHORS file for a more comprehensive list of contributors.
 * Additional contributors of this file:
 * Copyright (C) 2020      Adam Laurie
 *
 * This program is free software: you can redistribute it and/or modify it
 * under the terms of the GNU Lesser General Public License as published by the
 * Free Software Foundation, either version 3 of the License, or (at your
 * option) any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
 * more details.
 *
 * You should have received a copy of the GNU Lesser General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>
 */

use super::*;

pub(crate) struct Pn53xCore {
    pub(super) capabilities: Option<ChipCapabilities>,
    pub(super) power_mode: Pn53xPowerMode,
    pub(super) operating_mode: Pn53xOperatingMode,
    pub(super) protocol_state: Pn53xProtocolState,
    pub(super) last_command: Option<u8>,
    pub(super) last_status_byte: u8,
    pub(super) tx_bits: u8,
    pub(super) timer_prescaler: u16,
    pub(super) command_timeout: OperationTimeout,
    pub(super) atr_timeout: OperationTimeout,
    pub(super) communication_timeout: OperationTimeout,
    pub(super) properties: PropertyState,
    pub(super) parameters: u8,
    pub(super) current_target: Option<Target>,
}

impl Default for Pn53xCore {
    fn default() -> Self {
        Self {
            capabilities: None,
            power_mode: Pn53xPowerMode::LowVbat,
            operating_mode: Pn53xOperatingMode::Idle,
            protocol_state: Pn53xProtocolState::Ready,
            last_command: None,
            last_status_byte: 0,
            tx_bits: 0,
            timer_prescaler: 0,
            command_timeout: OperationTimeout::try_milliseconds(500)
                .expect("default command timeout is representable"),
            atr_timeout: OperationTimeout::try_milliseconds(103)
                .expect("default ATR timeout is representable"),
            communication_timeout: OperationTimeout::try_milliseconds(52)
                .expect("default communication timeout is representable"),
            properties: PropertyState::default(),
            parameters: 0,
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
        timeout: OperationTimeout,
    ) -> Result<Vec<u8>, Error> {
        let mut command_payload = Vec::with_capacity(payload.len() + 1);
        command_payload.push(command);
        command_payload.extend_from_slice(payload);

        let frame = build_frame(&command_payload)?;
        match transport.send(&frame, timeout) {
            Ok(()) => {}
            Err(TransportSendError::ProtocolStable(error)) => return Err(error),
            Err(TransportSendError::OutcomeUnknown(error @ Error::RecoveryFailed { .. })) => {
                return Err(self.require_reinitialization(error));
            }
            Err(TransportSendError::OutcomeUnknown(cause)) => {
                return Err(self.require_recovery("pn53x_send", cause));
            }
        }
        self.last_command = Some(command);

        let mut ack = [0u8; PN53X_ACK_FRAME.len()];
        let ack_len = match transport.receive(&mut ack, timeout) {
            Ok(length) => length,
            Err(error @ Error::Aborted(_)) => return Err(error),
            Err(error @ Error::RecoveryFailed { .. }) => {
                return Err(self.require_reinitialization(error));
            }
            Err(error) => return Err(self.require_recovery("pn53x_wait_for_ack", error)),
        };
        if !is_ack_frame(&ack[..ack_len]) {
            return Err(self.require_recovery(
                "pn53x_wait_for_ack",
                Error::InvalidEncoding("PN53x ACK frame"),
            ));
        }

        let mut response = [0u8; PN532_BUFFER_LEN];
        let response_len = match transport.receive(&mut response, timeout) {
            Ok(length) => length,
            Err(error @ Error::Aborted(_)) => return Err(error),
            Err(error @ Error::RecoveryFailed { .. }) => {
                return Err(self.require_reinitialization(error));
            }
            Err(error) => return Err(self.require_recovery("pn53x_wait_for_response", error)),
        };
        let payload = match parse_response_frame(&response[..response_len], command) {
            Ok(payload) => payload,
            Err(error) => return Err(self.require_recovery("pn53x_parse_response", error)),
        };
        Ok(payload)
    }

    fn require_recovery(&mut self, operation: &'static str, cause: Error) -> Error {
        let error = Error::OutcomeUnknown {
            operation,
            cause: Box::new(cause),
        };
        self.require_reinitialization(error)
    }

    fn require_reinitialization(&mut self, error: Error) -> Error {
        self.protocol_state = Pn53xProtocolState::NeedsReinitialization {
            cause: error.clone(),
        };
        error
    }

    fn read_registers_prepared<T: Pn53xTransport>(
        &mut self,
        transport: &mut T,
        registers: &[u16],
        timeout: OperationTimeout,
    ) -> Result<Vec<u8>, Error> {
        let mut command = Vec::with_capacity(registers.len() * 2);
        for register in registers {
            command.push((register >> 8) as u8);
            command.push(*register as u8);
        }
        let response =
            self.exchange_prepared_command(transport, PN53X_READ_REGISTER, &command, timeout)?;
        let values = if self.chip_type() == Pn53xType::Pn533 {
            let (status, data) = split_status_response(PN53X_READ_REGISTER, &response)?;
            self.last_status_byte = status;
            let mapped = pn53x_translate_status(status);
            if mapped < 0 {
                return Err(status_error("pn53x_restore_read_register", mapped));
            }
            data
        } else {
            response
        };
        if values.len() < registers.len() {
            return Err(Error::InvalidEncoding("ReadRegister response"));
        }
        Ok(values[..registers.len()].to_vec())
    }

    fn apply_register_masks_prepared<T: Pn53xTransport>(
        &mut self,
        transport: &mut T,
        updates: &[(u16, u8, u8)],
        timeout: OperationTimeout,
    ) -> Result<(), Error> {
        let registers: Vec<u16> = updates.iter().map(|(register, _, _)| *register).collect();
        let current = self.read_registers_prepared(transport, &registers, timeout)?;
        let writes: Vec<(u16, u8)> = updates
            .iter()
            .zip(current)
            .filter_map(|(&(register, mask, value), current)| {
                let next = (current & !mask) | (value & mask);
                (current != next).then_some((register, next))
            })
            .collect();
        if writes.is_empty() {
            return Ok(());
        }

        let mut command = Vec::with_capacity(writes.len() * 3);
        for (register, value) in writes {
            command.push((register >> 8) as u8);
            command.push(register as u8);
            command.push(value);
        }
        let _ =
            self.exchange_prepared_command(transport, PN53X_WRITE_REGISTER, &command, timeout)?;
        Ok(())
    }

    fn reset_frame_settings_prepared<T: Pn53xTransport>(
        &mut self,
        transport: &mut T,
        timeout: OperationTimeout,
    ) -> Result<(), Error> {
        self.apply_register_masks_prepared(
            transport,
            &[
                (
                    PN53X_REG_CIU_TX_MODE,
                    SYMBOL_TX_CRC_ENABLE,
                    SYMBOL_TX_CRC_ENABLE,
                ),
                (
                    PN53X_REG_CIU_RX_MODE,
                    SYMBOL_RX_CRC_ENABLE,
                    SYMBOL_RX_CRC_ENABLE,
                ),
                (PN53X_REG_CIU_MANUAL_RCV, SYMBOL_PARITY_DISABLE, 0x00),
                (PN53X_REG_CIU_STATUS2, SYMBOL_MF_CRYPTO1_ON, 0x00),
                (PN53X_REG_CIU_BIT_FRAMING, SYMBOL_TX_LAST_BITS, 0x00),
            ],
            timeout,
        )?;
        self.tx_bits = 0;
        self.properties.handle_crc = true;
        self.properties.handle_parity = true;
        self.properties.easy_framing = true;
        self.properties.activate_crypto1 = false;
        Ok(())
    }

    fn restore_protocol_defaults_prepared<T: Pn53xTransport>(
        &mut self,
        transport: &mut T,
        timeout: OperationTimeout,
    ) -> Result<(), Error> {
        self.apply_register_masks_prepared(
            transport,
            &[
                (
                    PN53X_REG_CIU_TX_MODE,
                    SYMBOL_TX_CRC_ENABLE | SYMBOL_TX_SPEED | SYMBOL_TX_FRAMING,
                    SYMBOL_TX_CRC_ENABLE,
                ),
                (
                    PN53X_REG_CIU_RX_MODE,
                    SYMBOL_RX_CRC_ENABLE
                        | SYMBOL_RX_SPEED
                        | SYMBOL_RX_FRAMING
                        | SYMBOL_RX_NO_ERROR
                        | SYMBOL_RX_MULTIPLE,
                    SYMBOL_RX_CRC_ENABLE,
                ),
                (
                    PN53X_REG_CIU_TX_AUTO,
                    SYMBOL_FORCE_100_ASK,
                    SYMBOL_FORCE_100_ASK,
                ),
                (PN53X_REG_CIU_MANUAL_RCV, SYMBOL_PARITY_DISABLE, 0x00),
                (PN53X_REG_CIU_STATUS2, SYMBOL_MF_CRYPTO1_ON, 0x00),
                (PN53X_REG_CIU_BIT_FRAMING, SYMBOL_TX_LAST_BITS, 0x00),
            ],
            timeout,
        )?;
        let _ = self.exchange_prepared_command(
            transport,
            PN53X_RF_CONFIGURATION,
            &[RFCI_FIELD, 0x01],
            timeout,
        )?;
        let _ = self.exchange_prepared_command(
            transport,
            PN53X_RF_CONFIGURATION,
            &[RFCI_RETRY_SELECT, 0x00, 0x01, 0x02],
            timeout,
        )?;
        self.tx_bits = 0;
        self.properties = PropertyState::default();
        Ok(())
    }

    fn recover<T: Pn53xTransport>(
        &mut self,
        transport: &mut T,
        timeout: OperationTimeout,
    ) -> Result<(), Error> {
        let Some(operation) = self.protocol_state.recovery_cause().cloned() else {
            return Ok(());
        };
        let recovery = (|| {
            transport.wake_up()?;
            self.power_mode = Pn53xPowerMode::Normal;
            let payload = self.exchange_prepared_command(
                transport,
                PN53X_GET_FIRMWARE_VERSION,
                &[],
                timeout,
            )?;
            self.capabilities = Some(ChipCapabilities::from_firmware_response(&payload)?);
            let _ = self.exchange_prepared_command(
                transport,
                PN53X_SET_PARAMETERS,
                &[PARAM_AUTO_ATR_RES | PARAM_AUTO_RATS],
                timeout,
            )?;
            self.parameters = PARAM_AUTO_ATR_RES | PARAM_AUTO_RATS;
            self.restore_protocol_defaults_prepared(transport, timeout)?;
            self.current_target = None;
            self.operating_mode = Pn53xOperatingMode::Idle;
            self.protocol_state = Pn53xProtocolState::Ready;
            Ok(())
        })();
        recovery.map_err(|recovery| Error::RecoveryFailed {
            operation: Box::new(operation),
            recovery: Box::new(recovery),
        })
    }

    fn ensure_ready<T: Pn53xTransport>(
        &mut self,
        profile: Pn53xProfile,
        transport: &mut T,
        timeout: OperationTimeout,
    ) -> Result<(), Error> {
        if matches!(
            self.protocol_state,
            Pn53xProtocolState::NeedsReinitialization { .. }
        ) {
            return self.recover(transport, timeout);
        }
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
                timeout,
            )?;
        }

        Ok(())
    }

    pub(super) fn reset_frame_settings<T: Pn53xTransport>(
        &mut self,
        profile: Pn53xProfile,
        transport: &mut T,
        timeout: OperationTimeout,
    ) -> Result<(), Error> {
        self.ensure_ready(profile, transport, timeout)?;
        self.reset_frame_settings_prepared(transport, timeout)
    }

    pub(crate) fn chip_type(&self) -> Pn53xType {
        self.capabilities
            .as_ref()
            .map_or(Pn53xType::Unknown, ChipCapabilities::chip_type)
    }

    pub(crate) fn firmware(&self) -> Option<&Pn53xFirmwareVersion> {
        self.capabilities.as_ref().map(ChipCapabilities::firmware)
    }

    pub(crate) fn capabilities(&self) -> Option<&ChipCapabilities> {
        self.capabilities.as_ref()
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

    pub(crate) fn exchange_command<T: Pn53xTransport>(
        &mut self,
        profile: Pn53xProfile,
        transport: &mut T,
        command: u8,
        payload: &[u8],
        timeout: OperationTimeout,
    ) -> Result<Vec<u8>, Error> {
        self.ensure_ready(profile, transport, timeout)?;
        self.exchange_prepared_command(transport, command, payload, timeout)
    }

    pub(crate) fn get_firmware_version<T: Pn53xTransport>(
        &mut self,
        profile: Pn53xProfile,
        transport: &mut T,
        timeout: OperationTimeout,
    ) -> Result<Pn53xFirmwareVersion, Error> {
        let payload =
            self.exchange_command(profile, transport, PN53X_GET_FIRMWARE_VERSION, &[], timeout)?;
        let capabilities = ChipCapabilities::from_firmware_response(&payload)?;
        let firmware = capabilities.firmware().clone();
        self.last_status_byte = 0;
        self.capabilities = Some(capabilities);
        Ok(firmware)
    }
}
