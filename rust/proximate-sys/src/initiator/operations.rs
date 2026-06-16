use super::log_general_debug;
use crate::bridge::decode::OutputBytes;
use crate::bridge::decode::{
    InputBytes, ParityMarker, ParityMarkerMut, baud_rate_from_c, decode_modulations,
    decode_optional_dep_info, decode_optional_target, dep_mode_from_c, modulation_from_c,
    property_from_c,
};
use crate::bridge::driver_shim::is_rust_shim_device;
use crate::bridge::encode::{CyclesOut, TargetInOut, TargetOut, TargetSliceOut};
use crate::bridge::status::{NFC_ESOFT, invalid_argument_status, runtime_result_status};
use crate::ffi_catch_unwind_int;
use crate::ffi_types::{
    nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_modulation, nfc_property, nfc_target,
};
use crate::initiator::driver_dispatch::{
    call_abort_command_impl, call_idle_impl, call_initiator_poll_target_impl, dispatch_driver_call,
};
use crate::initiator::runtime;
use crate::lifecycle::nfc_device;
use libc::{c_int, size_t};

fn property_name(property: nfc_property) -> &'static str {
    property_from_c(property).name()
}

pub(crate) unsafe fn nfc_device_set_property_int(
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
        match runtime::set_property_int(device, property_from_c(property), value) {
            Ok(()) => 0,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub(crate) unsafe fn nfc_device_set_property_bool(
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
        match runtime::set_property_bool(device, property_from_c(property), enable) {
            Ok(()) => 0,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub(crate) unsafe fn nfc_initiator_init(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int(
        "nfc_initiator_init",
        NFC_ESOFT,
        || match runtime::initiator_init(device) {
            Ok(status) => status,
            Err(error) => runtime_result_status(device, &error, true),
        },
    )
}

pub(crate) unsafe fn nfc_initiator_init_secure_element(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_init_secure_element", NFC_ESOFT, || {
        match runtime::initiator_init_secure_element(device) {
            Ok(status) => status,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub(crate) unsafe fn nfc_initiator_select_passive_target(
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
            let payload = match InputBytes::from_raw(device, init_data, init_data_len) {
                Ok(bytes) => bytes,
                Err(status) => return status,
            };
            let target = TargetOut::from_raw(target);

            match runtime::select_passive_target(
                device,
                modulation_from_c(nm),
                payload.as_optional(),
            ) {
                Ok(Some(runtime_target)) => {
                    target.write_back(&runtime_target);
                    1
                }
                Ok(None) => 0,
                Err(error) => runtime_result_status(device, &error, true),
            }
        },
    )
}

pub(crate) unsafe fn nfc_initiator_list_passive_targets(
    device: *mut nfc_device,
    nm: nfc_modulation,
    targets: *mut nfc_target,
    targets_len: size_t,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_list_passive_targets", NFC_ESOFT, || unsafe {
        if targets_len == 0 {
            return 0;
        }
        let targets = match TargetSliceOut::from_raw(device, targets, targets_len) {
            Ok(targets) => targets,
            Err(status) => return status,
        };

        match runtime::list_passive_targets(device, modulation_from_c(nm), targets_len) {
            Ok(runtime_targets) => {
                targets.write_back(&runtime_targets);
                runtime_targets.len() as c_int
            }
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub(crate) unsafe fn nfc_initiator_poll_target(
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

        let modulations = match decode_modulations(device, modulations, modulations_len) {
            Ok(modulations) => modulations,
            Err(status) => return status,
        };
        let target = TargetOut::from_raw(target);

        match runtime::poll_target(device, &modulations, poll_nr, period) {
            Ok(Some(runtime_target)) => {
                target.write_back(&runtime_target);
                1
            }
            Ok(None) => 0,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub(crate) unsafe fn nfc_initiator_select_dep_target(
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

        let initiator_info = decode_optional_dep_info(initiator);
        let target = TargetOut::from_raw(target);

        match runtime::select_dep_target(
            device,
            dep_mode_from_c(ndm),
            baud_rate_from_c(nbr),
            initiator_info.as_ref(),
            timeout,
        ) {
            Ok(Some(runtime_target)) => {
                target.write_back(&runtime_target);
                1
            }
            Ok(None) => 0,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub(crate) unsafe fn nfc_initiator_poll_dep_target(
    device: *mut nfc_device,
    ndm: nfc_dep_mode,
    nbr: nfc_baud_rate,
    initiator: *const nfc_dep_info,
    target: *mut nfc_target,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_poll_dep_target", NFC_ESOFT, || unsafe {
        let initiator_info = decode_optional_dep_info(initiator);
        let target = TargetOut::from_raw(target);

        match runtime::poll_dep_target(
            device,
            dep_mode_from_c(ndm),
            baud_rate_from_c(nbr),
            initiator_info.as_ref(),
            timeout,
        ) {
            Ok(Some(runtime_target)) => {
                target.write_back(&runtime_target);
                1
            }
            Ok(None) => 0,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub(crate) unsafe fn nfc_initiator_deselect_target(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_deselect_target", NFC_ESOFT, || {
        match runtime::deselect_target(device) {
            Ok(()) => 0,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub(crate) unsafe fn nfc_initiator_target_is_present(
    device: *mut nfc_device,
    target: *const nfc_target,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_target_is_present", NFC_ESOFT, || unsafe {
        let runtime_target = decode_optional_target(target);
        match runtime::target_is_present(device, runtime_target.as_ref()) {
            Ok(true) => 1,
            Ok(false) => 0,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub(crate) unsafe fn nfc_target_init(
    device: *mut nfc_device,
    target: *mut nfc_target,
    rx: *mut u8,
    rx_len: size_t,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_target_init", NFC_ESOFT, || unsafe {
        let mut target = match TargetInOut::from_raw(device, target) {
            Ok(target) => target,
            Err(status) => return status,
        };
        let mut rx = match OutputBytes::from_raw(device, rx, rx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };

        match runtime::target_init(device, target.as_mut(), rx.as_mut_slice(), timeout) {
            Ok(count) => {
                target.write_back();
                count as c_int
            }
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub(crate) unsafe fn nfc_initiator_transceive_bytes(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: size_t,
    rx: *mut u8,
    rx_len: size_t,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_initiator_transceive_bytes", NFC_ESOFT, || unsafe {
        let tx = match InputBytes::from_raw(device, tx, tx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        let mut rx = match OutputBytes::from_raw(device, rx, rx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        match runtime::transceive_bytes(device, tx.as_slice(), rx.as_mut_slice(), timeout) {
            Ok(count) => count as c_int,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub(crate) unsafe fn nfc_initiator_transceive_bits(
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
            return invalid_argument_status(device);
        }
        if !is_rust_shim_device(device) {
            return dispatch_driver_call(device, |driver| {
                driver
                    .initiator_transceive_bits
                    .map(|callback| callback(device, tx, tx_bits_len, tx_parity, rx, rx_parity))
            });
        }
        let tx = match InputBytes::from_raw(device, tx, tx_bytes_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        let mut rx = match OutputBytes::from_raw(device, rx, rx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        let tx_parity = ParityMarker::from_raw(tx_parity);
        let mut rx_parity = ParityMarkerMut::from_raw(rx_parity);
        match runtime::transceive_bits(
            device,
            tx.as_slice(),
            tx_bits_len,
            tx_parity.as_deref(),
            rx.as_mut_slice(),
            rx_parity.as_deref_mut(),
        ) {
            Ok(count) => count as c_int,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub(crate) unsafe fn nfc_initiator_transceive_bytes_timed(
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
            let tx = match InputBytes::from_raw(device, tx, tx_len) {
                Ok(bytes) => bytes,
                Err(status) => return status,
            };
            let mut rx = match OutputBytes::from_raw(device, rx, rx_len) {
                Ok(bytes) => bytes,
                Err(status) => return status,
            };
            let cycles = CyclesOut::from_raw(cycles);
            match runtime::transceive_bytes_timed(device, tx.as_slice(), rx.as_mut_slice()) {
                Ok((count, measured_cycles)) => {
                    cycles.write_back(measured_cycles);
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
pub(crate) unsafe fn nfc_initiator_transceive_bits_timed(
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
                return invalid_argument_status(device);
            }
            if !is_rust_shim_device(device) {
                return dispatch_driver_call(device, |driver| {
                    driver.initiator_transceive_bits_timed.map(|callback| {
                        callback(device, tx, tx_bits_len, tx_parity, rx, rx_parity, cycles)
                    })
                });
            }
            let tx = match InputBytes::from_raw(device, tx, tx_bytes_len) {
                Ok(bytes) => bytes,
                Err(status) => return status,
            };
            let mut rx = match OutputBytes::from_raw(device, rx, rx_len) {
                Ok(bytes) => bytes,
                Err(status) => return status,
            };
            let tx_parity = ParityMarker::from_raw(tx_parity);
            let mut rx_parity = ParityMarkerMut::from_raw(rx_parity);
            let cycles = CyclesOut::from_raw(cycles);
            match runtime::transceive_bits_timed(
                device,
                tx.as_slice(),
                tx_bits_len,
                tx_parity.as_deref(),
                rx.as_mut_slice(),
                rx_parity.as_deref_mut(),
            ) {
                Ok((count, measured_cycles)) => {
                    cycles.write_back(measured_cycles);
                    count as c_int
                }
                Err(error) => runtime_result_status(device, &error, true),
            }
        },
    )
}

pub(crate) unsafe fn nfc_target_send_bytes(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: size_t,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_target_send_bytes", NFC_ESOFT, || unsafe {
        let tx = match InputBytes::from_raw(device, tx, tx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        match runtime::target_send_bytes(device, tx.as_slice(), timeout) {
            Ok(count) => count as c_int,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub(crate) unsafe fn nfc_target_receive_bytes(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: size_t,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_target_receive_bytes", NFC_ESOFT, || unsafe {
        let mut rx = match OutputBytes::from_raw(device, rx, rx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        match runtime::target_receive_bytes(device, rx.as_mut_slice(), timeout) {
            Ok(count) => count as c_int,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub(crate) unsafe fn nfc_target_send_bits(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: size_t,
    tx_parity: *const u8,
) -> c_int {
    ffi_catch_unwind_int("nfc_target_send_bits", NFC_ESOFT, || unsafe {
        let tx_bytes_len = tx_bits_len.div_ceil(8);
        let tx = match InputBytes::from_raw(device, tx, tx_bytes_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        let tx_parity = ParityMarker::from_raw(tx_parity);
        match runtime::target_send_bits(device, tx.as_slice(), tx_bits_len, tx_parity.as_deref()) {
            Ok(count) => count as c_int,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub(crate) unsafe fn nfc_target_receive_bits(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: size_t,
    rx_parity: *mut u8,
) -> c_int {
    ffi_catch_unwind_int("nfc_target_receive_bits", NFC_ESOFT, || unsafe {
        let mut rx = match OutputBytes::from_raw(device, rx, rx_len) {
            Ok(bytes) => bytes,
            Err(status) => return status,
        };
        let mut rx_parity = ParityMarkerMut::from_raw(rx_parity);
        match runtime::target_receive_bits(device, rx.as_mut_slice(), rx_parity.as_deref_mut()) {
            Ok(count) => count as c_int,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub(crate) unsafe fn nfc_abort_command(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_abort_command", NFC_ESOFT, || unsafe {
        if !is_rust_shim_device(device) {
            return call_abort_command_impl(device);
        }

        match runtime::abort_command(device) {
            Ok(()) => 0,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub(crate) unsafe fn nfc_idle(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_idle", NFC_ESOFT, || unsafe {
        if !is_rust_shim_device(device) {
            return call_idle_impl(device);
        }

        match runtime::idle(device) {
            Ok(()) => 0,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}
