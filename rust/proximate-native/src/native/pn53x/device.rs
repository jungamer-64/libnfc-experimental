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

pub(crate) struct Pn53xDevice<T> {
    name: String,
    connstring: ConnectionString,
    profile: Pn53xProfile,
    pub(super) transport: T,
    pub(super) core: Pn53xCore,
    last_error: i32,
}

impl<T: Pn53xTransport + Send + 'static> Pn53xDevice<T> {
    pub(crate) fn probe_with_profile(
        name: impl Into<String>,
        connstring: ConnectionString,
        profile: Pn53xProfile,
        mut transport: T,
        timeout_ms: i32,
    ) -> Result<Self, Error> {
        let mut core = Pn53xCore {
            power_mode: profile.initial_power_mode,
            ..Pn53xCore::default()
        };
        core.get_firmware_version(profile, &mut transport, timeout_ms)?;
        let mut device = Self {
            name: name.into(),
            connstring,
            profile,
            transport,
            core,
            last_error: 0,
        };
        let _ = device.exchange_raw(
            PN53X_SET_PARAMETERS,
            &[PARAM_AUTO_ATR_RES | PARAM_AUTO_RATS],
            timeout_ms,
        )?;
        device.core.parameters = PARAM_AUTO_ATR_RES | PARAM_AUTO_RATS;
        device
            .core
            .reset_frame_settings(device.profile, &mut device.transport, timeout_ms)?;
        Ok(device)
    }

    pub(crate) fn probe_pn532(
        name: impl Into<String>,
        connstring: ConnectionString,
        transport: T,
        timeout_ms: i32,
    ) -> Result<Self, Error> {
        Self::probe_with_profile(
            name,
            connstring,
            Pn53xProfile::pn532("pn532"),
            transport,
            timeout_ms,
        )
    }

    #[allow(dead_code)]
    pub(crate) fn core(&self) -> &Pn53xCore {
        &self.core
    }

    fn remember<TValue>(&mut self, result: Result<TValue, Error>) -> Result<TValue, Error> {
        match &result {
            Ok(_) => self.last_error = 0,
            Err(error) => self.last_error = status_code(error),
        }
        result
    }

    fn resolve_timeout(timeout: OperationTimeout, default_millis: i32) -> Result<i32, Error> {
        let default_millis = u32::try_from(default_millis)
            .map_err(|_| Error::InvalidArgument("configured timeout"))?;
        timeout.resolve_libnfc_millis(default_millis)
    }

    fn firmware_text(&self) -> String {
        self.core
            .firmware()
            .map(Pn53xFirmwareVersion::text)
            .unwrap_or_else(|| format!("{} firmware unknown", self.core.chip_type().label()))
    }

    fn sam_configuration(&mut self, mode: Pn532SamMode, timeout_ms: i32) -> Result<i32, Error> {
        if self.core.chip_type() != Pn53xType::Pn532 {
            return self.remember(Err(status_error("pn532_SAMConfiguration", NFC_EDEVNOTSUPP)));
        }
        let payload = match mode {
            Pn532SamMode::Normal => [mode as u8, 0x00],
            Pn532SamMode::WiredCard => [mode as u8, 0x00],
            Pn532SamMode::VirtualCard => [mode as u8, 0x00],
            Pn532SamMode::DualCard => [mode as u8, 0x00],
        };
        let result = self
            .core
            .exchange_command(
                self.profile,
                &mut self.transport,
                PN532_SAM_CONFIGURATION,
                &payload,
                timeout_ms,
            )
            .map(|_| 0);
        self.remember(result)
    }

    fn exchange_raw(
        &mut self,
        command: u8,
        payload: &[u8],
        timeout_ms: i32,
    ) -> Result<Vec<u8>, Error> {
        let result = self.core.exchange_command(
            self.profile,
            &mut self.transport,
            command,
            payload,
            timeout_ms,
        );
        self.remember(result)
    }

    fn exchange_with_status(
        &mut self,
        operation: &'static str,
        command: u8,
        payload: &[u8],
        timeout_ms: i32,
    ) -> Result<Vec<u8>, Error> {
        let response = self.exchange_raw(command, payload, timeout_ms)?;
        let (status, data) = split_status_response(command, &response)?;
        self.core.last_status_byte = status;
        let mapped = pn53x_translate_status(status);
        if mapped < 0 {
            self.last_error = mapped;
            return Err(status_error(operation, mapped));
        }
        self.last_error = 0;
        Ok(data)
    }

    fn copy_into(
        operation: &'static str,
        source: &[u8],
        destination: &mut [u8],
    ) -> Result<usize, Error> {
        if source.len() > destination.len() {
            return Err(Error::DeviceOperationFailed {
                operation,
                code: NFC_EOVFLOW,
            });
        }
        destination[..source.len()].copy_from_slice(source);
        Ok(source.len())
    }

    fn read_register(&mut self, register: u16) -> Result<u8, Error> {
        let values = self.read_registers(&[register])?;
        values
            .into_iter()
            .next()
            .ok_or_else(|| status_error("read_register", NFC_EIO))
    }

    fn write_register(&mut self, register: u16, value: u8) -> Result<(), Error> {
        self.write_registers(&[(register, value)])
    }

    fn read_registers(&mut self, registers: &[u16]) -> Result<Vec<u8>, Error> {
        if registers.is_empty() {
            return Ok(Vec::new());
        }
        let mut payload = Vec::with_capacity(registers.len() * 2);
        for register in registers {
            payload.push((register >> 8) as u8);
            payload.push(*register as u8);
        }
        let response =
            self.exchange_raw(PN53X_READ_REGISTER, &payload, self.core.timeout_command_ms)?;
        let values = if self.core.chip_type() == Pn53xType::Pn533 {
            let (status, data) = split_status_response(PN53X_READ_REGISTER, &response)?;
            self.core.last_status_byte = status;
            let mapped = pn53x_translate_status(status);
            if mapped < 0 {
                return self.remember(Err(status_error("read_register", mapped)));
            }
            data
        } else {
            response
        };
        if values.len() < registers.len() {
            return self.remember(Err(status_error("read_register", NFC_EIO)));
        }
        Ok(values[..registers.len()].to_vec())
    }

    fn write_registers(&mut self, writes: &[(u16, u8)]) -> Result<(), Error> {
        if writes.is_empty() {
            return Ok(());
        }
        let mut payload = Vec::with_capacity(writes.len() * 3);
        for (register, value) in writes {
            payload.push((register >> 8) as u8);
            payload.push(*register as u8);
            payload.push(*value);
        }
        let _ = self.exchange_raw(PN53X_WRITE_REGISTER, &payload, self.core.timeout_command_ms)?;
        Ok(())
    }

    fn update_register_bits(&mut self, register: u16, mask: u8, value: u8) -> Result<(), Error> {
        let current = self.read_register(register)?;
        let next = (current & !mask) | (value & mask);
        if current != next {
            self.write_register(register, next)?;
        }
        Ok(())
    }

    fn update_register_masks(&mut self, updates: &[(u16, u8, u8)]) -> Result<(), Error> {
        let registers: Vec<u16> = updates.iter().map(|(register, _, _)| *register).collect();
        let current = self.read_registers(&registers)?;
        let writes: Vec<(u16, u8)> = updates
            .iter()
            .zip(current)
            .filter_map(|(&(register, mask, value), current)| {
                let next = (current & !mask) | (value & mask);
                (current != next).then_some((register, next))
            })
            .collect();
        self.write_registers(&writes)
    }

    fn set_parameters(&mut self, mask: u8, enable: bool) -> Result<(), Error> {
        let next = if enable {
            self.core.parameters | mask
        } else {
            self.core.parameters & !mask
        };
        if next == self.core.parameters {
            return Ok(());
        }
        let _ = self.exchange_raw(PN53X_SET_PARAMETERS, &[next], self.core.timeout_command_ms)?;
        self.core.parameters = next;
        Ok(())
    }

    fn rf_configuration(&mut self, payload: &[u8]) -> Result<(), Error> {
        let _ = self.exchange_raw(
            PN53X_RF_CONFIGURATION,
            payload,
            self.core.timeout_command_ms,
        )?;
        Ok(())
    }

    fn int_to_timeout(milliseconds: i32) -> u8 {
        if milliseconds == 0 {
            return 0;
        }
        let mut encoded = 0x10u8;
        let mut threshold = 3280;
        while threshold > 1 {
            if milliseconds > threshold {
                break;
            }
            encoded = encoded.saturating_sub(1);
            threshold /= 2;
        }
        encoded
    }

    /// Applies a boolean property after its chip response is confirmed.
    ///
    /// The three `Force*` properties are one-shot framing commands rather than
    /// readable state: enabling one changes registers, while disabling one
    /// intentionally preserves the framing already selected on the chip.
    fn apply_property_bool(&mut self, property: Property, enable: bool) -> Result<(), Error> {
        if self.core.property_bool_state(property) == Some(enable) {
            return Ok(());
        }

        match property {
            Property::HandleCrc => {
                let value = if enable { SYMBOL_TX_CRC_ENABLE } else { 0 };
                self.update_register_masks(&[
                    (PN53X_REG_CIU_TX_MODE, SYMBOL_TX_CRC_ENABLE, value),
                    (PN53X_REG_CIU_RX_MODE, SYMBOL_RX_CRC_ENABLE, value),
                ])?;
            }
            Property::HandleParity => {
                let value = if enable { 0 } else { SYMBOL_PARITY_DISABLE };
                self.update_register_bits(PN53X_REG_CIU_MANUAL_RCV, SYMBOL_PARITY_DISABLE, value)?;
            }
            Property::EasyFraming => {}
            Property::ActivateField => {
                self.rf_configuration(&[RFCI_FIELD, u8::from(enable)])?;
            }
            Property::ActivateCrypto1 => {
                let value = if enable { SYMBOL_MF_CRYPTO1_ON } else { 0 };
                self.update_register_bits(PN53X_REG_CIU_STATUS2, SYMBOL_MF_CRYPTO1_ON, value)?;
            }
            Property::InfiniteSelect => {
                let retries = if enable {
                    [0xff, 0xff, 0xff]
                } else {
                    [0x00, 0x01, 0x02]
                };
                self.rf_configuration(&[RFCI_RETRY_SELECT, retries[0], retries[1], retries[2]])?;
            }
            Property::AcceptInvalidFrames => {
                let value = if enable { SYMBOL_RX_NO_ERROR } else { 0 };
                self.update_register_bits(PN53X_REG_CIU_RX_MODE, SYMBOL_RX_NO_ERROR, value)?;
            }
            Property::AcceptMultipleFrames => {
                let value = if enable { SYMBOL_RX_MULTIPLE } else { 0 };
                self.update_register_bits(PN53X_REG_CIU_RX_MODE, SYMBOL_RX_MULTIPLE, value)?;
            }
            Property::AutoIso14443_4 => self.set_parameters(PARAM_AUTO_RATS, enable)?,
            Property::ForceIso14443A => {
                if enable {
                    self.update_register_masks(&[
                        (PN53X_REG_CIU_TX_MODE, SYMBOL_TX_FRAMING, 0x00),
                        (PN53X_REG_CIU_RX_MODE, SYMBOL_RX_FRAMING, 0x00),
                        (PN53X_REG_CIU_TX_AUTO, SYMBOL_FORCE_100_ASK, 0x40),
                    ])?;
                }
                return Ok(());
            }
            Property::ForceIso14443B => {
                if enable {
                    self.update_register_masks(&[
                        (PN53X_REG_CIU_TX_MODE, SYMBOL_TX_FRAMING, 0x03),
                        (PN53X_REG_CIU_RX_MODE, SYMBOL_RX_FRAMING, 0x03),
                    ])?;
                }
                return Ok(());
            }
            Property::ForceSpeed106 => {
                if enable {
                    self.update_register_masks(&[
                        (PN53X_REG_CIU_TX_MODE, SYMBOL_TX_SPEED, 0x00),
                        (PN53X_REG_CIU_RX_MODE, SYMBOL_RX_SPEED, 0x00),
                    ])?;
                }
                return Ok(());
            }
            Property::TimeoutCommand | Property::TimeoutAtr | Property::TimeoutCom => {
                return Err(Error::InvalidArgument("property"));
            }
        }

        self.core.set_property_bool(property, enable)
    }

    fn apply_property_int(&mut self, property: Property, value: i32) -> Result<(), Error> {
        match property {
            Property::TimeoutCommand => self.core.timeout_command_ms = value,
            Property::TimeoutAtr | Property::TimeoutCom => {
                let atr = if property == Property::TimeoutAtr {
                    value
                } else {
                    self.core.timeout_atr_ms
                };
                let communication = if property == Property::TimeoutCom {
                    value
                } else {
                    self.core.timeout_communication_ms
                };
                self.rf_configuration(&[
                    RFCI_TIMING,
                    0x00,
                    Self::int_to_timeout(atr),
                    Self::int_to_timeout(communication),
                ])?;
                self.core.timeout_atr_ms = atr;
                self.core.timeout_communication_ms = communication;
            }
            _ => return Err(Error::InvalidArgument("property")),
        }
        Ok(())
    }

    fn set_tx_bits(&mut self, bits: u8) -> Result<(), Error> {
        let bits = bits & SYMBOL_TX_LAST_BITS;
        if self.core.tx_bits == bits {
            return Ok(());
        }
        self.update_register_bits(PN53X_REG_CIU_BIT_FRAMING, SYMBOL_TX_LAST_BITS, bits)?;
        self.core.tx_bits = bits;
        Ok(())
    }

    fn reset_settings(&mut self) -> Result<(), Error> {
        self.core.reset_frame_settings(
            self.profile,
            &mut self.transport,
            self.core.timeout_command_ms,
        )
    }

    fn rx_last_bits(&mut self) -> Result<u8, Error> {
        Ok(self.read_register(PN53X_REG_CIU_CONTROL)? & SYMBOL_RX_LAST_BITS)
    }

    fn init_timer(&mut self, max_cycles: u32) -> Result<(), Error> {
        self.core.timer_prescaler = if max_cycles > 0xFFFF {
            (((max_cycles / 0xFFFF).saturating_sub(1)) / 2) as u16
        } else {
            0
        };
        self.write_registers(&[
            (
                PN53X_REG_CIU_TMODE,
                SYMBOL_TAUTO | (((self.core.timer_prescaler >> 8) as u8) & SYMBOL_TPRESCALERHI),
            ),
            (
                PN53X_REG_CIU_TPRESCALER,
                (self.core.timer_prescaler as u8) & SYMBOL_TPRESCALERLO,
            ),
            (PN53X_REG_CIU_TRELOAD_VAL_HI, 0xff),
            (PN53X_REG_CIU_TRELOAD_VAL_LO, 0xff),
        ])
    }

    fn timer_cycles(&mut self, last_cmd_byte: u8) -> Result<u32, Error> {
        let values =
            self.read_registers(&[PN53X_REG_CIU_TCOUNTER_VAL_HI, PN53X_REG_CIU_TCOUNTER_VAL_LO])?;
        let counter = u16::from(values[0]) << 8 | u16::from(values[1]);
        if counter == 0 {
            return Ok(u32::MAX);
        }

        let mut cycles = u32::from(0xFFFFu16 - counter);
        cycles = cycles
            .saturating_mul(u32::from(self.core.timer_prescaler) * 2 + 1)
            .saturating_add(1);
        let rx_detection_correction = match self.core.chip_type() {
            Pn53xType::Pn531 => 2 * 128,
            _ => 5 * 128,
        };
        cycles = cycles.saturating_sub(rx_detection_correction);
        if even_parity_bit(last_cmd_byte) == 1 {
            cycles = cycles.saturating_add(64);
        }
        Ok(cycles.saturating_add(self.profile.timer_correction))
    }

    fn timed_send_fifo(&mut self, tx: &[u8], tx_last_bits: u8) -> Result<(), Error> {
        let mut writes = Vec::with_capacity((tx.len() + 3) * 2);
        writes.push((
            PN53X_REG_CIU_COMMAND,
            SYMBOL_COMMAND & SYMBOL_COMMAND_TRANSCEIVE,
        ));
        writes.push((PN53X_REG_CIU_FIFO_LEVEL, SYMBOL_FLUSH_BUFFER));
        for byte in tx {
            writes.push((PN53X_REG_CIU_FIFO_DATA, *byte));
        }
        writes.push((
            PN53X_REG_CIU_BIT_FRAMING,
            SYMBOL_START_SEND | (tx_last_bits & SYMBOL_TX_LAST_BITS),
        ));
        self.write_registers(&writes)?;
        self.core.tx_bits = tx_last_bits & SYMBOL_TX_LAST_BITS;
        Ok(())
    }

    fn timed_wait_fifo_level(&mut self) -> Result<u8, Error> {
        let attempts = usize::from(3u16.saturating_mul(self.core.timer_prescaler * 2 + 1)).max(1);
        let mut level = 0u8;
        for _ in 0..attempts {
            level = self.read_register(PN53X_REG_CIU_FIFO_LEVEL)?;
            if level & SYMBOL_FIFO_LEVEL != 0 {
                return Ok(level);
            }
        }
        Ok(level)
    }

    fn timed_receive_fifo(
        &mut self,
        rx: &mut [u8],
        read_last_bits: bool,
    ) -> Result<(usize, u8), Error> {
        let mut fifo_level = self.timed_wait_fifo_level()?;
        let mut total = 0usize;
        while fifo_level & SYMBOL_FIFO_LEVEL != 0 {
            let chunk_len = usize::from(fifo_level & SYMBOL_FIFO_LEVEL);
            let mut registers = vec![PN53X_REG_CIU_FIFO_DATA; chunk_len];
            registers.push(PN53X_REG_CIU_FIFO_LEVEL);
            let values = self.read_registers(&registers)?;
            if total + chunk_len > rx.len() {
                return Err(status_error("transceive_timed", NFC_EOVFLOW));
            }
            rx[total..total + chunk_len].copy_from_slice(&values[..chunk_len]);
            total += chunk_len;
            fifo_level = values[chunk_len];
        }
        let last_bits = if read_last_bits && total != 0 {
            self.rx_last_bits()?
        } else {
            0
        };
        Ok((total, last_bits))
    }

    fn transceive_bytes_timed_shared(
        &mut self,
        operation: &'static str,
        tx: &[u8],
        rx: &mut [u8],
        max_cycles: TimerCycles,
    ) -> Result<(usize, TimerCycles), Error> {
        if !self.core.properties.handle_parity {
            return self.remember(Err(status_error(operation, NFC_EINVARG)));
        }
        if self.core.properties.easy_framing {
            return self.remember(Err(Error::UnsupportedOperation(operation)));
        }
        if tx.is_empty() {
            return self.remember(Err(status_error(operation, NFC_EINVARG)));
        }

        let txmode = if self.core.properties.handle_crc {
            Some(self.read_register(PN53X_REG_CIU_TX_MODE)?)
        } else {
            None
        };
        self.init_timer(max_cycles.get())?;
        self.timed_send_fifo(tx, 0)?;
        let (written, _) = self.timed_receive_fifo(rx, false)?;
        let last_cmd_byte = timer_last_command_byte(tx, txmode)?;
        let cycles = self.timer_cycles(last_cmd_byte)?;
        self.last_error = 0;
        Ok((written, TimerCycles::new(cycles)))
    }

    fn transceive_bits_timed_shared(
        &mut self,
        request: TimedBitTransceiveRequest<'_, '_, '_, '_>,
    ) -> Result<(usize, TimerCycles), Error> {
        let TimedBitTransceiveRequest {
            operation,
            tx,
            tx_bits_len,
            tx_parity,
            rx,
            rx_parity,
            max_cycles,
        } = request;
        if self.core.properties.easy_framing {
            return self.remember(Err(Error::UnsupportedOperation(operation)));
        }
        if self.core.properties.handle_crc {
            return self.remember(Err(Error::UnsupportedOperation(operation)));
        }

        let (payload, payload_bits_len) = if self.core.properties.handle_parity {
            if tx_parity.is_some() || rx_parity.is_some() {
                return self.remember(Err(Error::UnsupportedOperation(operation)));
            }
            let byte_len = bits_to_bytes_len(tx_bits_len);
            if tx.len() < byte_len {
                return self.remember(Err(status_error(operation, NFC_EINVARG)));
            }
            (tx[..byte_len].to_vec(), tx_bits_len)
        } else if tx_bits_len == 0 {
            (Vec::new(), 0)
        } else {
            (
                pn53x_wrap_frame(tx, tx_bits_len, tx_parity)?,
                tx_bits_len + (tx_bits_len / 8),
            )
        };

        self.init_timer(max_cycles.get())?;
        self.timed_send_fifo(&payload, (payload_bits_len % 8) as u8)?;
        let mut raw_rx = vec![0u8; rx.len().max(1)];
        let (raw_len, last_bits) = self.timed_receive_fifo(&mut raw_rx, true)?;
        let response_bits_len = raw_frame_bits_len(raw_len, last_bits);
        let written = if self.core.properties.handle_parity {
            let byte_len = bits_to_bytes_len(response_bits_len);
            Self::copy_into(operation, &raw_rx[..byte_len], rx)?;
            response_bits_len
        } else {
            pn53x_unwrap_frame(&raw_rx[..raw_len], response_bits_len, rx, rx_parity)?
        };
        let last_cmd_byte = payload.last().copied().unwrap_or(0);
        let cycles = self.timer_cycles(last_cmd_byte)?;
        self.last_error = 0;
        Ok((written, TimerCycles::new(cycles)))
    }

    fn transceive_bits_shared(
        &mut self,
        request: BitTransceiveRequest<'_, '_, '_>,
    ) -> Result<usize, Error> {
        let BitTransceiveRequest {
            operation,
            command,
            tx,
            tx_bits_len,
            tx_parity,
            rx,
            rx_parity,
            timeout_ms,
        } = request;
        let (payload, payload_bits_len) = if self.core.properties.handle_parity {
            if tx_parity.is_some() || rx_parity.is_some() {
                return self.remember(Err(Error::UnsupportedOperation(operation)));
            }
            let byte_len = bits_to_bytes_len(tx_bits_len);
            if tx.len() < byte_len {
                return self.remember(Err(status_error(operation, NFC_EINVARG)));
            }
            (tx[..byte_len].to_vec(), tx_bits_len)
        } else if tx_bits_len == 0 {
            (Vec::new(), 0)
        } else {
            (
                pn53x_wrap_frame(tx, tx_bits_len, tx_parity)?,
                tx_bits_len + (tx_bits_len / 8),
            )
        };

        self.set_tx_bits((payload_bits_len % 8) as u8)?;
        let response = self.exchange_with_status(operation, command, &payload, timeout_ms)?;
        let response_bits_len = raw_frame_bits_len(response.len(), self.rx_last_bits()?);
        let result_bits = if self.core.properties.handle_parity {
            let byte_len = bits_to_bytes_len(response_bits_len);
            Self::copy_into(operation, &response[..byte_len], rx)?;
            response_bits_len
        } else {
            pn53x_unwrap_frame(&response, response_bits_len, rx, rx_parity)?
        };
        self.last_error = 0;
        Ok(result_bits)
    }

    fn with_temporary_bool_property<R>(
        &mut self,
        property: Property,
        value: bool,
        f: impl FnOnce(&mut Self) -> Result<R, Error>,
    ) -> Result<R, Error> {
        let previous = self.core.property_bool_state(property).unwrap_or(false);
        self.apply_property_bool(property, value)?;
        let result = f(self);
        let restore = self.apply_property_bool(property, previous);
        match (result, restore) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Err(operation), Err(recovery)) => Err(Error::RecoveryFailed {
                operation: Box::new(operation),
                recovery: Box::new(recovery),
            }),
        }
    }

    fn select_passive_target_with_timeout(
        &mut self,
        modulation: Modulation,
        init_data: &[u8],
        timeout_ms: i32,
    ) -> Result<Option<Target>, Error> {
        let capabilities = self
            .core
            .capabilities()
            .expect("probe establishes chip capabilities");
        if !capabilities.supports(modulation, Mode::Initiator) {
            return Err(Error::MissingCapability("initiator modulation"));
        }
        match modulation.modulation_type() {
            ModulationType::Iso14443Bi
            | ModulationType::Iso14443B2Sr
            | ModulationType::Iso14443B2Ct
            | ModulationType::Iso14443BiClass => {
                return self.select_specialized_iso14443b(modulation, init_data, timeout_ms);
            }
            ModulationType::Barcode => {
                return self.select_barcode(modulation, timeout_ms);
            }
            _ => {}
        }
        let Some(passive_modulation) = nm_to_pm(modulation) else {
            return Err(Error::UnsupportedOperation("select_passive_target"));
        };
        let mut payload = Vec::with_capacity(init_data.len() + 2);
        payload.push(0x01);
        payload.push(passive_modulation);
        payload.extend_from_slice(init_data);

        let response = self.exchange_raw(PN53X_IN_LIST_PASSIVE_TARGET, &payload, timeout_ms)?;
        let target = if response.first().copied().unwrap_or(0) == 0 {
            None
        } else {
            let target = decode_target_data(
                self.core.chip_type(),
                modulation,
                response
                    .get(1..)
                    .ok_or(Error::InvalidEncoding("InListPassiveTarget response"))?,
            )?;
            if modulation.modulation_type() == ModulationType::Iso14443A
                && modulation.baud_rate() != BaudRate::Br106
            {
                let speed = match modulation.baud_rate() {
                    BaudRate::Br106 => 0x00,
                    BaudRate::Br212 => 0x01,
                    BaudRate::Br424 => 0x02,
                    BaudRate::Br847 => 0x03,
                };
                let _ = self.exchange_with_status(
                    "select_passive_target_psl",
                    PN53X_IN_PSL,
                    &[0x01, speed, speed],
                    0,
                )?;
            }
            Some(target)
        };
        if let Some(target) = &target {
            self.core.remember_target(target.clone());
        } else {
            self.core.clear_target();
        }
        Ok(target)
    }

    fn transceive_raw_bytes(&mut self, tx: &[u8], timeout_ms: i32) -> Result<Vec<u8>, Error> {
        let mut response = vec![0u8; PN53X_EXTENDED_FRAME_DATA_MAX_LEN];
        let timeout = OperationTimeout::try_milliseconds(
            u32::try_from(timeout_ms).map_err(|_| Error::InvalidArgument("poll timeout"))?,
        )?;
        let length = self.transceive_bytes_driver(tx, &mut response, timeout)?;
        response.truncate(length);
        Ok(response)
    }

    fn configure_iclass(&mut self) -> Result<(), Error> {
        self.write_registers(&[
            (PN53X_REG_CIU_TX_MODE, 0x03),
            (PN53X_REG_CIU_RX_MODE, 0x0b),
            (PN53X_REG_CIU_MANUAL_RCV, 0x10),
            (PN53X_REG_CIU_RF_CFG, 0x70),
            (PN53X_REG_CIU_GS_N_OFF, 0x88),
            (PN53X_REG_CIU_GS_N_ON, 0xf8),
            (PN53X_REG_CIU_CW_GS_P, 0x3f),
            (PN53X_REG_CIU_MOD_GS_P, 0x10),
            (PN53X_REG_CIU_TRELOAD_VAL_HI, 0x69),
            (PN53X_REG_CIU_TRELOAD_VAL_LO, 0xf0),
        ])
    }

    fn select_specialized_iso14443b(
        &mut self,
        modulation: Modulation,
        init_data: &[u8],
        timeout_ms: i32,
    ) -> Result<Option<Target>, Error> {
        if self.core.chip_type() == Pn53xType::Rcs360 {
            return Err(Error::UnsupportedOperation(
                "RCS360 raw ISO14443B discovery",
            ));
        }
        self.apply_property_bool(Property::ForceIso14443B, true)?;
        self.apply_property_bool(Property::ForceSpeed106, true)?;
        self.apply_property_bool(Property::HandleCrc, true)?;
        self.apply_property_bool(Property::EasyFraming, false)?;

        loop {
            let attempt = (|| -> Result<Vec<u8>, Error> {
                match modulation.modulation_type() {
                    ModulationType::Iso14443Bi => {
                        let target_data = self.transceive_raw_bytes(init_data, timeout_ms)?;
                        if target_data.len() < 6 {
                            return Err(Error::InvalidEncoding("ISO14443BI discovery"));
                        }
                        let mut attrib = target_data[..6].to_vec();
                        attrib[1] = 0x0f;
                        let _ = self.transceive_raw_bytes(&attrib, timeout_ms)?;
                        Ok(target_data)
                    }
                    ModulationType::Iso14443B2Sr => {
                        let chip_id = self.transceive_raw_bytes(&[0x06, 0x00], timeout_ms)?;
                        let chip_id = *chip_id
                            .first()
                            .ok_or(Error::InvalidEncoding("ISO14443B2SR chip id"))?;
                        let _ = self.transceive_raw_bytes(&[0x0e, chip_id], timeout_ms)?;
                        self.transceive_raw_bytes(&[0x0b], timeout_ms)
                    }
                    ModulationType::Iso14443B2Ct => {
                        let product = self.transceive_raw_bytes(&[0x10], timeout_ms)?;
                        if product.len() < 2 {
                            return Err(Error::InvalidEncoding("ISO14443B2CT product data"));
                        }
                        let uid_lsb = self.transceive_raw_bytes(&[0x9f, 0xff, 0xff], timeout_ms)?;
                        if uid_lsb.len() != 2 {
                            return Err(Error::InvalidEncoding("ISO14443B2CT UID LSB"));
                        }
                        let uid_msb = self.transceive_raw_bytes(&[0xc4], timeout_ms)?;
                        if uid_msb.len() < 2 {
                            return Err(Error::InvalidEncoding("ISO14443B2CT UID MSB"));
                        }
                        Ok(vec![
                            uid_lsb[0], uid_lsb[1], product[0], product[1], uid_msb[0], uid_msb[1],
                        ])
                    }
                    ModulationType::Iso14443BiClass => {
                        self.configure_iclass()?;
                        match self.transceive_raw_bytes(&[0x0a], timeout_ms) {
                            Ok(_) | Err(Error::RfTransmission(_)) => {}
                            Err(error) => return Err(error),
                        }
                        let anticol = self.transceive_raw_bytes(&[0x0c], timeout_ms)?;
                        if anticol.len() < 8 {
                            return Err(Error::InvalidEncoding("iClass anticollision"));
                        }
                        let mut select = Vec::with_capacity(9);
                        select.push(0x81);
                        select.extend_from_slice(&anticol[..8]);
                        let uid = self.transceive_raw_bytes(&select, timeout_ms)?;
                        if uid.len() < 8 {
                            return Err(Error::InvalidEncoding("iClass UID"));
                        }
                        Ok(uid[..8].to_vec())
                    }
                    _ => unreachable!("caller restricts specialized ISO14443B variants"),
                }
            })();

            match attempt {
                Ok(raw) => {
                    let target = decode_target_data(self.core.chip_type(), modulation, &raw)?;
                    self.core.remember_target(target.clone());
                    return Ok(Some(target));
                }
                Err(Error::RfTransmission(_)) if self.core.last_status_byte == 0x01 => {
                    if !self.core.properties.infinite_select {
                        self.core.clear_target();
                        return Ok(None);
                    }
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn select_barcode(
        &mut self,
        modulation: Modulation,
        _timeout_ms: i32,
    ) -> Result<Option<Target>, Error> {
        if self.core.chip_type() == Pn53xType::Rcs360 {
            return Err(Error::UnsupportedOperation("RCS360 NFC Barcode discovery"));
        }
        self.apply_property_bool(Property::ActivateField, false)?;
        let result = self.with_temporary_bool_property(Property::HandleCrc, false, |device| {
            device.with_temporary_bool_property(Property::HandleParity, false, |device| {
                loop {
                    let mut received = [0u8; PN53X_EXTENDED_FRAME_DATA_MAX_LEN];
                    let mut parity = [0u8; PN53X_EXTENDED_FRAME_DATA_MAX_LEN];
                    let empty = BitFrame::try_new(&[], 0, None)?;
                    let bits = match device.transceive_bits_driver(
                        empty,
                        &mut received,
                        Some(&mut parity),
                    ) {
                        Ok(bits) => bits,
                        Err(Error::RfTransmission(_) | Error::Chip(_)) => {
                            if device.core.properties.infinite_select {
                                continue;
                            }
                            return Ok(None);
                        }
                        Err(error) => return Err(error),
                    };

                    let mut barcode = [0u8; PN53X_EXTENDED_FRAME_DATA_MAX_LEN];
                    let mut output_bits = 0usize;
                    let push_bit = |output: &mut [u8], offset: &mut usize, bit: u8| {
                        output[*offset / 8] |= (bit & 1) << (7 - (*offset % 8));
                        *offset += 1;
                    };
                    push_bit(&mut barcode, &mut output_bits, 1);
                    for position in 0..(bits / 8) {
                        for bit in 0..8 {
                            push_bit(
                                &mut barcode,
                                &mut output_bits,
                                (received[position] >> bit) & 1,
                            );
                        }
                        push_bit(&mut barcode, &mut output_bits, parity[position]);
                    }
                    let position = bits / 8;
                    for bit in 0..(bits % 8) {
                        push_bit(
                            &mut barcode,
                            &mut output_bits,
                            (received[position] >> bit) & 1,
                        );
                    }
                    if !output_bits.is_multiple_of(128) {
                        if device.core.properties.infinite_select {
                            continue;
                        }
                        return Ok(None);
                    }
                    let length = output_bits / 8;
                    if length < 2 {
                        return Err(Error::InvalidEncoding("NFC Barcode length"));
                    }
                    let crc = iso14443a_crc_append(&barcode[..length - 2]);
                    if crc[1] != barcode[length - 2] || crc[0] != barcode[length - 1] {
                        if device.core.properties.infinite_select {
                            continue;
                        }
                        return Ok(None);
                    }
                    let target = decode_target_data(
                        device.core.chip_type(),
                        modulation,
                        &barcode[..length],
                    )?;
                    device.core.remember_target(target.clone());
                    return Ok(Some(target));
                }
            })
        });
        if result.as_ref().is_ok_and(Option::is_none) {
            self.core.clear_target();
        }
        result
    }

    fn poll_target_pn532(
        &mut self,
        modulations: &[Modulation],
        iterations: PollIterations,
        period: PollPeriod,
    ) -> Result<Option<Target>, Error> {
        let mut target_types = Vec::with_capacity(modulations.len() + 1);
        for modulation in modulations {
            if !self
                .core
                .capabilities()
                .expect("probe establishes chip capabilities")
                .supports(*modulation, Mode::Initiator)
            {
                return Err(Error::MissingCapability("initiator modulation"));
            }
            let target_type =
                nm_to_ptt(*modulation).ok_or(Error::InvalidArgument("AutoPoll modulation"))?;
            if self.core.properties.auto_iso14443_4 && target_type == 0x10 {
                target_types.push(0x20);
            }
            target_types.push(target_type);
        }
        if target_types.len() > 15 {
            return Err(Error::InvalidArgument("AutoPoll target types"));
        }

        let mut payload = Vec::with_capacity(target_types.len() + 2);
        payload.push(iterations.to_libnfc());
        payload.push(period.get());
        payload.extend_from_slice(&target_types);
        let response = self.exchange_raw(PN532_IN_AUTO_POLL, &payload, 0)?;
        let Some((&count, mut encoded_targets)) = response.split_first() else {
            return Err(Error::InvalidEncoding("InAutoPoll response"));
        };
        if count == 0 {
            self.core.clear_target();
            return Ok(None);
        }
        if count > 2 {
            return Err(Error::Chip("InAutoPoll target count"));
        }

        let mut selected = None;
        for _ in 0..count {
            let (&target_type, tail) = encoded_targets
                .split_first()
                .ok_or(Error::InvalidEncoding("InAutoPoll target type"))?;
            let (&target_len, target_tail) = tail
                .split_first()
                .ok_or(Error::InvalidEncoding("InAutoPoll target length"))?;
            let target_len = usize::from(target_len);
            let target_data = target_tail
                .get(..target_len)
                .ok_or(Error::InvalidEncoding("InAutoPoll target data"))?;
            encoded_targets = &target_tail[target_len..];
            let modulation = ptt_to_nm(target_type)?;
            selected = Some(decode_target_data(
                self.core.chip_type(),
                modulation,
                target_data,
            )?);
        }
        if let Some(target) = &selected {
            self.core.remember_target(target.clone());
        }
        Ok(selected)
    }

    fn presence_transceive_bytes(
        &mut self,
        tx: &[u8],
        timeout_ms: i32,
        easy_framing: bool,
        attempts: usize,
    ) -> Result<bool, Error> {
        self.with_temporary_bool_property(Property::EasyFraming, easy_framing, |device| {
            let timeout = OperationTimeout::from_libnfc_millis(timeout_ms)?;
            for _ in 0..attempts {
                let mut rx = [0u8; PN53X_EXTENDED_FRAME_DATA_MAX_LEN];
                match device.transceive_bytes_driver(tx, &mut rx, timeout) {
                    Ok(len) if len > 0 => return Ok(true),
                    Ok(_) => {}
                    Err(Error::RfTransmission(_))
                        if device.core.last_status_byte == PN53X_STATUS_TIMEOUT =>
                    {
                        return Err(status_error("target_is_present", NFC_ETGRELEASED));
                    }
                    Err(Error::RfTransmission(_) | Error::Chip(_)) => {}
                    Err(error) => return Err(error),
                }
            }
            Err(status_error("target_is_present", NFC_ETGRELEASED))
        })
    }

    fn presence_transceive_bits(&mut self, attempts: usize) -> Result<bool, Error> {
        self.apply_property_bool(Property::ActivateField, false)?;
        self.with_temporary_bool_property(Property::HandleCrc, false, |device| {
            device.with_temporary_bool_property(Property::HandleParity, false, |device| {
                for _ in 0..attempts {
                    device.apply_property_bool(Property::ActivateField, false)?;
                    let mut rx = [0u8; PN53X_EXTENDED_FRAME_DATA_MAX_LEN];
                    let mut parity = [0u8; PN53X_EXTENDED_FRAME_DATA_MAX_LEN];
                    let empty = BitFrame::try_new(&[], 0, None)?;
                    match device.transceive_bits_driver(empty, &mut rx, Some(&mut parity)) {
                        Ok(len) if len > 0 => return Ok(true),
                        Ok(_) | Err(Error::RfTransmission(_) | Error::Chip(_)) => {}
                        Err(error) => return Err(error),
                    }
                }
                Err(status_error("target_is_present", NFC_ETGRELEASED))
            })
        })
    }

    fn check_iclass_presence(&mut self) -> Result<bool, Error> {
        self.configure_iclass()?;
        self.with_temporary_bool_property(Property::EasyFraming, false, |device| {
            let timeout = OperationTimeout::try_milliseconds(300)?;
            let mut rx = [0u8; PN53X_EXTENDED_FRAME_DATA_MAX_LEN];
            match device.transceive_bytes_driver(&[0x0a], &mut rx, timeout) {
                Ok(_) => {}
                Err(Error::RfTransmission(_))
                    if device.core.last_status_byte == PN53X_STATUS_TIMEOUT => {}
                Err(error) => return Err(error),
            }

            for _ in 0..2 {
                let mut rx = [0u8; PN53X_EXTENDED_FRAME_DATA_MAX_LEN];
                match device.transceive_bytes_driver(&[0x0c], &mut rx, timeout) {
                    Ok(len) if len > 0 => return Ok(true),
                    Ok(_) | Err(Error::RfTransmission(_) | Error::Chip(_)) => {}
                    Err(error) => return Err(error),
                }
            }
            Err(status_error("target_is_present", NFC_ETGRELEASED))
        })
    }

    fn diagnose_card_presence(&mut self) -> Result<bool, Error> {
        const PN53X_DIAGNOSE: u8 = 0x00;
        let response = self.exchange_raw(PN53X_DIAGNOSE, &[0x06], 1000)?;
        let Some(&status) = response.first() else {
            return Err(status_error("target_is_present", NFC_EIO));
        };
        self.core.last_status_byte = status & 0x3f;
        let mapped = pn53x_translate_status(self.core.last_status_byte);
        if mapped < 0 {
            return Err(status_error("target_is_present", mapped));
        }
        Ok(true)
    }

    fn check_iso14443a_presence(&mut self, target: &Target) -> Result<bool, Error> {
        match target.info() {
            TargetInfo::Iso14443A { atqa, sak, uid, .. } if sak & SAK_ISO14443_4_COMPLIANT != 0 => {
                self.presence_transceive_bytes(&[0xb2], 300, false, 2)
            }
            TargetInfo::Iso14443A { atqa, sak, .. } if *sak == 0x00 && *atqa == [0x00, 0x44] => {
                if self.core.chip_type() == Pn53xType::Pn533 {
                    self.diagnose_card_presence()
                } else {
                    self.presence_transceive_bytes(&[0x30, 0x00], 300, true, 2)
                }
            }
            TargetInfo::Iso14443A { sak, uid, .. } if *sak & SAK_MIFARE_CLASSIC_MASK != 0 => {
                let init_data = cascade_iso14443a_uid(uid);
                self.with_temporary_bool_property(Property::InfiniteSelect, false, |device| {
                    device
                        .select_passive_target_driver(target.modulation(), &init_data)
                        .map(|found| found.is_some())
                })
            }
            _ => Err(status_error("target_is_present", NFC_EDEVNOTSUPP)),
        }
    }

    fn check_current_target_presence(&mut self, target: &Target) -> Result<bool, Error> {
        match target.modulation().modulation_type() {
            ModulationType::Iso14443A => self.check_iso14443a_presence(target),
            ModulationType::Iso14443B => {
                self.presence_transceive_bytes(&[0xba, 0x01], 300, false, 2)
            }
            ModulationType::Iso14443Bi => match target.info() {
                TargetInfo::Iso14443Bi { div, .. } => {
                    let mut command = vec![0x01, 0x0f];
                    command.extend_from_slice(div);
                    self.presence_transceive_bytes(&command, 300, false, 2)
                }
                _ => Err(status_error("target_is_present", NFC_EDEVNOTSUPP)),
            },
            ModulationType::Iso14443B2Sr => self.presence_transceive_bytes(&[0x0b], 300, false, 2),
            ModulationType::Iso14443B2Ct => match target.info() {
                TargetInfo::Iso14443B2Ct { uid, .. } => {
                    self.presence_transceive_bytes(&[0x9f, uid[0], uid[1]], 300, false, 2)
                }
                _ => Err(status_error("target_is_present", NFC_EDEVNOTSUPP)),
            },
            ModulationType::Iso14443BiClass => self.check_iclass_presence(),
            ModulationType::Jewel => self.presence_transceive_bytes(&[0x78], -1, true, 2),
            ModulationType::Felica => match target.info() {
                TargetInfo::Felica { id, .. } => {
                    let mut command = vec![0x0a, 0x04];
                    command.extend_from_slice(id);
                    self.presence_transceive_bytes(&command, 300, true, 3)
                }
                _ => Err(status_error("target_is_present", NFC_EDEVNOTSUPP)),
            },
            ModulationType::Dep => self.diagnose_card_presence(),
            ModulationType::Barcode => self.presence_transceive_bits(3),
        }
    }

    fn enter_low_vbat(&mut self) -> Result<(), Error> {
        if !matches!(self.core.chip_type(), Pn53xType::Pn531 | Pn53xType::Pn532) {
            return Err(Error::MissingCapability("PN53x PowerDown"));
        }
        let _ = self.exchange_raw(PN53X_POWER_DOWN, &[0xf0], self.core.timeout_command_ms)?;
        self.core.power_mode = Pn53xPowerMode::LowVbat;
        Ok(())
    }
}

impl<T: Pn53xTransport + Send + 'static> DeviceMeta for Pn53xDevice<T> {
    fn name(&self) -> &str {
        &self.name
    }

    fn connstring(&self) -> &ConnectionString {
        &self.connstring
    }

    fn last_error(&self) -> i32 {
        self.last_error
    }
}

impl<T: Pn53xTransport + Send + 'static> InfoBackend for Pn53xDevice<T> {
    fn information_about(&mut self) -> Result<String, Error> {
        let message = format!("{} via {}", self.firmware_text(), self.connstring);
        self.last_error = 0;
        Ok(message)
    }
}

impl<T: Pn53xTransport + Send + 'static> PropertyBackend for Pn53xDevice<T> {
    fn set_property_bool(&mut self, property: Property, enable: bool) -> Result<(), Error> {
        let result = self.apply_property_bool(property, enable);
        self.remember(result)
    }

    fn set_property_int(&mut self, property: Property, value: i32) -> Result<(), Error> {
        let result = self.apply_property_int(property, value);
        self.remember(result)
    }

    fn supported_modulations(&mut self, mode: Mode) -> Result<Vec<ModulationType>, Error> {
        self.last_error = 0;
        if mode == Mode::Target && self.profile.usb_model == Some(Pn53xUsbModel::AskLogo) {
            return Ok(Vec::new());
        }
        Ok(self
            .core
            .capabilities()
            .expect("probe establishes chip capabilities")
            .supported_modulations(mode))
    }

    fn supported_baud_rates(
        &mut self,
        mode: Mode,
        modulation_type: ModulationType,
    ) -> Result<Vec<BaudRate>, Error> {
        self.last_error = 0;
        Ok(self
            .core
            .capabilities()
            .expect("probe establishes chip capabilities")
            .supported_baud_rates(mode, modulation_type))
    }

    fn property_bool_state(&self, property: Property) -> Option<bool> {
        self.core.property_bool_state(property)
    }
}

