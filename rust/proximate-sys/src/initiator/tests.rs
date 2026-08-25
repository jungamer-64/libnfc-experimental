use super::accessors::*;
use super::operations::*;
use crate::c_abi::misc_exports::{nfc_close, nfc_free};
use crate::c_abi::types::*;
use crate::c_boundary::status::{NFC_EDEVNOTSUPP, NFC_EINVARG, NFC_EOVFLOW, NFC_ESOFT};
use crate::core::context::{nfc_exit, nfc_init};
use crate::core::driver_registration::nfc_register_driver;
use crate::core::runtime::nfc_open;
use crate::test_support::{
    external_driver, external_state_snapshot, external_test_guard, fake_abi_device,
    reset_external_state,
};
use std::ffi::{CStr, CString};
use std::ptr;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

fn iso14443a() -> nfc_modulation {
    nfc_modulation {
        nmt: nfc_modulation_type::NMT_ISO14443A,
        nbr: nfc_baud_rate::NBR_106,
    }
}

fn open_external(
    mut driver: crate::lifecycle::nfc_driver,
) -> (*mut crate::nfc_context, *mut crate::nfc_device) {
    assert_eq!(unsafe { nfc_register_driver(ptr::addr_of!(driver)) }, 0);
    // Prove that registration owns the callback-table snapshot.
    driver.open = None;
    assert!(driver.open.is_none());
    let mut context = ptr::null_mut();
    unsafe { nfc_init(ptr::addr_of_mut!(context)) };
    let connstring = CString::new("fakec:one").unwrap();
    let device = unsafe { nfc_open(context, connstring.as_ptr()) };
    assert!(!device.is_null());
    (context, device)
}

#[test]
fn device_name_pointer_is_stable_until_close() {
    let (device, _state) = fake_abi_device();
    let first = unsafe { nfc_device_get_name(device) };
    let second = unsafe { nfc_device_get_name(device) };
    assert_eq!(first, second);
    assert_eq!(unsafe { CStr::from_ptr(first) }, c"fake device");
    unsafe { nfc_close(device) };
}

#[test]
fn device_connstring_pointer_is_stable_until_close() {
    let (device, _state) = fake_abi_device();
    let first = unsafe { nfc_device_get_connstring(device) };
    let second = unsafe { nfc_device_get_connstring(device) };
    assert_eq!(first, second);
    assert_eq!(unsafe { CStr::from_ptr(first) }, c"fake:device");
    unsafe { nfc_close(device) };
}

#[test]
fn modulation_capability_pointer_is_immutable_per_query_key() {
    let (device, _state) = fake_abi_device();
    let mut first = ptr::null();
    let mut second = ptr::null();
    assert_eq!(
        unsafe { nfc_device_get_supported_modulation(device, nfc_mode::N_INITIATOR, &mut first) },
        0
    );
    assert_eq!(
        unsafe { nfc_device_get_supported_modulation(device, nfc_mode::N_INITIATOR, &mut second) },
        0
    );
    assert_eq!(first, second);
    assert_eq!(unsafe { *first }, nfc_modulation_type::NMT_ISO14443A);
    unsafe { nfc_close(device) };
}

#[test]
fn modulation_cache_separates_mode_keys() {
    let (device, _state) = fake_abi_device();
    let mut initiator = ptr::null();
    let mut target = ptr::null();
    unsafe {
        assert_eq!(
            nfc_device_get_supported_modulation(device, nfc_mode::N_INITIATOR, &mut initiator),
            0
        );
        assert_eq!(
            nfc_device_get_supported_modulation(device, nfc_mode::N_TARGET, &mut target),
            0
        );
    }
    assert_ne!(initiator, target);
    unsafe { nfc_close(device) };
}

#[test]
fn baud_rate_capability_pointer_is_immutable_per_query_key() {
    let (device, _state) = fake_abi_device();
    let mut first = ptr::null();
    let mut second = ptr::null();
    unsafe {
        assert_eq!(
            nfc_device_get_supported_baud_rate(
                device,
                nfc_modulation_type::NMT_ISO14443A,
                &mut first
            ),
            0
        );
        assert_eq!(
            nfc_device_get_supported_baud_rate(
                device,
                nfc_modulation_type::NMT_ISO14443A,
                &mut second
            ),
            0
        );
    }
    assert_eq!(first, second);
    assert_eq!(unsafe { *first }, nfc_baud_rate::NBR_106);
    unsafe { nfc_close(device) };
}

