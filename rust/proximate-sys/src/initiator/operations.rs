use super::log_general_debug;
use crate::c_abi::types::{
    nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_modulation, nfc_property, nfc_target,
};
use crate::c_boundary::status::{
    NFC_ESOFT, bounded_count_status, invalid_argument_status, reset_device_last_error,
    runtime_result_status,
};
use crate::domain_bridge::decode::OutputBytes;
use crate::domain_bridge::decode::{
    InputBytes, ParityMarker, ParityMarkerMut, baud_rate_from_c, bool_property_from_c,
    decode_modulations, decode_optional_dep_info, decode_optional_target, dep_mode_from_c,
    modulation_from_c, timeout_property_from_c,
};
use crate::domain_bridge::encode::{CyclesInOut, TargetInOut, TargetOut, TargetSliceOut};
use crate::ffi_catch_unwind_int;
use crate::initiator::runtime;
use crate::lifecycle::nfc_device;
use libc::{c_int, size_t};
use proximate_driver as rt;

fn property_name(property: nfc_property) -> &'static str {
    match property {
        nfc_property::NP_TIMEOUT_COMMAND => "NP_TIMEOUT_COMMAND",
        nfc_property::NP_TIMEOUT_ATR => "NP_TIMEOUT_ATR",
        nfc_property::NP_TIMEOUT_COM => "NP_TIMEOUT_COM",
        nfc_property::NP_HANDLE_CRC => "NP_HANDLE_CRC",
        nfc_property::NP_HANDLE_PARITY => "NP_HANDLE_PARITY",
        nfc_property::NP_ACTIVATE_FIELD => "NP_ACTIVATE_FIELD",
        nfc_property::NP_ACTIVATE_CRYPTO1 => "NP_ACTIVATE_CRYPTO1",
        nfc_property::NP_INFINITE_SELECT => "NP_INFINITE_SELECT",
        nfc_property::NP_ACCEPT_INVALID_FRAMES => "NP_ACCEPT_INVALID_FRAMES",
        nfc_property::NP_ACCEPT_MULTIPLE_FRAMES => "NP_ACCEPT_MULTIPLE_FRAMES",
        nfc_property::NP_AUTO_ISO14443_4 => "NP_AUTO_ISO14443_4",
        nfc_property::NP_EASY_FRAMING => "NP_EASY_FRAMING",
        nfc_property::NP_FORCE_ISO14443_A => "NP_FORCE_ISO14443_A",
        nfc_property::NP_FORCE_ISO14443_B => "NP_FORCE_ISO14443_B",
        nfc_property::NP_FORCE_SPEED_106 => "NP_FORCE_SPEED_106",
        _ => "UNKNOWN_PROPERTY",
    }
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
        let timeout = rt::OperationTimeout::from_configured_millis(value);
        let result = timeout_property_from_c(property)
            .and_then(|property| timeout.map(|timeout| (property, timeout)))
            .and_then(|(property, timeout)| runtime::set_timeout(device, property, timeout));
        match result {
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
        let result = bool_property_from_c(property)
            .and_then(|property| runtime::set_property_bool(device, property, enable));
        match result {
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
            let modulation = match modulation_from_c(nm) {
                Ok(value) => value,
                Err(_) => return invalid_argument_status(device),
            };

            match runtime::select_passive_target(device, modulation, payload.as_optional()) {
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
        let modulation = match modulation_from_c(nm) {
            Ok(value) => value,
            Err(_) => return invalid_argument_status(device),
        };

        match runtime::list_passive_targets(device, modulation, targets_len) {
            Ok(runtime_targets) => {
                let status = bounded_count_status(device, runtime_targets.len(), targets_len);
                if status >= 0 {
                    targets.write_back(&runtime_targets);
                }
                status
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
        let initiator_info = match decode_optional_dep_info(initiator) {
            Ok(value) => value,
            Err(_) => return invalid_argument_status(device),
        };
        let mode = match dep_mode_from_c(ndm) {
            Ok(value) => value,
            Err(_) => return invalid_argument_status(device),
        };
        let baud_rate = match baud_rate_from_c(nbr) {
            Ok(value) => value,
            Err(_) => return invalid_argument_status(device),
        };
        let target = TargetOut::from_raw(target);

        match runtime::select_dep_target(device, mode, baud_rate, initiator_info.as_ref(), timeout)
        {
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
        let initiator_info = match decode_optional_dep_info(initiator) {
            Ok(value) => value,
            Err(_) => return invalid_argument_status(device),
        };
        let mode = match dep_mode_from_c(ndm) {
            Ok(value) => value,
            Err(_) => return invalid_argument_status(device),
        };
        let baud_rate = match baud_rate_from_c(nbr) {
            Ok(value) => value,
            Err(_) => return invalid_argument_status(device),
        };
        let target = TargetOut::from_raw(target);

        match runtime::poll_dep_target(device, mode, baud_rate, initiator_info.as_ref(), timeout) {
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
        let runtime_target = match decode_optional_target(target) {
            Ok(value) => value,
            Err(_) => return invalid_argument_status(device),
        };
        match runtime::target_is_present(device, runtime_target.as_ref()) {
            Ok(true) => {
                reset_device_last_error(device);
                0
            }
            Ok(false) => runtime_result_status(
                device,
                &proximate_driver::Error::TargetReleased("target_is_present"),
                false,
            ),
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
                let status = bounded_count_status(device, count, rx_len);
                if status >= 0 {
                    target.write_back();
                }
                status
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
            Ok(count) => bounded_count_status(device, count, rx_len),
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
            Ok(count) => bounded_count_status(device, count, rx_len.saturating_mul(8)),
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
            let cycles = CyclesInOut::from_raw(cycles);
            match runtime::transceive_bytes_timed(
                device,
                tx.as_slice(),
                rx.as_mut_slice(),
                cycles.initial(),
            ) {
                Ok((count, measured_cycles)) => {
                    let status = bounded_count_status(device, count, rx_len);
                    if status >= 0 {
                        cycles.write_back(measured_cycles);
                    }
                    status
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
            let cycles = CyclesInOut::from_raw(cycles);
            match runtime::transceive_bits_timed(
                device,
                tx.as_slice(),
                tx_bits_len,
                tx_parity.as_deref(),
                rx.as_mut_slice(),
                rx_parity.as_deref_mut(),
                cycles.initial(),
            ) {
                Ok((count, measured_cycles)) => {
                    let status = bounded_count_status(device, count, rx_len.saturating_mul(8));
                    if status >= 0 {
                        cycles.write_back(measured_cycles);
                    }
                    status
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
            Ok(count) => bounded_count_status(device, count, tx_len),
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
            Ok(count) => bounded_count_status(device, count, rx_len),
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
            Ok(count) => bounded_count_status(device, count, tx_bits_len),
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
            Ok(count) => bounded_count_status(device, count, rx_len.saturating_mul(8)),
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub(crate) unsafe fn nfc_abort_command(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_abort_command", NFC_ESOFT, || {
        match runtime::abort(device) {
            Ok(()) => 0,
            Err(error) => runtime_result_status(device, &error, true),
        }
    })
}

pub(crate) unsafe fn nfc_idle(device: *mut nfc_device) -> c_int {
    ffi_catch_unwind_int("nfc_idle", NFC_ESOFT, || match runtime::idle(device) {
        Ok(()) => 0,
        Err(error) => runtime_result_status(device, &error, true),
    })
}