impl<T: Pn53xTransport + Send + 'static> InitiatorBackend for Pn53xDevice<T> {
    fn initiator_init_driver(&mut self) -> Result<i32, Error> {
        self.reset_settings()?;
        self.update_register_bits(PN53X_REG_CIU_CONTROL, SYMBOL_INITIATOR, SYMBOL_INITIATOR)?;
        self.core.operating_mode = Pn53xOperatingMode::Initiator;
        self.core.clear_target();
        self.last_error = 0;
        Ok(0)
    }

    fn initiator_init_secure_element_driver(&mut self) -> Result<i32, Error> {
        let Some(mode) = self.profile.secure_element_mode else {
            return Err(Error::UnsupportedOperation("initiator_init_secure_element"));
        };
        self.sam_configuration(mode, self.core.timeout_command_ms)
    }

    fn select_passive_target_driver(
        &mut self,
        nm: Modulation,
        init_data: &[u8],
    ) -> Result<Option<Target>, Error> {
        let result = self.select_passive_target_with_timeout(nm, init_data, 300);
        self.remember(result)
    }

    fn poll_target_driver(
        &mut self,
        modulations: &[Modulation],
        iterations: PollIterations,
        period: PollPeriod,
    ) -> Result<Option<Target>, Error> {
        if modulations.is_empty() {
            return self.remember(Err(Error::InvalidArgument("modulations")));
        }

        if self.core.chip_type() == Pn53xType::Pn532 {
            let result = self.poll_target_pn532(modulations, iterations, period);
            return self.remember(result);
        }

        let timeout_ms = i32::from(period.get()) * 150;
        let result = self.with_temporary_bool_property(Property::InfiniteSelect, true, |device| {
            loop {
                for _ in 0..iterations.to_libnfc() {
                    for modulation in modulations {
                        if let Some(target) = device.select_passive_target_with_timeout(
                            *modulation,
                            default_initiator_payload(*modulation),
                            timeout_ms,
                        )? {
                            return Ok(Some(target));
                        }
                    }
                }
                if !iterations.is_continuous() {
                    return Ok(None);
                }
            }
        });
        self.remember(result)
    }

    fn select_dep_target_driver(
        &mut self,
        ndm: DepMode,
        nbr: BaudRate,
        initiator: Option<&DepInfo>,
        timeout: OperationTimeout,
    ) -> Result<Option<Target>, Error> {
        let payload = build_injump_for_dep_command(ndm, nbr, initiator)?;
        let timeout = Self::resolve_timeout(timeout, self.core.timeout_command_ms)?;
        let response = self.exchange_with_status(
            "select_dep_target",
            PN53X_IN_JUMP_FOR_DEP,
            &payload,
            timeout,
        )?;
        let target = parse_dep_target(&response, ndm, nbr)?;
        if let Some(target) = &target {
            self.core.remember_target(target.clone());
        } else {
            self.core.clear_target();
        }
        self.last_error = 0;
        Ok(target)
    }

    fn deselect_target_driver(&mut self) -> Result<(), Error> {
        let _ = self.exchange_with_status(
            "deselect_target",
            PN53X_IN_DESELECT,
            &[0x00],
            self.core.timeout_command_ms,
        )?;
        self.core.clear_target();
        self.last_error = 0;
        Ok(())
    }

    fn target_is_present_driver(&mut self, target: Option<&Target>) -> Result<bool, Error> {
        let Some(current) = self.core.current_target().cloned() else {
            return self.remember(Err(status_error("target_is_present", NFC_EINVARG)));
        };
        if target.is_some_and(|target| *target != current) {
            self.core.clear_target();
            return self.remember(Err(status_error("target_is_present", NFC_ETGRELEASED)));
        }
        match self.check_current_target_presence(&current) {
            Ok(found) => {
                if !found {
                    self.core.clear_target();
                }
                self.last_error = 0;
                Ok(found)
            }
            Err(error) => {
                let code = status_code(&error);
                if matches!(code, NFC_ETGRELEASED | NFC_ETIMEOUT) {
                    self.core.clear_target();
                }
                self.remember(Err(error))
            }
        }
    }

    fn transceive_bytes_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: OperationTimeout,
    ) -> Result<usize, Error> {
        let timeout = Self::resolve_timeout(timeout, self.core.timeout_communication_ms)?;
        self.set_tx_bits(0)?;
        let response = if self.core.properties.easy_framing {
            let mut payload = Vec::with_capacity(tx.len() + 1);
            payload.push(0x01);
            payload.extend_from_slice(tx);
            self.exchange_with_status(
                "transceive_bytes",
                PN53X_IN_DATA_EXCHANGE,
                &payload,
                timeout,
            )?
        } else {
            self.exchange_with_status("transceive_bytes", PN53X_IN_COMMUNICATE_THRU, tx, timeout)?
        };
        let written = Self::copy_into("transceive_bytes", &response, rx)?;
        self.last_error = 0;
        Ok(written)
    }

    fn transceive_bits_driver(
        &mut self,
        tx: BitFrame<'_>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        self.transceive_bits_shared(BitTransceiveRequest {
            operation: "transceive_bits",
            command: PN53X_IN_COMMUNICATE_THRU,
            tx: tx.bytes(),
            tx_bits_len: tx.bit_len(),
            tx_parity: tx.parity(),
            rx,
            rx_parity,
            timeout_ms: self.core.timeout_communication_ms,
        })
    }

    fn transceive_bytes_timed_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        max_cycles: TimerCycles,
    ) -> Result<(usize, TimerCycles), Error> {
        self.transceive_bytes_timed_shared("transceive_bytes_timed", tx, rx, max_cycles)
    }

    fn transceive_bits_timed_driver(
        &mut self,
        tx: BitFrame<'_>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
        max_cycles: TimerCycles,
    ) -> Result<(usize, TimerCycles), Error> {
        self.transceive_bits_timed_shared(TimedBitTransceiveRequest {
            operation: "transceive_bits_timed",
            tx: tx.bytes(),
            tx_bits_len: tx.bit_len(),
            tx_parity: tx.parity(),
            rx,
            rx_parity,
            max_cycles,
        })
    }

    fn command_abort_handle(&self) -> Option<proximate_driver::CommandAbortHandle> {
        self.transport.command_abort_handle()
    }

    fn abort_command_driver(&mut self) -> Result<(), Error> {
        let result = self.transport.abort_command();
        self.remember(result)
    }

    fn idle_driver(&mut self) -> Result<(), Error> {
        let mode = self.core.operating_mode;
        if mode == Pn53xOperatingMode::Idle {
            self.last_error = 0;
            return Ok(());
        }
        let _ = self.exchange_with_status(
            "idle_release",
            PN53X_IN_RELEASE,
            &[0x00],
            self.core.timeout_command_ms,
        )?;
        self.core.clear_target();
        if mode == Pn53xOperatingMode::Initiator {
            self.apply_property_bool(Property::ActivateField, false)?;
        }
        if self.core.chip_type() == Pn53xType::Pn532 {
            self.enter_low_vbat()?;
        }
        self.core.operating_mode = Pn53xOperatingMode::Idle;
        self.last_error = 0;
        Ok(())
    }

    fn powerdown_driver(&mut self) -> Result<(), Error> {
        let result = self.enter_low_vbat();
        self.remember(result)
    }
}

