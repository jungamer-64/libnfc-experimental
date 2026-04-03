use super::*;

fn invalid_argument(device: *mut nfc_device) -> c_int {
    unsafe { set_device_last_error(device, NFC_EINVARG) };
    NFC_EINVARG
}

fn soft_error(device: *mut nfc_device) -> c_int {
    unsafe { set_device_last_error(device, NFC_ESOFT) };
    NFC_ESOFT
}

pub(super) struct InputBytes<'a>(&'a [u8]);

impl<'a> InputBytes<'a> {
    pub(super) unsafe fn from_raw(
        device: *mut nfc_device,
        bytes: *const u8,
        len: size_t,
    ) -> Result<Self, c_int> {
        if len == 0 {
            return Ok(Self(&[]));
        }
        if bytes.is_null() {
            return Err(invalid_argument(device));
        }
        Ok(Self(unsafe { slice::from_raw_parts(bytes, len) }))
    }

    pub(super) fn as_slice(&self) -> &[u8] {
        self.0
    }

    pub(super) fn as_optional(&self) -> Option<&[u8]> {
        (!self.0.is_empty()).then_some(self.0)
    }
}

pub(super) struct OutputBytes<'a>(&'a mut [u8]);

impl<'a> OutputBytes<'a> {
    pub(super) unsafe fn from_raw(
        device: *mut nfc_device,
        bytes: *mut u8,
        len: size_t,
    ) -> Result<Self, c_int> {
        if len == 0 {
            return Ok(Self(&mut []));
        }
        if bytes.is_null() {
            return Err(invalid_argument(device));
        }
        Ok(Self(unsafe { slice::from_raw_parts_mut(bytes, len) }))
    }

    pub(super) fn as_mut_slice(&mut self) -> &mut [u8] {
        self.0
    }
}

pub(super) struct ParityMarker<'a>(Option<&'a [u8]>);

impl<'a> ParityMarker<'a> {
    pub(super) unsafe fn from_raw(bytes: *const u8) -> Self {
        if bytes.is_null() {
            Self(None)
        } else {
            Self(Some(unsafe { slice::from_raw_parts(bytes, 1) }))
        }
    }

    pub(super) fn as_deref(&self) -> Option<&[u8]> {
        self.0
    }
}

pub(super) struct ParityMarkerMut<'a>(Option<&'a mut [u8]>);

impl<'a> ParityMarkerMut<'a> {
    pub(super) unsafe fn from_raw(bytes: *mut u8) -> Self {
        if bytes.is_null() {
            Self(None)
        } else {
            Self(Some(unsafe { slice::from_raw_parts_mut(bytes, 1) }))
        }
    }

    pub(super) fn as_deref_mut(&mut self) -> Option<&mut [u8]> {
        self.0.as_deref_mut()
    }
}

pub(super) struct TargetOut {
    raw: *mut nfc_target,
}

impl TargetOut {
    pub(super) unsafe fn from_raw(raw: *mut nfc_target) -> Self {
        Self { raw }
    }

    pub(super) fn write_back(&self, target: &rt::Target) {
        if !self.raw.is_null() {
            write_target_to_c(target, self.raw);
        }
    }
}

pub(super) struct TargetSliceOut {
    raw: *mut nfc_target,
    len: usize,
}

impl TargetSliceOut {
    pub(super) unsafe fn from_raw(
        device: *mut nfc_device,
        raw: *mut nfc_target,
        len: size_t,
    ) -> Result<Self, c_int> {
        if len == 0 {
            return Ok(Self { raw, len: 0 });
        }
        if raw.is_null() {
            return Err(invalid_argument(device));
        }
        Ok(Self { raw, len })
    }

    pub(super) fn write_back(&self, targets: &[rt::Target]) {
        for (index, target) in targets.iter().take(self.len).enumerate() {
            write_target_to_c(target, unsafe { self.raw.add(index) });
        }
    }
}

pub(super) struct TargetInOut {
    raw: *mut nfc_target,
    value: rt::Target,
}

impl TargetInOut {
    pub(super) unsafe fn from_raw(
        device: *mut nfc_device,
        raw: *mut nfc_target,
    ) -> Result<Self, c_int> {
        if raw.is_null() {
            return Err(invalid_argument(device));
        }
        Ok(Self {
            raw,
            value: target_from_c(raw.cast_const()),
        })
    }

    pub(super) fn as_mut(&mut self) -> &mut rt::Target {
        &mut self.value
    }

    pub(super) fn write_back(&self) {
        write_target_to_c(&self.value, self.raw);
    }
}

pub(super) struct CyclesOut {
    raw: *mut u32,
}

