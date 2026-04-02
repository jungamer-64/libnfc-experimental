use super::*;

pub(super) struct RustDeviceState {
    pub(super) handle: Box<dyn rt::OpenedDevice>,
    pub(super) strerror: CString,
    pub(super) supported_modulations: Vec<nfc_modulation_type>,
    pub(super) supported_baud_rates: Vec<nfc_baud_rate>,
}

unsafe fn rust_device_state<'a>(device: *mut nfc_device) -> Option<&'a mut RustDeviceState> {
    let device = unsafe { as_mut(device) }?;
    unsafe { (device.driver_data as *mut RustDeviceState).as_mut() }
}

fn refresh_cached_strerror(state: &mut RustDeviceState) -> *const c_char {
    let message = CString::new(state.handle.strerror())
        .unwrap_or_else(|_| CString::new("invalid strerror").expect("static string is valid"));
    state.strerror = message;
    state.strerror.as_ptr()
}

unsafe extern "C" fn rust_device_close(device: *mut nfc_device) {
    let Some(device_ref) = (unsafe { as_mut(device) }) else {
        return;
    };

    let state_ptr = device_ref.driver_data as *mut RustDeviceState;
    let driver_ptr = device_ref.driver as *mut nfc_driver;
    device_ref.driver_data = ptr::null_mut();
    device_ref.driver = ptr::null();
    if !state_ptr.is_null() {
        unsafe { drop(Box::from_raw(state_ptr)) };
    }
    if !driver_ptr.is_null() {
        unsafe { drop(Box::from_raw(driver_ptr)) };
    }
    unsafe { release_allocated_ptr(device.cast()) };
}

unsafe extern "C" fn rust_device_strerror(device: *const nfc_device) -> *const c_char {
    let Some(state) = (unsafe { rust_device_state(device.cast_mut()) }) else {
        return ptr::null();
    };
    refresh_cached_strerror(state)
}

unsafe extern "C" fn rust_device_initiator_init(device: *mut nfc_device) -> c_int {
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    status_from_result(device, state.handle.initiator_init())
}

unsafe extern "C" fn rust_device_initiator_init_secure_element(device: *mut nfc_device) -> c_int {
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    status_from_result(device, state.handle.initiator_init_secure_element())
}