impl<T: Pn53xTransport + Send + 'static> TargetBackend for Pn53xDevice<T> {
    fn target_init_driver(
        &mut self,
        target: &mut Target,
        rx: &mut [u8],
        timeout: OperationTimeout,
    ) -> Result<usize, Error> {
        self.reset_settings()?;
        match target.modulation().modulation_type() {
            ModulationType::Iso14443A => self.set_parameters(PARAM_AUTO_ATR_RES, false)?,
            ModulationType::Dep => self.set_parameters(PARAM_AUTO_ATR_RES, true)?,
            ModulationType::Felica => {}
            _ => return Err(Error::MissingCapability("PN53x target modulation")),
        }
        self.update_register_bits(
            PN53X_REG_CIU_TX_AUTO,
            SYMBOL_INITIAL_RF_ON,
            SYMBOL_INITIAL_RF_ON,
        )?;
        let command =
            build_target_init_command(self.core.chip_type(), self.core.properties, target)?;
        let timeout = Self::resolve_timeout(timeout, self.core.timeout_command_ms)?;
        let response = self.exchange_raw(PN53X_TG_INIT_AS_TARGET, &command[1..], timeout)?;
        let Some((&activation_mode, payload)) = response.split_first() else {
            return self.remember(Err(status_error("target_init", NFC_EIO)));
        };
        let (modulation, dep_mode) = decode_activation_mode(activation_mode)?;
        target.apply_activation(modulation, dep_mode)?;
        let written = Self::copy_into("target_init", payload, rx)?;
        self.core.operating_mode = Pn53xOperatingMode::Target;
        self.core.remember_target(target.clone());
        self.last_error = 0;
        Ok(written)
    }

    fn target_send_bytes_driver(
        &mut self,
        tx: &[u8],
        timeout: OperationTimeout,
    ) -> Result<usize, Error> {
        let timeout = Self::resolve_timeout(timeout, self.core.timeout_communication_ms)?;
        self.set_tx_bits(0)?;
        let command = match self.core.current_target() {
            Some(target) if self.core.properties.easy_framing => {
                match target.modulation().modulation_type() {
                    ModulationType::Dep => PN53X_TG_SET_DATA,
                    ModulationType::Iso14443A
                        if self.core.chip_type() == Pn53xType::Pn532
                            && self.core.properties.auto_iso14443_4
                            && is_iso14443_4_target(target) =>
                    {
                        PN53X_TG_SET_DATA
                    }
                    _ => PN53X_TG_RESPONSE_TO_INITIATOR,
                }
            }
            _ => PN53X_TG_RESPONSE_TO_INITIATOR,
        };
        let _ = self.exchange_with_status("target_send_bytes", command, tx, timeout)?;
        self.last_error = 0;
        Ok(tx.len())
    }

    fn target_receive_bytes_driver(
        &mut self,
        rx: &mut [u8],
        timeout: OperationTimeout,
    ) -> Result<usize, Error> {
        let timeout = Self::resolve_timeout(timeout, self.core.timeout_communication_ms)?;
        let command = match self.core.current_target() {
            Some(target) if self.core.properties.easy_framing => {
                match target.modulation().modulation_type() {
                    ModulationType::Dep => PN53X_TG_GET_DATA,
                    ModulationType::Iso14443A
                        if self.core.chip_type() == Pn53xType::Pn532
                            && self.core.properties.auto_iso14443_4
                            && is_iso14443_4_target(target) =>
                    {
                        PN53X_TG_GET_DATA
                    }
                    _ => PN53X_TG_GET_INITIATOR_COMMAND,
                }
            }
            _ => PN53X_TG_GET_INITIATOR_COMMAND,
        };
        let response = self.exchange_with_status("target_receive_bytes", command, &[], timeout)?;
        let written = Self::copy_into("target_receive_bytes", &response, rx)?;
        self.last_error = 0;
        Ok(written)
    }

    fn target_send_bits_driver(&mut self, tx: BitFrame<'_>) -> Result<usize, Error> {
        let mut sink = [];
        let _ = self.transceive_bits_shared(BitTransceiveRequest {
            operation: "target_send_bits",
            command: PN53X_TG_RESPONSE_TO_INITIATOR,
            tx: tx.bytes(),
            tx_bits_len: tx.bit_len(),
            tx_parity: tx.parity(),
            rx: &mut sink,
            rx_parity: None,
            timeout_ms: self.core.timeout_communication_ms,
        })?;
        self.last_error = 0;
        Ok(tx.bit_len())
    }

    fn target_receive_bits_driver(
        &mut self,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        self.transceive_bits_shared(BitTransceiveRequest {
            operation: "target_receive_bits",
            command: PN53X_TG_GET_INITIATOR_COMMAND,
            tx: &[],
            tx_bits_len: 0,
            tx_parity: None,
            rx,
            rx_parity,
            timeout_ms: self.core.timeout_communication_ms,
        })
    }
}