impl CyclesOut {
    pub(super) unsafe fn from_raw(raw: *mut u32) -> Self {
        Self { raw }
    }

    pub(super) fn write_back(&self, cycles: u32) {
        if let Some(raw) = unsafe { as_mut(self.raw) } {
            *raw = cycles;
        }
    }
}

pub(super) struct CStringOut {
    raw: *mut *mut c_char,
}

impl CStringOut {
    pub(super) unsafe fn from_raw(
        device: *mut nfc_device,
        raw: *mut *mut c_char,
    ) -> Result<Self, c_int> {
        if raw.is_null() {
            return Err(invalid_argument(device));
        }
        Ok(Self { raw })
    }

    pub(super) fn write_back(&self, device: *mut nfc_device, value: &str) -> c_int {
        let rendered = match CString::new(value) {
            Ok(value) => value,
            Err(_) => return soft_error(device),
        };
        let allocation_len = rendered.as_bytes().len() + 1;
        let allocation = unsafe { libc::malloc(allocation_len) as *mut c_char };
        if allocation.is_null() {
            return soft_error(device);
        }

        if !unsafe { copy_bytes_to_c_buffer(allocation, allocation_len, rendered.as_bytes()) } {
            unsafe { libc::free(allocation.cast()) };
            return soft_error(device);
        }

        unsafe {
            *self.raw = allocation;
            set_device_last_error(device, 0);
        }
        rendered.as_bytes().len() as c_int
    }
}

pub(super) struct SupportedModulationsOut {
    device: *mut nfc_device,
    raw: *mut *const nfc_modulation_type,
}

impl SupportedModulationsOut {
    pub(super) unsafe fn from_raw(
        device: *mut nfc_device,
        raw: *mut *const nfc_modulation_type,
    ) -> Result<Self, c_int> {
        if raw.is_null() || unsafe { rust_device_state_mut(device) }.is_none() {
            return Err(invalid_argument(device));
        }
        Ok(Self { device, raw })
    }

    pub(super) fn write_back(&self, values: Vec<rt::ModulationType>) -> c_int {
        let Some(state) = (unsafe { rust_device_state_mut(self.device) }) else {
            return invalid_argument(self.device);
        };
        state.supported_modulations.clear();
        state
            .supported_modulations
            .extend(values.into_iter().map(modulation_type_to_c));
        state
            .supported_modulations
            .push(nfc_modulation_type::NMT_UNDEFINED);
        unsafe {
            *self.raw = state.supported_modulations.as_ptr();
            set_device_last_error(self.device, 0);
        }
        0
    }
}

pub(super) struct SupportedBaudRatesOut {
    device: *mut nfc_device,
    raw: *mut *const nfc_baud_rate,
}

impl SupportedBaudRatesOut {
    pub(super) unsafe fn from_raw(
        device: *mut nfc_device,
        raw: *mut *const nfc_baud_rate,
    ) -> Result<Self, c_int> {
        if raw.is_null() || unsafe { rust_device_state_mut(device) }.is_none() {
            return Err(invalid_argument(device));
        }
        Ok(Self { device, raw })
    }

    pub(super) fn write_back(&self, values: Vec<rt::BaudRate>) -> c_int {
        let Some(state) = (unsafe { rust_device_state_mut(self.device) }) else {
            return invalid_argument(self.device);
        };
        state.supported_baud_rates.clear();
        state
            .supported_baud_rates
            .extend(values.into_iter().map(baud_rate_to_c));
        state
            .supported_baud_rates
            .push(nfc_baud_rate::NBR_UNDEFINED);
        unsafe {
            *self.raw = state.supported_baud_rates.as_ptr();
            set_device_last_error(self.device, 0);
        }
        0
    }
}

pub(super) unsafe fn decode_modulations(
    device: *mut nfc_device,
    modulations: *const nfc_modulation,
    len: size_t,
) -> Result<Vec<rt::Modulation>, c_int> {
    if len == 0 {
        return Ok(Vec::new());
    }
    if modulations.is_null() {
        return Err(invalid_argument(device));
    }
    Ok(unsafe { slice::from_raw_parts(modulations, len) }
        .iter()
        .copied()
        .map(modulation_from_c)
        .collect())
}

pub(super) unsafe fn decode_optional_dep_info(
    initiator: *const nfc_dep_info,
) -> Option<rt::DepInfo> {
    if initiator.is_null() {
        None
    } else {
        Some(dep_info_from_c(unsafe { ptr::read_unaligned(initiator) }))
    }
}

pub(super) unsafe fn decode_optional_target(target: *const nfc_target) -> Option<rt::Target> {
    (!target.is_null()).then(|| target_from_c(target))
}