unsafe extern "C" fn rust_device_select_passive_target(
    device: *mut nfc_device,
    modulation: nfc_modulation,
    init_data: *const u8,
    init_data_len: usize,
    target: *mut nfc_target,
) -> c_int {
    let payload = match unsafe { input_slice(device, init_data, init_data_len) } {
        Ok(bytes) if bytes.is_empty() => None,
        Ok(bytes) => Some(bytes),
        Err(status) => return status,
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    option_target_from_result(
        device,
        target,
        state
            .handle
            .select_passive_target(modulation_from_c(modulation), payload),
    )
}

unsafe extern "C" fn rust_device_poll_target(
    device: *mut nfc_device,
    modulations: *const nfc_modulation,
    modulation_count: usize,
    poll_nr: u8,
    period: u8,
    target: *mut nfc_target,
) -> c_int {
    let modulations = if modulation_count == 0 {
        &[]
    } else if modulations.is_null() {
        set_device_last_error(device, NFC_EINVARG);
        return NFC_EINVARG;
    } else {
        unsafe { slice::from_raw_parts(modulations, modulation_count) }
    };
    let runtime_modulations: Vec<_> = modulations.iter().copied().map(modulation_from_c).collect();
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    option_target_from_result(
        device,
        target,
        state
            .handle
            .poll_target(&runtime_modulations, poll_nr, period),
    )
}

unsafe extern "C" fn rust_device_select_dep_target(
    device: *mut nfc_device,
    dep_mode: nfc_dep_mode,
    baud_rate: nfc_baud_rate,
    initiator: *const nfc_dep_info,
    target: *mut nfc_target,
    timeout: c_int,
) -> c_int {
    let initiator = if initiator.is_null() {
        None
    } else {
        Some(dep_info_from_c(unsafe { ptr::read_unaligned(initiator) }))
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    option_target_from_result(
        device,
        target,
        state.handle.select_dep_target(
            dep_mode_from_c(dep_mode),
            baud_rate_from_c(baud_rate),
            initiator.as_ref(),
            timeout,
        ),
    )
}

unsafe extern "C" fn rust_device_deselect_target(device: *mut nfc_device) -> c_int {
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    status_from_result(device, state.handle.deselect_target().map(|()| 0))
}

unsafe extern "C" fn rust_device_transceive_bytes(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: usize,
    rx: *mut u8,
    rx_len: usize,
    timeout: c_int,
) -> c_int {
    let tx = match unsafe { input_slice(device, tx, tx_len) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let rx = match unsafe { output_slice(device, rx, rx_len) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    count_from_result(device, state.handle.transceive_bytes(tx, rx, timeout))
}

unsafe extern "C" fn rust_device_transceive_bits(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: usize,
    tx_parity: *const u8,
    rx: *mut u8,
    rx_parity: *mut u8,
) -> c_int {
    let tx = match unsafe { input_slice(device, tx, tx_bits_len.div_ceil(8)) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let rx_len = tx_bits_len.div_ceil(8).max(1);
    let rx = match unsafe { output_slice(device, rx, rx_len) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    count_from_result(
        device,
        state.handle.transceive_bits(
            tx,
            tx_bits_len,
            unsafe { parity_marker(tx_parity) },
            rx,
            unsafe { parity_marker_mut(rx_parity) },
        ),
    )
}

unsafe extern "C" fn rust_device_transceive_bytes_timed(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: usize,
    rx: *mut u8,
    rx_len: usize,
    cycles: *mut u32,
) -> c_int {
    let tx = match unsafe { input_slice(device, tx, tx_len) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let rx = match unsafe { output_slice(device, rx, rx_len) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    match state.handle.transceive_bytes_timed(tx, rx) {
        Ok((count, measured_cycles)) => {
            if let Some(cycles) = unsafe { as_mut(cycles) } {
                *cycles = measured_cycles;
            }
            set_device_last_error(device, 0);
            count as c_int
        }
        Err(error) => {
            let status = driver_error_status(&error);
            if status == NFC_EDEVNOTSUPP {
                unsupported_driver_status(device)
            } else {
                set_device_last_error(device, status);
                status
            }
        }
    }
}

unsafe extern "C" fn rust_device_transceive_bits_timed(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: usize,
    tx_parity: *const u8,
    rx: *mut u8,
    rx_parity: *mut u8,
    cycles: *mut u32,
) -> c_int {
    let tx = match unsafe { input_slice(device, tx, tx_bits_len.div_ceil(8)) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let rx_len = tx_bits_len.div_ceil(8).max(1);
    let rx = match unsafe { output_slice(device, rx, rx_len) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    match state.handle.transceive_bits_timed(
        tx,
        tx_bits_len,
        unsafe { parity_marker(tx_parity) },
        rx,
        unsafe { parity_marker_mut(rx_parity) },
    ) {
        Ok((count, measured_cycles)) => {
            if let Some(cycles) = unsafe { as_mut(cycles) } {
                *cycles = measured_cycles;
            }
            set_device_last_error(device, 0);
            count as c_int
        }
        Err(error) => {
            let status = driver_error_status(&error);
            if status == NFC_EDEVNOTSUPP {
                unsupported_driver_status(device)
            } else {
                set_device_last_error(device, status);
                status
            }
        }
    }
}

unsafe extern "C" fn rust_device_target_is_present(
    device: *mut nfc_device,
    target: *const nfc_target,
) -> c_int {
    let runtime_target = (!target.is_null()).then(|| target_from_c(target));
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    bool_from_result(
        device,
        state.handle.target_is_present(runtime_target.as_ref()),
    )
}

unsafe extern "C" fn rust_device_target_init(
    device: *mut nfc_device,
    target: *mut nfc_target,
    rx: *mut u8,
    rx_len: usize,
    timeout: c_int,
) -> c_int {
    if target.is_null() {
        set_device_last_error(device, NFC_EINVARG);
        return NFC_EINVARG;
    }
    let mut runtime_target = target_from_c(target.cast_const());
    let rx = match unsafe { output_slice(device, rx, rx_len) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    match state.handle.target_init(&mut runtime_target, rx, timeout) {
        Ok(count) => {
            set_device_last_error(device, 0);
            write_target_to_c(&runtime_target, target);
            count as c_int
        }
        Err(error) => {
            let status = driver_error_status(&error);
            if status == NFC_EDEVNOTSUPP {
                unsupported_driver_status(device)
            } else {
                set_device_last_error(device, status);
                status
            }
        }
    }
}

unsafe extern "C" fn rust_device_target_send_bytes(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: usize,
    timeout: c_int,
) -> c_int {
    let tx = match unsafe { input_slice(device, tx, tx_len) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    count_from_result(device, state.handle.target_send_bytes(tx, timeout))
}

unsafe extern "C" fn rust_device_target_receive_bytes(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: usize,
    timeout: c_int,
) -> c_int {
    let rx = match unsafe { output_slice(device, rx, rx_len) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    count_from_result(device, state.handle.target_receive_bytes(rx, timeout))
}

unsafe extern "C" fn rust_device_target_send_bits(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: usize,
    tx_parity: *const u8,
) -> c_int {
    let tx = match unsafe { input_slice(device, tx, tx_bits_len.div_ceil(8)) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    count_from_result(
        device,
        state
            .handle
            .target_send_bits(tx, tx_bits_len, unsafe { parity_marker(tx_parity) }),
    )
}

unsafe extern "C" fn rust_device_target_receive_bits(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: usize,
    rx_parity: *mut u8,
) -> c_int {
    let rx = match unsafe { output_slice(device, rx, rx_len) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    count_from_result(
        device,
        state
            .handle
            .target_receive_bits(rx, unsafe { parity_marker_mut(rx_parity) }),
    )
}

unsafe extern "C" fn rust_device_set_property_bool(
    device: *mut nfc_device,
    property: nfc_property,
    enable: bool,
) -> c_int {
    let runtime_property = property_from_c(property);
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    match state.handle.set_property_bool(runtime_property, enable) {
        Ok(()) => {
            let mirrored = state
                .handle
                .property_bool_state(runtime_property)
                .unwrap_or(enable);
            sync_bool_property(device, runtime_property, mirrored);
            set_device_last_error(device, 0);
            0
        }
        Err(error) => {
            let status = driver_error_status(&error);
            if status == NFC_EDEVNOTSUPP {
                unsupported_driver_status(device)
            } else {
                set_device_last_error(device, status);
                status
            }
        }
    }
}

unsafe extern "C" fn rust_device_set_property_int(
    device: *mut nfc_device,
    property: nfc_property,
    value: c_int,
) -> c_int {
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    status_from_result(
        device,
        state
            .handle
            .set_property_int(property_from_c(property), value)
            .map(|()| 0),
    )
}

unsafe extern "C" fn rust_device_get_supported_modulation(
    device: *mut nfc_device,
    mode: nfc_mode,
    supported: *mut *const nfc_modulation_type,
) -> c_int {
    let Some(supported) = (unsafe { as_mut(supported) }) else {
        set_device_last_error(device, NFC_EINVARG);
        return NFC_EINVARG;
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    match state.handle.supported_modulations(match mode {
        nfc_mode::N_TARGET => rt::Mode::Target,
        nfc_mode::N_INITIATOR => rt::Mode::Initiator,
    }) {
        Ok(values) => {
            state.supported_modulations = values.into_iter().map(modulation_type_to_c).collect();
            state
                .supported_modulations
                .push(nfc_modulation_type::NMT_UNDEFINED);
            *supported = state.supported_modulations.as_ptr();
            set_device_last_error(device, 0);
            0
        }
        Err(error) => {
            let status = driver_error_status(&error);
            if status == NFC_EDEVNOTSUPP {
                unsupported_driver_status(device)
            } else {
                set_device_last_error(device, status);
                status
            }
        }
    }
}

unsafe extern "C" fn rust_device_get_supported_baud_rate(
    device: *mut nfc_device,
    mode: nfc_mode,
    modulation_type: nfc_modulation_type,
    supported: *mut *const nfc_baud_rate,
) -> c_int {
    let Some(supported) = (unsafe { as_mut(supported) }) else {
        set_device_last_error(device, NFC_EINVARG);
        return NFC_EINVARG;
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    match state.handle.supported_baud_rates(
        match mode {
            nfc_mode::N_TARGET => rt::Mode::Target,
            nfc_mode::N_INITIATOR => rt::Mode::Initiator,
        },
        modulation_type_from_c(modulation_type),
    ) {
        Ok(values) => {
            state.supported_baud_rates = values.into_iter().map(baud_rate_to_c).collect();
            state
                .supported_baud_rates
                .push(nfc_baud_rate::NBR_UNDEFINED);
            *supported = state.supported_baud_rates.as_ptr();
            set_device_last_error(device, 0);
            0
        }
        Err(error) => {
            let status = driver_error_status(&error);
            if status == NFC_EDEVNOTSUPP {
                unsupported_driver_status(device)
            } else {
                set_device_last_error(device, status);
                status
            }
        }
    }
}

unsafe extern "C" fn rust_device_get_information_about(
    device: *mut nfc_device,
    buffer: *mut *mut c_char,
) -> c_int {
    let Some(buffer) = (unsafe { as_mut(buffer) }) else {
        set_device_last_error(device, NFC_EINVARG);
        return NFC_EINVARG;
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    match state.handle.information_about() {
        Ok(message) => {
            let allocation = unsafe { libc::malloc(message.len() + 1) as *mut c_char };
            if allocation.is_null() {
                set_device_last_error(device, NFC_ESOFT);
                return NFC_ESOFT;
            }
            if !unsafe { copy_bytes_to_c_buffer(allocation, message.len() + 1, message.as_bytes()) }
            {
                unsafe { release_allocated_ptr(allocation.cast()) };
                set_device_last_error(device, NFC_ESOFT);
                return NFC_ESOFT;
            }
            *buffer = allocation;
            set_device_last_error(device, 0);
            0
        }
        Err(error) => {
            let status = driver_error_status(&error);
            if status == NFC_EDEVNOTSUPP {
                unsupported_driver_status(device)
            } else {
                set_device_last_error(device, status);
                status
            }
        }
    }
}

unsafe extern "C" fn rust_device_abort_command(device: *mut nfc_device) -> c_int {
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    status_from_result(device, state.handle.abort_command().map(|()| 0))
}

unsafe extern "C" fn rust_device_idle(device: *mut nfc_device) -> c_int {
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    status_from_result(device, state.handle.idle().map(|()| 0))
}

unsafe extern "C" fn rust_device_powerdown(device: *mut nfc_device) -> c_int {
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    status_from_result(device, state.handle.powerdown().map(|()| 0))
}

pub(super) fn build_rust_device_shim_driver(caps: rt::DeviceCaps) -> nfc_driver {
    nfc_driver {
        name: RUST_DEVICE_DRIVER_NAME,
        scan_type: scan_type_enum::NOT_AVAILABLE,
        scan: None,
        open: None,
        close: Some(rust_device_close),
        strerror: Some(rust_device_strerror),
        initiator_init: caps
            .contains(rt::DeviceCaps::INITIATOR_INIT)
            .then_some(rust_device_initiator_init),
        initiator_init_secure_element: caps
            .contains(rt::DeviceCaps::INITIATOR_INIT_SECURE_ELEMENT)
            .then_some(rust_device_initiator_init_secure_element),
        initiator_select_passive_target: caps
            .contains(rt::DeviceCaps::SELECT_PASSIVE_TARGET)
            .then_some(rust_device_select_passive_target),
        initiator_poll_target: caps
            .contains(rt::DeviceCaps::POLL_TARGET)
            .then_some(rust_device_poll_target),
        initiator_select_dep_target: caps
            .contains(rt::DeviceCaps::SELECT_DEP_TARGET)
            .then_some(rust_device_select_dep_target),
        initiator_deselect_target: caps
            .contains(rt::DeviceCaps::DESELECT_TARGET)
            .then_some(rust_device_deselect_target),
        initiator_transceive_bytes: caps
            .contains(rt::DeviceCaps::TRANSCEIVE_BYTES)
            .then_some(rust_device_transceive_bytes),
        initiator_transceive_bits: caps
            .contains(rt::DeviceCaps::TRANSCEIVE_BITS)
            .then_some(rust_device_transceive_bits),
        initiator_transceive_bytes_timed: caps
            .contains(rt::DeviceCaps::TRANSCEIVE_BYTES_TIMED)
            .then_some(rust_device_transceive_bytes_timed),
        initiator_transceive_bits_timed: caps
            .contains(rt::DeviceCaps::TRANSCEIVE_BITS_TIMED)
            .then_some(rust_device_transceive_bits_timed),
        initiator_target_is_present: caps
            .contains(rt::DeviceCaps::TARGET_IS_PRESENT)
            .then_some(rust_device_target_is_present),
        target_init: caps
            .contains(rt::DeviceCaps::TARGET_INIT)
            .then_some(rust_device_target_init),
        target_send_bytes: caps
            .contains(rt::DeviceCaps::TARGET_SEND_BYTES)
            .then_some(rust_device_target_send_bytes),
        target_receive_bytes: caps
            .contains(rt::DeviceCaps::TARGET_RECEIVE_BYTES)
            .then_some(rust_device_target_receive_bytes),
        target_send_bits: caps
            .contains(rt::DeviceCaps::TARGET_SEND_BITS)
            .then_some(rust_device_target_send_bits),
        target_receive_bits: caps
            .contains(rt::DeviceCaps::TARGET_RECEIVE_BITS)
            .then_some(rust_device_target_receive_bits),
        device_set_property_bool: caps
            .contains(rt::DeviceCaps::SET_PROPERTY_BOOL)
            .then_some(rust_device_set_property_bool),
        device_set_property_int: caps
            .contains(rt::DeviceCaps::SET_PROPERTY_INT)
            .then_some(rust_device_set_property_int),
        get_supported_modulation: caps
            .contains(rt::DeviceCaps::SUPPORTED_MODULATIONS)
            .then_some(rust_device_get_supported_modulation),
        get_supported_baud_rate: caps
            .contains(rt::DeviceCaps::SUPPORTED_BAUD_RATES)
            .then_some(rust_device_get_supported_baud_rate),
        device_get_information_about: caps
            .contains(rt::DeviceCaps::INFO)
            .then_some(rust_device_get_information_about),
        abort_command: caps
            .contains(rt::DeviceCaps::ABORT_COMMAND)
            .then_some(rust_device_abort_command),
        idle: caps
            .contains(rt::DeviceCaps::IDLE)
            .then_some(rust_device_idle),
        powerdown: caps
            .contains(rt::DeviceCaps::POWERDOWN)
            .then_some(rust_device_powerdown),
    }
}

pub(crate) fn attach_rust_device(
    device: rt::Device,
    context: *const nfc_context,
) -> Result<*mut nfc_device, rt::Error> {
    let name = device.name().to_string();
    let connstring = device.connstring().clone();
    let handle = device.into_handle();
    let caps = handle.caps();
    let connstring_c =
        CString::new(connstring.as_str()).map_err(|_| rt::Error::InvalidEncoding("connstring"))?;
    let raw = unsafe { nfc_device_new(context, connstring_c.as_ptr()) };
    if raw.is_null() {
        return Err(rt::Error::DriverOpenFailed(connstring.as_str().to_string()));
    }

    let state = Box::new(RustDeviceState {
        strerror: CString::new(handle.strerror())
            .unwrap_or_else(|_| CString::new("invalid strerror").expect("static string is valid")),
        handle,
        supported_modulations: Vec::new(),
        supported_baud_rates: Vec::new(),
    });
    let driver = Box::new(build_rust_device_shim_driver(caps));
    let driver_ptr = Box::into_raw(driver);
    if let Some(device_ref) = unsafe { as_mut(raw) } {
        device_ref.context = context;
        device_ref.driver = driver_ptr;
        device_ref.driver_data = Box::into_raw(state).cast();
    }

    if !copy_device_identity(raw, &name, &connstring) {
        unsafe { rust_device_close(raw) };
        return Err(rt::Error::BufferTooSmall {
            needed: name.len().max(connstring.as_str().len()) + 1,
            available: DEVICE_NAME_LENGTH.min(NFC_BUFSIZE_CONNSTRING),
        });
    }

    if let Some(state) = unsafe { rust_device_state(raw) } {
        sync_property_mirrors(raw, state.handle.as_ref());
        set_device_last_error(raw, state.handle.last_error());
    }

    Ok(raw)
}

pub(crate) fn is_rust_shim_device(raw: *mut nfc_device) -> bool {
    unsafe { as_ref(raw) }
        .and_then(|device| unsafe { as_ref(device.driver) })
        .map(|driver| ptr::eq(driver.name, RUST_DEVICE_DRIVER_NAME))
        .unwrap_or(false)
}