impl<T: Pn53xTransport + Send + 'static> Pn53xBackend for Pn53xDevice<T> {
    fn pn53x_transceive_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: OperationTimeout,
    ) -> Result<usize, Error> {
        let Some((&command, payload)) = tx.split_first() else {
            return self.remember(Err(status_error("pn53x_transceive", NFC_EINVARG)));
        };
        let timeout = Self::resolve_timeout(timeout, self.core.timeout_command_ms)?;
        let response = self.exchange_raw(command, payload, timeout)?;
        let written = Self::copy_into("pn53x_transceive", &response, rx)?;
        self.last_error = 0;
        Ok(written)
    }

    fn pn53x_read_register_driver(&mut self, register: u16) -> Result<u8, Error> {
        let value = self.read_register(register)?;
        self.last_error = 0;
        Ok(value)
    }

    fn pn53x_write_register_driver(
        &mut self,
        register: u16,
        symbol_mask: u8,
        value: u8,
    ) -> Result<(), Error> {
        self.update_register_bits(register, symbol_mask, value)?;
        self.last_error = 0;
        Ok(())
    }

    fn pn532_sam_configuration_driver(
        &mut self,
        mode: u8,
        timeout: OperationTimeout,
    ) -> Result<i32, Error> {
        let mode = Pn532SamMode::from_raw(mode)
            .ok_or_else(|| status_error("pn532_SAMConfiguration", NFC_EINVARG))?;
        let timeout = Self::resolve_timeout(timeout, self.core.timeout_command_ms)?;
        self.sam_configuration(mode, timeout)
    }
}