#[test]
fn invalid_mode_is_rejected_at_boundary() {
    let (device, _state) = fake_abi_device();
    let mut output = ptr::null();
    assert_eq!(
        unsafe { nfc_device_get_supported_modulation(device, nfc_mode::from_raw(99), &mut output) },
        NFC_EINVARG
    );
    unsafe { nfc_close(device) };
}

#[test]
fn invalid_modulation_type_is_rejected_at_boundary() {
    let (device, _state) = fake_abi_device();
    let mut output = ptr::null();
    assert_eq!(
        unsafe {
            nfc_device_get_supported_baud_rate(
                device,
                nfc_modulation_type::from_raw(99),
                &mut output,
            )
        },
        NFC_EINVARG
    );
    unsafe { nfc_close(device) };
}

#[test]
fn invalid_boolean_property_is_rejected_at_boundary() {
    let (device, _state) = fake_abi_device();
    assert_eq!(
        unsafe { nfc_device_set_property_bool(device, nfc_property::from_raw(99), true) },
        NFC_EINVARG
    );
    unsafe { nfc_close(device) };
}

#[test]
fn invalid_timeout_property_is_rejected_at_boundary() {
    let (device, _state) = fake_abi_device();
    assert_eq!(
        unsafe { nfc_device_set_property_int(device, nfc_property::from_raw(99), 10) },
        NFC_EINVARG
    );
    unsafe { nfc_close(device) };
}

#[test]
fn null_device_is_reported_as_invalid_argument() {
    assert_eq!(unsafe { nfc_initiator_init(ptr::null_mut()) }, NFC_EINVARG);
}

#[test]
fn null_input_with_zero_length_is_valid() {
    let (device, _state) = fake_abi_device();
    let mut rx = [0u8; 1];
    assert_eq!(
        unsafe {
            nfc_initiator_transceive_bytes(device, ptr::null(), 0, rx.as_mut_ptr(), rx.len(), 0)
        },
        0
    );
    unsafe { nfc_close(device) };
}

#[test]
fn null_input_with_nonzero_length_is_rejected() {
    let (device, _state) = fake_abi_device();
    let mut rx = [0u8; 1];
    assert_eq!(
        unsafe {
            nfc_initiator_transceive_bytes(device, ptr::null(), 1, rx.as_mut_ptr(), rx.len(), 0)
        },
        NFC_EINVARG
    );
    unsafe { nfc_close(device) };
}

#[test]
fn null_output_with_nonzero_length_is_rejected() {
    let (device, _state) = fake_abi_device();
    assert_eq!(
        unsafe { nfc_initiator_transceive_bytes(device, [1u8].as_ptr(), 1, ptr::null_mut(), 1, 0) },
        NFC_EINVARG
    );
    unsafe { nfc_close(device) };
}

#[test]
fn passive_target_is_projected_back_to_c() {
    let (device, _state) = fake_abi_device();
    let mut target = unsafe { std::mem::zeroed::<nfc_target>() };
    assert_eq!(
        unsafe {
            nfc_initiator_select_passive_target(device, iso14443a(), ptr::null(), 0, &mut target)
        },
        1
    );
    let modulation = unsafe { ptr::addr_of!(target.nm).read_unaligned() };
    let modulation_type = unsafe { ptr::addr_of!(modulation.nmt).read_unaligned() };
    assert_eq!(modulation_type, nfc_modulation_type::NMT_ISO14443A);
    unsafe { nfc_close(device) };
}

#[test]
fn information_allocation_is_released_with_nfc_free() {
    let (device, _state) = fake_abi_device();
    let mut output = ptr::null_mut();
    assert!(unsafe { nfc_device_get_information_about(device, &mut output) } > 0);
    assert_eq!(
        unsafe { CStr::from_ptr(output) },
        c"fake device information"
    );
    unsafe {
        nfc_free(output.cast());
        nfc_close(device);
    }
}

#[test]
fn backend_panic_poison_closes_future_operations_with_soft_error() {
    let (device, state) = fake_abi_device();
    state.panic_next.store(true, Ordering::SeqCst);
    assert_eq!(unsafe { nfc_initiator_init(device) }, NFC_ESOFT);
    assert_eq!(unsafe { nfc_initiator_init(device) }, NFC_ESOFT);
    assert_eq!(unsafe { nfc_device_get_last_error(device) }, NFC_ESOFT);
    unsafe { nfc_close(device) };
}

