use super::*;

fn with_initiator_device<R>(
    device: *mut nfc_device,
    f: impl FnOnce(&mut rt::InitiatorDevice<'_>) -> Result<R, rt::Error>,
) -> Result<R, rt::Error> {
    let mut runtime_device = borrowed_device(device);
    let mut initiator = runtime_device.initiator()?;
    f(&mut initiator)
}

fn with_target_device<R>(
    device: *mut nfc_device,
    f: impl FnOnce(&mut rt::TargetDevice<'_>) -> Result<R, rt::Error>,
) -> Result<R, rt::Error> {
    let mut runtime_device = borrowed_device(device);
    let mut target = runtime_device.target()?;
    f(&mut target)
}

pub unsafe fn nfc_device_set_property_int(
    device: *mut nfc_device,
    property: nfc_property,
    value: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_device_set_property_int", NFC_ESOFT, || {
        log_general_debug(&format!(
            "set_property_int {} {}",
            property_name(property),
            if value != 0 { "True" } else { "False" }
        ));
        let mut adapter = borrowed_device(device);
        match adapter.set_property_int(property_from_c(property), value) {
            Ok(()) => 0,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub unsafe fn nfc_device_set_property_bool(
    device: *mut nfc_device,
    property: nfc_property,
    enable: bool,
) -> c_int {
    ffi_catch_unwind_int("nfc_device_set_property_bool", NFC_ESOFT, || {
        log_general_debug(&format!(
            "set_property_bool {} {}",
            property_name(property),
            if enable { "True" } else { "False" }
        ));
        let mut adapter = borrowed_device(device);
        match adapter.set_property_bool(property_from_c(property), enable) {
            Ok(()) => 0,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub unsafe fn nfc_initiator_init(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int(
        "nfc_initiator_init",
        NFC_ESOFT,
        || match with_initiator_device(device, |initiator| initiator.init()) {
            Ok(status) => status,
            Err(error) => runtime_result_status(device, &error, true),
        },
    )
}

pub unsafe fn nfc_initiator_init_secure_element(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_init_secure_element", NFC_ESOFT, || {
        match with_initiator_device(device, |initiator| initiator.init_secure_element()) {
            Ok(status) => status,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub unsafe fn nfc_initiator_select_passive_target(
    device: *mut nfc_device,
    nm: nfc_modulation,
    init_data: *const u8,
    init_data_len: size_t,
    target: *mut nfc_target,
) -> c_int {
    ffi_catch_unwind_int(
        "nfc_initiator_select_passive_target",
        NFC_ESOFT,
        || unsafe {
            let payload = match input_bytes(device, init_data, init_data_len) {
                Ok([]) => None,
                Ok(bytes) => Some(bytes),
                Err(status) => return status,
            };

            match with_initiator_device(device, |initiator| {
                initiator.select_passive_target(modulation_from_c(nm), payload)
            }) {
                Ok(Some(runtime_target)) => {
                    if !target.is_null() {
                        write_target_to_c(&runtime_target, target);
                    }
                    1
                }
                Ok(None) => 0,
                Err(error) => runtime_result_status(device, &error, true),
            }
        },
    )
}

pub unsafe fn nfc_initiator_list_passive_targets(
    device: *mut nfc_device,
    nm: nfc_modulation,
    targets: *mut nfc_target,
    targets_len: size_t,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_list_passive_targets", NFC_ESOFT, || unsafe {
        if targets_len == 0 {
            return 0;
        }
        if targets.is_null() {
            set_device_last_error(device, NFC_EINVARG);
            return NFC_EINVARG;
        }

        match with_initiator_device(device, |initiator| {
            initiator.list_passive_targets(modulation_from_c(nm), targets_len)
        }) {
            Ok(runtime_targets) => {
                for (index, runtime_target) in runtime_targets.iter().enumerate() {
                    write_target_to_c(runtime_target, targets.add(index));
                }
                runtime_targets.len() as c_int
            }
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub unsafe fn nfc_initiator_poll_target(
    device: *mut nfc_device,
    modulations: *const nfc_modulation,
    modulations_len: size_t,
    poll_nr: u8,
    period: u8,
    target: *mut nfc_target,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_poll_target", NFC_ESOFT, || unsafe {
        if !is_rust_shim_device(device) {
            return call_initiator_poll_target_impl(
                device,
                modulations,
                modulations_len,
                poll_nr,
                period,
                target,
            );
        }

        let modulations = if modulations_len == 0 {
            &[]
        } else if modulations.is_null() {
            set_device_last_error(device, NFC_EINVARG);
            return NFC_EINVARG;
        } else {
            slice::from_raw_parts(modulations, modulations_len)
        };
        let runtime_modulations: Vec<_> =
            modulations.iter().copied().map(modulation_from_c).collect();

        match with_initiator_device(device, |initiator| {
            initiator.poll_target(&runtime_modulations, poll_nr, period)
        }) {
            Ok(Some(runtime_target)) => {
                if !target.is_null() {
                    write_target_to_c(&runtime_target, target);
                }
                1
            }
            Ok(None) => 0,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub unsafe fn nfc_initiator_select_dep_target(
    device: *mut nfc_device,
    ndm: nfc_dep_mode,
    nbr: nfc_baud_rate,
    initiator: *const nfc_dep_info,
    target: *mut nfc_target,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_select_dep_target", NFC_ESOFT, || unsafe {
        if !is_rust_shim_device(device) {
            return dispatch_driver_call(device, |driver| {
                driver
                    .initiator_select_dep_target
                    .map(|callback| callback(device, ndm, nbr, initiator, target, timeout))
            });
        }

        let initiator_info = if initiator.is_null() {
            None
        } else {
            Some(dep_info_from_c(ptr::read_unaligned(initiator)))
        };
        match with_initiator_device(device, |initiator| {
            initiator.select_dep_target(
                dep_mode_from_c(ndm),
                baud_rate_from_c(nbr),
                initiator_info.as_ref(),
                timeout,
            )
        }) {
            Ok(Some(runtime_target)) => {
                if !target.is_null() {
                    write_target_to_c(&runtime_target, target);
                }
                1
            }
            Ok(None) => 0,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub unsafe fn nfc_initiator_poll_dep_target(
    device: *mut nfc_device,
    ndm: nfc_dep_mode,
    nbr: nfc_baud_rate,
    initiator: *const nfc_dep_info,
    target: *mut nfc_target,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_poll_dep_target", NFC_ESOFT, || unsafe {
        let initiator_info = if initiator.is_null() {
            None
        } else {
            Some(dep_info_from_c(ptr::read_unaligned(initiator)))
        };
        match with_initiator_device(device, |initiator| {
            initiator.poll_dep_target(
                dep_mode_from_c(ndm),
                baud_rate_from_c(nbr),
                initiator_info.as_ref(),
                timeout,
            )
        }) {
            Ok(Some(runtime_target)) => {
                if !target.is_null() {
                    write_target_to_c(&runtime_target, target);
                }
                1
            }
            Ok(None) => 0,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub unsafe fn nfc_initiator_deselect_target(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int(
        "nfc_initiator_deselect_target",
        NFC_ESOFT,
        || match with_initiator_device(device, |initiator| initiator.deselect_target()) {
            Ok(()) => 0,
            Err(error) => runtime_result_status(device, &error, true),
        },
    )
}

pub unsafe fn nfc_initiator_target_is_present(
    device: *mut nfc_device,
    target: *const nfc_target,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_target_is_present", NFC_ESOFT, || {
        let runtime_target = (!target.is_null()).then(|| target_from_c(target));
        match with_initiator_device(device, |initiator| {
            initiator.target_is_present(runtime_target.as_ref())
        }) {
            Ok(true) => 1,
            Ok(false) => 0,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub unsafe fn nfc_target_init(
    device: *mut nfc_device,
    target: *mut nfc_target,
    rx: *mut u8,
    rx_len: size_t,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_target_init", NFC_ESOFT, || unsafe {
        if target.is_null() {
            set_device_last_error(device, NFC_EINVARG);
            return NFC_EINVARG;
        }
        let mut runtime_target = target_from_c(target.cast_const());
        let rx = match output_bytes(device, rx, rx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };

        match with_target_device(device, |target_device| {
            target_device.init(&mut runtime_target, rx, timeout)
        }) {
            Ok(count) => {
                write_target_to_c(&runtime_target, target);
                count as c_int
            }
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub unsafe fn nfc_initiator_transceive_bytes(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: size_t,
    rx: *mut u8,
    rx_len: size_t,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_transceive_bytes", NFC_ESOFT, || unsafe {
        let tx = match input_bytes(device, tx, tx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        let rx = match output_bytes(device, rx, rx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        match with_initiator_device(device, |initiator| {
            initiator.transceive_bytes(tx, rx, timeout)
        }) {
            Ok(count) => count as c_int,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub unsafe fn nfc_initiator_transceive_bits(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: size_t,
    tx_parity: *const u8,
    rx: *mut u8,
    rx_len: size_t,
    rx_parity: *mut u8,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_transceive_bits", NFC_ESOFT, || unsafe {
        let tx_bytes_len = tx_bits_len.div_ceil(8);
        if tx_bytes_len > 0 && tx.is_null() {
            set_device_last_error(device, NFC_EINVARG);
            return NFC_EINVARG;
        }
        if !is_rust_shim_device(device) {
            return dispatch_driver_call(device, |driver| {
                driver
                    .initiator_transceive_bits
                    .map(|callback| callback(device, tx, tx_bits_len, tx_parity, rx, rx_parity))
            });
        }
        let tx = match input_bytes(device, tx, tx_bytes_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        let rx = match output_bytes(device, rx, rx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        match with_initiator_device(device, |initiator| {
            initiator.transceive_bits(
                tx,
                tx_bits_len,
                marker_bytes(tx_parity),
                rx,
                marker_bytes_mut(rx_parity),
            )
        }) {
            Ok(count) => count as c_int,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub unsafe fn nfc_initiator_transceive_bytes_timed(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: size_t,
    rx: *mut u8,
    rx_len: size_t,
    cycles: *mut u32,
) -> c_int {
    ffi_catch_unwind_int(
        "nfc_initiator_transceive_bytes_timed",
        NFC_ESOFT,
        || unsafe {
            let tx = match input_bytes(device, tx, tx_len) {
                Ok(bytes) => bytes,
                Err(status) => return status,
            };
            let rx = match output_bytes(device, rx, rx_len) {
                Ok(bytes) => bytes,
                Err(status) => return status,
            };
            match with_initiator_device(device, |initiator| {
                initiator.transceive_bytes_timed(tx, rx)
            }) {
                Ok((count, measured_cycles)) => {
                    if let Some(cycles) = as_mut(cycles) {
                        *cycles = measured_cycles;
                    }
                    count as c_int
                }
                Err(error) => runtime_result_status(device, &error, true),
            }
        },
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "Mirrors the libnfc C ABI entrypoint shape."
)]
pub unsafe fn nfc_initiator_transceive_bits_timed(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: size_t,
    tx_parity: *const u8,
    rx: *mut u8,
    rx_len: size_t,
    rx_parity: *mut u8,
    cycles: *mut u32,
) -> c_int {
    ffi_catch_unwind_int(
        "nfc_initiator_transceive_bits_timed",
        NFC_ESOFT,
        || unsafe {
            let tx_bytes_len = tx_bits_len.div_ceil(8);
            if tx_bytes_len > 0 && tx.is_null() {
                set_device_last_error(device, NFC_EINVARG);
                return NFC_EINVARG;
            }
            if !is_rust_shim_device(device) {
                return dispatch_driver_call(device, |driver| {
                    driver.initiator_transceive_bits_timed.map(|callback| {
                        callback(device, tx, tx_bits_len, tx_parity, rx, rx_parity, cycles)
                    })
                });
            }
            let tx = match input_bytes(device, tx, tx_bytes_len) {
                Ok(bytes) => bytes,
                Err(status) => return status,
            };
            let rx = match output_bytes(device, rx, rx_len) {
                Ok(bytes) => bytes,
                Err(status) => return status,
            };
            match with_initiator_device(device, |initiator| {
                initiator.transceive_bits_timed(
                    tx,
                    tx_bits_len,
                    marker_bytes(tx_parity),
                    rx,
                    marker_bytes_mut(rx_parity),
                )
            }) {
                Ok((count, measured_cycles)) => {
                    if let Some(cycles) = as_mut(cycles) {
                        *cycles = measured_cycles;
                    }
                    count as c_int
                }
                Err(error) => runtime_result_status(device, &error, true),
            }
        },
    )
}

pub unsafe fn nfc_target_send_bytes(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: size_t,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_target_send_bytes", NFC_ESOFT, || unsafe {
        let tx = match input_bytes(device, tx, tx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        match with_target_device(device, |target_device| {
            target_device.send_bytes(tx, timeout)
        }) {
            Ok(count) => count as c_int,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub unsafe fn nfc_target_receive_bytes(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: size_t,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_target_receive_bytes", NFC_ESOFT, || unsafe {
        let rx = match output_bytes(device, rx, rx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        match with_target_device(device, |target_device| {
            target_device.receive_bytes(rx, timeout)
        }) {
            Ok(count) => count as c_int,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub unsafe fn nfc_target_send_bits(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: size_t,
    tx_parity: *const u8,
) -> c_int {
    ffi_catch_unwind_int("nfc_target_send_bits", NFC_ESOFT, || unsafe {
        let tx_bytes_len = tx_bits_len.div_ceil(8);
        let tx = match input_bytes(device, tx, tx_bytes_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        match with_target_device(device, |target_device| {
            target_device.send_bits(tx, tx_bits_len, marker_bytes(tx_parity))
        }) {
            Ok(count) => count as c_int,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub unsafe fn nfc_target_receive_bits(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: size_t,
    rx_parity: *mut u8,
) -> c_int {
    ffi_catch_unwind_int("nfc_target_receive_bits", NFC_ESOFT, || unsafe {
        let rx = match output_bytes(device, rx, rx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        match with_target_device(device, |target_device| {
            target_device.receive_bits(rx, marker_bytes_mut(rx_parity))
        }) {
            Ok(count) => count as c_int,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub unsafe fn nfc_abort_command(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_abort_command", NFC_ESOFT, || unsafe {
        if !is_rust_shim_device(device) {
            return call_abort_command_impl(device);
        }

        match with_initiator_device(device, |initiator| initiator.abort_command()) {
            Ok(()) => 0,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub unsafe fn nfc_idle(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_idle", NFC_ESOFT, || unsafe {
        if !is_rust_shim_device(device) {
            return call_idle_impl(device);
        }

        match with_initiator_device(device, |initiator| initiator.idle()) {
            Ok(()) => 0,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}
