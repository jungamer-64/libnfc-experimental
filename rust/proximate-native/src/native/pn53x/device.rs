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
        Ok(Self {
            name: name.into(),
            connstring,
            profile,
            transport,
            core,
            last_error: 0,
        })
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

    fn set_tx_bits(&mut self, bits: u8) -> Result<(), Error> {
        let bits = bits & SYMBOL_TX_LAST_BITS;
        if self.core.tx_bits == bits {
            return Ok(());
        }
        self.update_register_bits(PN53X_REG_CIU_BIT_FRAMING, SYMBOL_TX_LAST_BITS, bits)?;
        self.core.tx_bits = bits;
        Ok(())
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
    ) -> Result<(usize, u32), Error> {
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
        self.init_timer(0)?;
        self.timed_send_fifo(tx, 0)?;
        let (written, _) = self.timed_receive_fifo(rx, false)?;
        let last_cmd_byte = timer_last_command_byte(tx, txmode)?;
        let cycles = self.timer_cycles(last_cmd_byte)?;
        self.last_error = 0;
        Ok((written, cycles))
    }

    fn transceive_bits_timed_shared(
        &mut self,
        operation: &'static str,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), Error> {
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

        self.init_timer(0)?;
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
        Ok((written, cycles))
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
        self.core.set_property_bool(property, value)?;
        let result = f(self);
        let restore = self.core.set_property_bool(property, previous);
        match (result, restore) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) | (Err(_), Err(error)) => Err(error),
        }
    }

    fn presence_transceive_bytes(
        &mut self,
        tx: &[u8],
        timeout_ms: i32,
        easy_framing: bool,
    ) -> Result<bool, Error> {
        self.with_temporary_bool_property(Property::EasyFraming, easy_framing, |device| {
            let mut rx = [0u8; PN53X_EXTENDED_FRAME_DATA_MAX_LEN];
            let len = device.transceive_bytes_driver(tx, &mut rx, timeout_ms)?;
            Ok(len > 0)
        })
    }

    fn presence_transceive_bits(&mut self, _timeout_ms: i32) -> Result<bool, Error> {
        self.with_temporary_bool_property(Property::HandleParity, false, |device| {
            let mut rx = [0u8; PN53X_EXTENDED_FRAME_DATA_MAX_LEN];
            let mut parity = [0u8; PN53X_EXTENDED_FRAME_DATA_MAX_LEN];
            let len = device.transceive_bits_driver(&[], 0, None, &mut rx, Some(&mut parity))?;
            Ok(len > 0)
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
        match &target.info {
            TargetInfo::Iso14443A { atqa, sak, uid, .. } if sak & SAK_ISO14443_4_COMPLIANT != 0 => {
                self.presence_transceive_bytes(&[0xb2], 300, false)
            }
            TargetInfo::Iso14443A { atqa, sak, .. } if *sak == 0x00 && *atqa == [0x00, 0x44] => {
                self.presence_transceive_bytes(&[0x30, 0x00], 300, true)
            }
            TargetInfo::Iso14443A { sak, uid, .. } if *sak & SAK_MIFARE_CLASSIC_MASK != 0 => {
                let init_data = cascade_iso14443a_uid(uid);
                self.with_temporary_bool_property(Property::InfiniteSelect, false, |device| {
                    device
                        .select_passive_target_driver(target.modulation, &init_data)
                        .map(|found| found.is_some())
                })
            }
            _ => Err(status_error("target_is_present", NFC_EDEVNOTSUPP)),
        }
    }

    fn check_current_target_presence(&mut self, target: &Target) -> Result<bool, Error> {
        match target.modulation.modulation_type {
            ModulationType::Iso14443A => self.check_iso14443a_presence(target),
            ModulationType::Iso14443B => self.presence_transceive_bytes(&[0xba, 0x01], 300, false),
            ModulationType::Jewel => self.presence_transceive_bytes(&[0x78], -1, true),
            ModulationType::Felica => match &target.info {
                TargetInfo::Felica { id, .. } => {
                    let mut command = vec![0x0a, 0x04];
                    command.extend_from_slice(id);
                    self.presence_transceive_bytes(&command, 300, true)
                }
                _ => Err(status_error("target_is_present", NFC_EDEVNOTSUPP)),
            },
            ModulationType::Dep => self.diagnose_card_presence(),
            ModulationType::Barcode => self.presence_transceive_bits(300),
            _ => Err(status_error("target_is_present", NFC_EDEVNOTSUPP)),
        }
    }
}

impl<T: Pn53xTransport + Send + 'static> DeviceMeta for Pn53xDevice<T> {
    fn name(&self) -> &str {
        &self.name
    }

    fn connstring(&self) -> &ConnectionString {
        &self.connstring
    }

    fn caps(&self) -> DeviceCaps {
        let mut caps = DeviceCaps::INFO
            | DeviceCaps::SET_PROPERTY_BOOL
            | DeviceCaps::SET_PROPERTY_INT
            | DeviceCaps::SUPPORTED_MODULATIONS
            | DeviceCaps::SUPPORTED_BAUD_RATES
            | DeviceCaps::INITIATOR_INIT
            | DeviceCaps::SELECT_PASSIVE_TARGET
            | DeviceCaps::POLL_TARGET
            | DeviceCaps::SELECT_DEP_TARGET
            | DeviceCaps::DESELECT_TARGET
            | DeviceCaps::TARGET_IS_PRESENT
            | DeviceCaps::TARGET_INIT
            | DeviceCaps::TRANSCEIVE_BYTES
            | DeviceCaps::TRANSCEIVE_BITS
            | DeviceCaps::TRANSCEIVE_BYTES_TIMED
            | DeviceCaps::TRANSCEIVE_BITS_TIMED
            | DeviceCaps::TARGET_SEND_BYTES
            | DeviceCaps::TARGET_RECEIVE_BYTES
            | DeviceCaps::TARGET_SEND_BITS
            | DeviceCaps::TARGET_RECEIVE_BITS
            | DeviceCaps::ABORT_COMMAND
            | DeviceCaps::IDLE
            | DeviceCaps::POWERDOWN
            | DeviceCaps::PN53X_TRANSCEIVE
            | DeviceCaps::PN53X_READ_REGISTER
            | DeviceCaps::PN53X_WRITE_REGISTER
            | DeviceCaps::PN532_SAM_CONFIGURATION;
        if self.profile.secure_element_mode.is_some() {
            caps |= DeviceCaps::INITIATOR_INIT_SECURE_ELEMENT;
        }
        caps
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
        let result = self.core.set_property_bool(property, enable);
        self.remember(result)
    }

    fn set_property_int(&mut self, property: Property, value: i32) -> Result<(), Error> {
        let result = self.core.set_property_int(property, value);
        self.remember(result)
    }

    fn supported_modulations(&mut self, mode: Mode) -> Result<Vec<ModulationType>, Error> {
        self.last_error = 0;
        Ok(self.profile.supported_modulations(mode))
    }

    fn supported_baud_rates(
        &mut self,
        mode: Mode,
        modulation_type: ModulationType,
    ) -> Result<Vec<BaudRate>, Error> {
        self.last_error = 0;
        Ok(match (mode, modulation_type) {
            (_, ModulationType::Iso14443A)
            | (_, ModulationType::Iso14443B)
            | (_, ModulationType::Jewel) => vec![BaudRate::Br106],
            (_, ModulationType::Felica) => vec![BaudRate::Br212, BaudRate::Br424],
            (_, ModulationType::Dep) => {
                vec![BaudRate::Br106, BaudRate::Br212, BaudRate::Br424]
            }
            _ => Vec::new(),
        })
    }

    fn property_bool_state(&self, property: Property) -> Option<bool> {
        self.core.property_bool_state(property)
    }
}