#[test]
fn ordinary_operations_are_serialized_by_device_mutex() {
    let (device, state) = fake_abi_device();
    let address = device as usize;
    let first = std::thread::spawn(move || {
        let mut rx = [0u8; 1];
        unsafe {
            nfc_initiator_transceive_bytes(
                address as *mut _,
                [1u8].as_ptr(),
                1,
                rx.as_mut_ptr(),
                1,
                0,
            )
        }
    });
    let address = device as usize;
    let second = std::thread::spawn(move || {
        let mut rx = [0u8; 1];
        unsafe {
            nfc_initiator_transceive_bytes(
                address as *mut _,
                [2u8].as_ptr(),
                1,
                rx.as_mut_ptr(),
                1,
                0,
            )
        }
    });
    assert_eq!(first.join().unwrap(), 1);
    assert_eq!(second.join().unwrap(), 1);
    assert_eq!(state.max_active.load(Ordering::SeqCst), 1);
    unsafe { nfc_close(device) };
}

#[test]
fn abort_authority_remains_available_during_blocking_operation() {
    let (device, state) = fake_abi_device();
    state.block.store(true, Ordering::SeqCst);
    let address = device as usize;
    let worker = std::thread::spawn(move || {
        let mut rx = [0u8; 1];
        unsafe {
            nfc_initiator_transceive_bytes(
                address as *mut _,
                [7u8].as_ptr(),
                1,
                rx.as_mut_ptr(),
                1,
                0,
            )
        }
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    while state.active.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(unsafe { nfc_abort_command(device) }, 0);
    assert_eq!(worker.join().unwrap(), 1);
    assert_eq!(state.aborts.load(Ordering::SeqCst), 1);
    unsafe { nfc_close(device) };
}

#[test]
fn close_owns_backend_finalization_once() {
    let (device, state) = fake_abi_device();
    unsafe { nfc_close(device) };
    assert_eq!(state.closes.load(Ordering::SeqCst), 1);
}

#[test]
fn external_initiator_operation_uses_same_c_entrypoint() {
    let _guard = external_test_guard();
    reset_external_state();
    let (context, device) = open_external(external_driver(c"fakec"));
    assert_eq!(unsafe { nfc_initiator_init(device) }, 0);
    assert!(
        external_state_snapshot()
            .operations
            .contains(&"initiator_init")
    );
    unsafe {
        nfc_close(device);
        nfc_exit(context);
    }
}

#[test]
fn missing_external_callback_maps_to_device_not_supported() {
    let _guard = external_test_guard();
    reset_external_state();
    let mut driver = external_driver(c"fakec");
    driver.initiator_init = None;
    let (context, device) = open_external(driver);
    assert_eq!(unsafe { nfc_initiator_init(device) }, 0);
    assert_eq!(
        unsafe { nfc_device_get_last_error(device) },
        NFC_EDEVNOTSUPP
    );
    unsafe {
        nfc_close(device);
        nfc_exit(context);
    }
}

#[test]
fn external_capability_array_is_cached_by_abi_device() {
    let _guard = external_test_guard();
    reset_external_state();
    let (context, device) = open_external(external_driver(c"fakec"));
    let mut first = ptr::null();
    let mut second = ptr::null();
    unsafe {
        assert_eq!(
            nfc_device_get_supported_modulation(device, nfc_mode::N_INITIATOR, &mut first),
            0
        );
        assert_eq!(
            nfc_device_get_supported_modulation(device, nfc_mode::N_INITIATOR, &mut second),
            0
        );
    }
    assert_eq!(first, second);
    unsafe {
        nfc_close(device);
        nfc_exit(context);
    }
}

#[test]
fn external_transceive_and_target_families_share_device_path() {
    let _guard = external_test_guard();
    reset_external_state();
    let (context, device) = open_external(external_driver(c"fakec"));
    let tx = [1u8, 2, 3];
    let mut rx = [0u8; 3];
    assert_eq!(
        unsafe {
            nfc_initiator_transceive_bytes(
                device,
                tx.as_ptr(),
                tx.len(),
                rx.as_mut_ptr(),
                rx.len(),
                0,
            )
        },
        3
    );
    assert_eq!(rx, tx);
    assert_eq!(
        unsafe { nfc_target_send_bytes(device, tx.as_ptr(), tx.len(), 0) },
        3
    );
    unsafe {
        nfc_close(device);
        nfc_exit(context);
    }
}

#[test]
fn external_abort_uses_mutex_independent_capability() {
    let _guard = external_test_guard();
    reset_external_state();
    let (context, device) = open_external(external_driver(c"fakec"));
    assert_eq!(unsafe { nfc_abort_command(device) }, 0);
    assert_eq!(external_state_snapshot().aborts, 1);
    unsafe {
        nfc_close(device);
        nfc_exit(context);
    }
}

#[test]
fn external_close_callback_runs_once_at_abi_close() {
    let _guard = external_test_guard();
    reset_external_state();
    let (context, device) = open_external(external_driver(c"fakec"));
    unsafe { nfc_close(device) };
    assert_eq!(external_state_snapshot().closes, 1);
    unsafe { nfc_exit(context) };
}

#[test]
fn successful_operation_resets_previous_last_error() {
    let (device, _state) = fake_abi_device();
    assert_eq!(
        unsafe { nfc_device_set_property_bool(device, nfc_property::from_raw(99), true) },
        NFC_EINVARG
    );
    assert_eq!(unsafe { nfc_initiator_init(device) }, 0);
    assert_eq!(unsafe { nfc_device_get_last_error(device) }, 0);
    unsafe { nfc_close(device) };
}

#[test]
fn null_capability_output_is_rejected() {
    let (device, _state) = fake_abi_device();
    assert_eq!(
        unsafe {
            nfc_device_get_supported_modulation(device, nfc_mode::N_INITIATOR, ptr::null_mut())
        },
        NFC_EINVARG
    );
    unsafe { nfc_close(device) };
}

#[test]
fn null_information_output_is_rejected() {
    let (device, _state) = fake_abi_device();
    assert_eq!(
        unsafe { nfc_device_get_information_about(device, ptr::null_mut()) },
        NFC_EINVARG
    );
    unsafe { nfc_close(device) };
}

#[test]
fn huge_raw_lengths_are_rejected_before_slice_construction() {
    let (device, _state) = fake_abi_device();
    let dangling = std::ptr::NonNull::<u8>::dangling().as_ptr();
    let mut rx = [0u8; 1];
    assert_eq!(
        unsafe {
            nfc_initiator_transceive_bytes(device, dangling, usize::MAX, rx.as_mut_ptr(), 1, 0)
        },
        NFC_EINVARG
    );
    let tx = [0u8; 1];
    assert_eq!(
        unsafe { nfc_initiator_transceive_bytes(device, tx.as_ptr(), 1, dangling, usize::MAX, 0) },
        NFC_EINVARG
    );
    unsafe { nfc_close(device) };
}

#[test]
fn invalid_dep_mode_is_rejected_at_boundary() {
    let (device, _state) = fake_abi_device();
    let mut target = unsafe { std::mem::zeroed::<nfc_target>() };
    assert_eq!(
        unsafe {
            nfc_initiator_select_dep_target(
                device,
                nfc_dep_mode::from_raw(99),
                nfc_baud_rate::NBR_106,
                ptr::null(),
                &mut target,
                0,
            )
        },
        NFC_EINVARG
    );
    unsafe { nfc_close(device) };
}

#[test]
fn invalid_dep_baud_rate_is_rejected_at_boundary() {
    let (device, _state) = fake_abi_device();
    let mut target = unsafe { std::mem::zeroed::<nfc_target>() };
    assert_eq!(
        unsafe {
            nfc_initiator_select_dep_target(
                device,
                nfc_dep_mode::NDM_PASSIVE,
                nfc_baud_rate::from_raw(99),
                ptr::null(),
                &mut target,
                0,
            )
        },
        NFC_EINVARG
    );
    unsafe { nfc_close(device) };
}

#[test]
fn external_property_family_uses_unified_device_path() {
    let _guard = external_test_guard();
    reset_external_state();
    let (context, device) = open_external(external_driver(c"fakec"));
    assert_eq!(
        unsafe { nfc_device_set_property_bool(device, nfc_property::NP_EASY_FRAMING, false) },
        0
    );
    assert!(
        external_state_snapshot()
            .operations
            .contains(&"property_bool")
    );
    unsafe {
        nfc_close(device);
        nfc_exit(context)
    };
}

#[test]
fn external_timeout_family_uses_unified_device_path() {
    let _guard = external_test_guard();
    reset_external_state();
    let (context, device) = open_external(external_driver(c"fakec"));
    assert_eq!(
        unsafe { nfc_device_set_property_int(device, nfc_property::NP_TIMEOUT_COMMAND, 25) },
        0
    );
    assert!(
        external_state_snapshot()
            .operations
            .contains(&"property_int")
    );
    unsafe {
        nfc_close(device);
        nfc_exit(context)
    };
}

#[test]
fn external_timed_transceive_projects_cycle_count() {
    let _guard = external_test_guard();
    reset_external_state();
    let (context, device) = open_external(external_driver(c"fakec"));
    let tx = [9u8, 8];
    let mut rx = [0u8; 2];
    let mut cycles = 100u32;
    assert_eq!(
        unsafe {
            nfc_initiator_transceive_bytes_timed(
                device,
                tx.as_ptr(),
                2,
                rx.as_mut_ptr(),
                2,
                &mut cycles,
            )
        },
        2
    );
    assert_eq!(cycles, 42);
    unsafe {
        nfc_close(device);
        nfc_exit(context)
    };
}

unsafe extern "C" fn external_oversized_receive(
    _device: *mut crate::nfc_device,
    _rx: *mut u8,
    rx_len: usize,
    _timeout: libc::c_int,
) -> libc::c_int {
    libc::c_int::try_from(rx_len.saturating_add(1)).unwrap_or(libc::c_int::MAX)
}

#[test]
fn external_target_receive_projects_bytes_and_rejects_oversized_count() {
    let _guard = external_test_guard();
    reset_external_state();
    let (context, device) = open_external(external_driver(c"fakec"));
    let mut rx = [0u8; 2];
    assert_eq!(
        unsafe { nfc_target_receive_bytes(device, rx.as_mut_ptr(), rx.len(), 0) },
        1
    );
    assert_eq!(rx[0], 0xa5);
    unsafe {
        nfc_close(device);
        nfc_exit(context)
    };

    reset_external_state();
    let mut driver = external_driver(c"fakec");
    driver.target_receive_bytes = Some(external_oversized_receive);
    let (context, device) = open_external(driver);
    let mut rx = [0u8; 1];
    assert_eq!(
        unsafe { nfc_target_receive_bytes(device, rx.as_mut_ptr(), rx.len(), 0) },
        NFC_EOVFLOW
    );
    assert_eq!(unsafe { nfc_device_get_last_error(device) }, NFC_EOVFLOW);
    unsafe {
        nfc_close(device);
        nfc_exit(context)
    };
}

#[test]
fn external_information_is_reallocated_for_nfc_free() {
    let _guard = external_test_guard();
    reset_external_state();
    let (context, device) = open_external(external_driver(c"fakec"));
    let mut output = ptr::null_mut();
    assert!(unsafe { nfc_device_get_information_about(device, &mut output) } > 0);
    assert_eq!(unsafe { CStr::from_ptr(output) }, c"external information");
    unsafe {
        nfc_free(output.cast());
        nfc_close(device);
        nfc_exit(context)
    };
}

static UNTERMINATED_MODULATIONS: [nfc_modulation_type; 64] =
    [nfc_modulation_type::NMT_ISO14443A; 64];

unsafe extern "C" fn unterminated_modulations(
    _device: *mut crate::nfc_device,
    _mode: nfc_mode,
    output: *mut *const nfc_modulation_type,
) -> libc::c_int {
    unsafe { *output = UNTERMINATED_MODULATIONS.as_ptr() };
    0
}

#[test]
fn unterminated_external_modulation_array_is_rejected() {
    let _guard = external_test_guard();
    reset_external_state();
    let mut driver = external_driver(c"fakec");
    driver.get_supported_modulation = Some(unterminated_modulations);
    let (context, device) = open_external(driver);
    let mut output = ptr::null();
    assert_eq!(
        unsafe { nfc_device_get_supported_modulation(device, nfc_mode::N_INITIATOR, &mut output) },
        NFC_EINVARG
    );
    unsafe {
        nfc_close(device);
        nfc_exit(context)
    };
}

static UNTERMINATED_BAUD_RATES: [nfc_baud_rate; 64] = [nfc_baud_rate::NBR_106; 64];

unsafe extern "C" fn unterminated_baud_rates(
    _device: *mut crate::nfc_device,
    _mode: nfc_mode,
    _modulation: nfc_modulation_type,
    output: *mut *const nfc_baud_rate,
) -> libc::c_int {
    unsafe { *output = UNTERMINATED_BAUD_RATES.as_ptr() };
    0
}

#[test]
fn unterminated_external_baud_rate_array_is_rejected() {
    let _guard = external_test_guard();
    reset_external_state();
    let mut driver = external_driver(c"fakec");
    driver.get_supported_baud_rate = Some(unterminated_baud_rates);
    let (context, device) = open_external(driver);
    let mut output = ptr::null();
    assert_eq!(
        unsafe {
            nfc_device_get_supported_baud_rate(
                device,
                nfc_modulation_type::NMT_ISO14443A,
                &mut output,
            )
        },
        NFC_EINVARG
    );
    unsafe {
        nfc_close(device);
        nfc_exit(context)
    };
}