impl<T: Pn53xTransport + Send + 'static> InitiatorBackend for Pn53xDevice<T> {
    fn initiator_init_driver(&mut self) -> Result<i32, Error> {
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
        let Some(pm) = nm_to_pm(nm) else {
            return self.remember(Err(Error::UnsupportedOperation("select_passive_target")));
        };
        let mut payload = Vec::with_capacity(init_data.len() + 3);
        payload.push(0x01);
        payload.push(pm);
        payload.extend_from_slice(init_data);

        let response = self.exchange_raw(
            PN53X_IN_LIST_PASSIVE_TARGET,
            &payload,
            self.core.timeout_command_ms,
        )?;
        let target = if response.first().copied().unwrap_or(0) == 0 {
            None
        } else {
            Some(decode_target_data(
                self.core.chip_type(),
                nm,
                &response[1..],
            )?)
        };
        if let Some(target) = &target {
            self.core.remember_target(target.clone());
        } else {
            self.core.clear_target();
        }
        self.last_error = 0;
        Ok(target)
    }

    fn poll_target_driver(
        &mut self,
        modulations: &[Modulation],
        poll_nr: u8,
        period: u8,
    ) -> Result<Option<Target>, Error> {
        if modulations.is_empty() {
            return self.remember(Err(Error::InvalidArgument("modulations")));
        }

        let delay = Duration::from_micros(u64::from(period) * 150_000);
        let mut remaining = if poll_nr == 0xff {
            usize::MAX
        } else {
            usize::from(poll_nr.max(1))
        };

        while remaining > 0 {
            for modulation in modulations {
                if let Some(target) = self.select_passive_target_driver(
                    *modulation,
                    default_initiator_payload(*modulation),
                )? {
                    self.last_error = 0;
                    return Ok(Some(target));
                }
            }
            if poll_nr != 0xff {
                remaining -= 1;
            }
            thread::sleep(delay);
        }

        self.last_error = 0;
        Ok(None)
    }

    fn select_dep_target_driver(
        &mut self,
        ndm: DepMode,
        nbr: BaudRate,
        initiator: Option<&DepInfo>,
        timeout: i32,
    ) -> Result<Option<Target>, Error> {
        let payload = build_injump_for_dep_command(ndm, nbr, initiator)?;
        let timeout = if timeout >= 0 {
            timeout
        } else {
            self.core.timeout_command_ms
        };
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
        timeout: i32,
    ) -> Result<usize, Error> {
        let timeout = if timeout >= 0 {
            timeout
        } else {
            self.core.timeout_communication_ms
        };
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
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        self.transceive_bits_shared(BitTransceiveRequest {
            operation: "transceive_bits",
            command: PN53X_IN_COMMUNICATE_THRU,
            tx,
            tx_bits_len,
            tx_parity,
            rx,
            rx_parity,
            timeout_ms: self.core.timeout_communication_ms,
        })
    }

    fn transceive_bytes_timed_driver(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
    ) -> Result<(usize, u32), Error> {
        self.transceive_bytes_timed_shared("transceive_bytes_timed", tx, rx)
    }

    fn transceive_bits_timed_driver(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), Error> {
        self.transceive_bits_timed_shared(
            "transceive_bits_timed",
            tx,
            tx_bits_len,
            tx_parity,
            rx,
            rx_parity,
        )
    }

    fn abort_command_driver(&mut self) -> Result<(), Error> {
        let result = self.transport.abort_command();
        self.remember(result)
    }

    fn idle_driver(&mut self) -> Result<(), Error> {
        self.last_error = 0;
        Ok(())
    }

    fn powerdown_driver(&mut self) -> Result<(), Error> {
        self.core.power_mode = Pn53xPowerMode::PowerDown;
        self.last_error = 0;
        Ok(())
    }
}

impl<T: Pn53xTransport + Send + 'static> TargetBackend for Pn53xDevice<T> {
    fn target_init_driver(
        &mut self,
        target: &mut Target,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error> {
        let command =
            build_target_init_command(self.core.chip_type(), self.core.properties, target)?;
        let timeout = if timeout >= 0 {
            timeout
        } else {
            self.core.timeout_command_ms
        };
        let response = self.exchange_raw(PN53X_TG_INIT_AS_TARGET, &command[1..], timeout)?;
        let Some((&activation_mode, payload)) = response.split_first() else {
            return self.remember(Err(status_error("target_init", NFC_EIO)));
        };
        let (modulation, dep_mode) = decode_activation_mode(activation_mode);
        target.modulation = modulation;
        if let TargetInfo::Dep(info) = &mut target.info {
            info.mode = dep_mode;
        }
        let written = Self::copy_into("target_init", payload, rx)?;
        self.core.remember_target(target.clone());
        self.last_error = 0;
        Ok(written)
    }

    fn target_send_bytes_driver(&mut self, tx: &[u8], timeout: i32) -> Result<usize, Error> {
        let timeout = if timeout >= 0 {
            timeout
        } else {
            self.core.timeout_communication_ms
        };
        self.set_tx_bits(0)?;
        let command = match self.core.current_target() {
            Some(target) if self.core.properties.easy_framing => {
                match target.modulation.modulation_type {
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

    fn target_receive_bytes_driver(&mut self, rx: &mut [u8], timeout: i32) -> Result<usize, Error> {
        let timeout = if timeout >= 0 {
            timeout
        } else {
            self.core.timeout_communication_ms
        };
        let command = match self.core.current_target() {
            Some(target) if self.core.properties.easy_framing => {
                match target.modulation.modulation_type {
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

    fn target_send_bits_driver(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
    ) -> Result<usize, Error> {
        let mut sink = [];
        let _ = self.transceive_bits_shared(BitTransceiveRequest {
            operation: "target_send_bits",
            command: PN53X_TG_RESPONSE_TO_INITIATOR,
            tx,
            tx_bits_len,
            tx_parity,
            rx: &mut sink,
            rx_parity: None,
            timeout_ms: self.core.timeout_communication_ms,
        })?;
        self.last_error = 0;
        Ok(tx_bits_len)
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
        timeout: i32,
    ) -> Result<usize, Error> {
        let Some((&command, payload)) = tx.split_first() else {
            return self.remember(Err(status_error("pn53x_transceive", NFC_EINVARG)));
        };
        let timeout = if timeout >= 0 {
            timeout
        } else {
            self.core.timeout_command_ms
        };
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

    fn pn532_sam_configuration_driver(&mut self, mode: u8, timeout: i32) -> Result<i32, Error> {
        let mode = Pn532SamMode::from_raw(mode)
            .ok_or_else(|| status_error("pn532_SAMConfiguration", NFC_EINVARG))?;
        let timeout = if timeout >= 0 {
            timeout
        } else {
            self.core.timeout_command_ms
        };
        self.sam_configuration(mode, timeout)
    }
}
