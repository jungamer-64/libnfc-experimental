use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use proximate_driver::{
    BaudRate, ConnectionString, Context, Driver, Error, Mode, Modulation, ModulationType,
    OperationTimeout, PollIterations, PollPeriod, Target, TargetInfo,
};

use super::Pn71xxDriver;
use super::consts::{
    DESFIRE_ATS, NFA_PROTOCOL_T1T, NFC_EINVARG, TARGET_TYPE_FELICA, TARGET_TYPE_ISO14443_3A,
    TARGET_TYPE_ISO14443_3B, TARGET_TYPE_MIFARE_CLASSIC,
};
use super::fake::{
    backend_state_snapshot, emit_tag_arrival_for_tests, emit_tag_departure_for_tests,
    reset_test_world, with_backend_state_mut,
};
use super::runtime::{Pn71xxSession, current_tag_snapshot, runtime_snapshot};
use crate::nci::TagInfo;

fn modulation(modulation_type: ModulationType, baud_rate: BaudRate) -> Modulation {
    Modulation::try_new(modulation_type, baud_rate).unwrap()
}

trait TestDeviceOps {
    fn select_passive_target(
        &mut self,
        modulation: Modulation,
        init_data: Option<&[u8]>,
    ) -> Result<Option<Target>, Error>;

    fn poll_target(
        &mut self,
        modulations: &[Modulation],
        poll_nr: u8,
        period: u8,
    ) -> Result<Option<Target>, Error>;

    fn transceive_bytes(&mut self, tx: &[u8], rx: &mut [u8], timeout: i32) -> Result<usize, Error>;

    fn target_is_present(&mut self, target: Option<&Target>) -> Result<bool, Error>;
}

impl TestDeviceOps for proximate_driver::Device {
    fn select_passive_target(
        &mut self,
        modulation: Modulation,
        init_data: Option<&[u8]>,
    ) -> Result<Option<Target>, Error> {
        let mut passive_scan = self.passive_scan_ops()?;
        passive_scan.select_passive_target(modulation, init_data)
    }

    fn poll_target(
        &mut self,
        modulations: &[Modulation],
        poll_nr: u8,
        period: u8,
    ) -> Result<Option<Target>, Error> {
        let mut passive_scan = self.passive_scan_ops()?;
        passive_scan.poll_target(
            modulations,
            PollIterations::from_libnfc(poll_nr)?,
            PollPeriod::try_new(period)?,
        )
    }

    fn transceive_bytes(&mut self, tx: &[u8], rx: &mut [u8], timeout: i32) -> Result<usize, Error> {
        let mut initiator_io = self.initiator_io_ops()?;
        initiator_io.transceive_bytes(tx, rx, OperationTimeout::from_libnfc_millis(timeout)?)
    }

    fn target_is_present(&mut self, target: Option<&Target>) -> Result<bool, Error> {
        let mut session = self.session_ops()?;
        session.target_is_present(target)
    }
}

fn test_guard() -> &'static Mutex<()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(()))
}

fn make_tag(technology: u32, uid: &[u8], protocol: u8) -> TagInfo {
    let mut tag = TagInfo {
        technology,
        handle: 0x1234,
        protocol,
        ..Default::default()
    };
    let copy_len = uid.len().min(tag.uid.len());
    tag.uid[..copy_len].copy_from_slice(&uid[..copy_len]);
    tag.uid_length = copy_len as u32;
    tag
}

fn open_device(connstring: &ConnectionString) -> proximate_driver::Device {
    let driver = Pn71xxDriver::new();
    proximate_driver::Device::from_backend(driver.open(&Context::new(), connstring).unwrap())
}

#[test]
fn scan_reports_success_and_failure() {
    let _guard = test_guard().lock().unwrap();
    reset_test_world();

    let driver = Pn71xxDriver::new();
    let found = driver.scan(&Context::new()).unwrap();
    assert_eq!(
        found
            .into_iter()
            .map(|device| device.connstring)
            .collect::<Vec<_>>(),
        vec![ConnectionString::new("pn71xx").unwrap()]
    );
    let snapshot = backend_state_snapshot();
    assert_eq!(snapshot.initialize_calls, 1);
    assert_eq!(snapshot.deinitialize_calls, 1);

    reset_test_world();
    with_backend_state_mut(|state| state.init_result = -1);
    let found = driver.scan(&Context::new()).unwrap();
    assert!(found.is_empty());
    let snapshot = backend_state_snapshot();
    assert_eq!(snapshot.initialize_calls, 1);
    assert_eq!(snapshot.deinitialize_calls, 0);
}

#[test]
fn open_works_without_prior_scan() {
    let _guard = test_guard().lock().unwrap();
    reset_test_world();

    let connstring = ConnectionString::new("pn71xx").unwrap();
    let device = open_device(&connstring);
    assert_eq!(device.name(), "pn71xx-device");
    let snapshot = backend_state_snapshot();
    assert_eq!(snapshot.initialize_calls, 1);
    assert_eq!(snapshot.register_calls, 1);
    assert_eq!(snapshot.enable_calls, 1);

    drop(device);
}

#[test]
fn second_concurrent_open_is_rejected() {
    let _guard = test_guard().lock().unwrap();
    reset_test_world();

    let connstring = ConnectionString::new("pn71xx").unwrap();
    let first = open_device(&connstring);
    let driver = Pn71xxDriver::new();
    let error = match driver.open(&Context::new(), &connstring) {
        Ok(_) => panic!("second open should be rejected"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::DriverOpenFailed(_)));

    drop(first);
}

#[test]
fn close_tears_down_runtime_and_backend() {
    let _guard = test_guard().lock().unwrap();
    reset_test_world();

    let connstring = ConnectionString::new("pn71xx").unwrap();
    let device = open_device(&connstring);
    emit_tag_arrival_for_tests(make_tag(TARGET_TYPE_ISO14443_3A, &[0x11, 0x22], 0));

    drop(device);

    let runtime = runtime_snapshot();
    assert_eq!(runtime.session, Pn71xxSession::Inactive);

    let backend = backend_state_snapshot();
    assert_eq!(backend.disable_calls, 1);
    assert_eq!(backend.deregister_calls, 1);
    assert_eq!(backend.deinitialize_calls, 1);
    assert!(backend.current_tag.is_none());
}

#[test]
fn callbacks_update_cached_tag_state() {
    let _guard = test_guard().lock().unwrap();
    reset_test_world();

    let connstring = ConnectionString::new("pn71xx").unwrap();
    let device = open_device(&connstring);

    emit_tag_arrival_for_tests(make_tag(TARGET_TYPE_ISO14443_3A, &[0x44], 0));
    assert!(current_tag_snapshot().is_some());

    emit_tag_departure_for_tests();
    assert!(current_tag_snapshot().is_none());

    drop(device);
}

#[test]
fn select_passive_target_maps_supported_technology_families() {
    let _guard = test_guard().lock().unwrap();
    reset_test_world();

    let connstring = ConnectionString::new("pn71xx").unwrap();
    let mut device = open_device(&connstring);

    let cases = [
        (
            make_tag(TARGET_TYPE_MIFARE_CLASSIC, &[0x01, 0x02, 0x03, 0x04], 0),
            modulation(ModulationType::Iso14443A, BaudRate::Br106),
        ),
        (
            make_tag(TARGET_TYPE_ISO14443_3A, &[0x10, 0x11, 0x12, 0x13], 0),
            modulation(ModulationType::Iso14443A, BaudRate::Br106),
        ),
        (
            make_tag(TARGET_TYPE_ISO14443_3B, &[0x21, 0x22, 0x23, 0x24], 0),
            modulation(ModulationType::Iso14443B, BaudRate::Br106),
        ),
        (
            make_tag(TARGET_TYPE_ISO14443_3B, &[0x31, 0x32, 0x33, 0x34], 0),
            modulation(ModulationType::Iso14443Bi, BaudRate::Br106),
        ),
        (
            make_tag(
                TARGET_TYPE_ISO14443_3B,
                &[0x41, 0x42, 0x43, 0x44, 0x45, 0x46],
                0,
            ),
            modulation(ModulationType::Iso14443B2Sr, BaudRate::Br106),
        ),
        (
            make_tag(TARGET_TYPE_ISO14443_3B, &[0x51, 0x52, 0x53, 0x54], 0),
            modulation(ModulationType::Iso14443B2Ct, BaudRate::Br106),
        ),
        (
            make_tag(
                TARGET_TYPE_FELICA,
                &[0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68],
                0,
            ),
            modulation(ModulationType::Felica, BaudRate::Br212),
        ),
        (
            make_tag(
                TARGET_TYPE_ISO14443_3A,
                &[0x71, 0x72, 0x73, 0x74],
                NFA_PROTOCOL_T1T,
            ),
            modulation(ModulationType::Jewel, BaudRate::Br106),
        ),
    ];

    for (tag, modulation) in cases {
        emit_tag_arrival_for_tests(tag);
        let target = device
            .select_passive_target(modulation, None)
            .unwrap()
            .expect("target should be present");

        match target.info() {
            TargetInfo::Iso14443A { uid, sak, ats, .. } => {
                assert_eq!(uid.as_slice(), &tag.uid[..tag.uid_length as usize]);
                if tag.technology == TARGET_TYPE_MIFARE_CLASSIC {
                    assert_eq!(*sak, 0x08);
                    assert!(ats.is_empty());
                } else {
                    assert_eq!(*sak, 0x20);
                    assert_eq!(ats.as_slice(), DESFIRE_ATS);
                }
            }
            TargetInfo::Iso14443B { pupi, .. } => assert_eq!(*pupi, [0x21, 0x22, 0x23, 0x24]),
            TargetInfo::Iso14443Bi { div, .. } => assert_eq!(*div, [0x31, 0x32, 0x33, 0x34]),
            TargetInfo::Iso14443B2Sr { uid } => {
                assert_eq!(&uid[..6], &[0x41, 0x42, 0x43, 0x44, 0x45, 0x46])
            }
            TargetInfo::Iso14443B2Ct { uid, .. } => assert_eq!(*uid, [0x51, 0x52, 0x53, 0x54]),
            TargetInfo::Felica { id, .. } => {
                assert_eq!(*id, [0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68])
            }
            TargetInfo::Jewel { id, .. } => assert_eq!(*id, [0x71, 0x72, 0x73, 0x74]),
            _ => panic!("unexpected target kind"),
        }
    }
}

#[test]
fn poll_target_retries_until_tag_appears() {
    let _guard = test_guard().lock().unwrap();
    reset_test_world();

    let connstring = ConnectionString::new("pn71xx").unwrap();
    let mut device = open_device(&connstring);
    let worker = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(10));
        emit_tag_arrival_for_tests(make_tag(TARGET_TYPE_ISO14443_3A, &[0xAA, 0xBB], 0));
    });

    let modulations = [modulation(ModulationType::Iso14443A, BaudRate::Br106)];
    let target = device
        .poll_target(&modulations, 2, 1)
        .unwrap()
        .expect("target should appear");
    worker.join().unwrap();
    match target.info() {
        TargetInfo::Iso14443A { uid, .. } => assert_eq!(uid.as_slice(), &[0xAA, 0xBB]),
        _ => panic!("unexpected target kind"),
    }
}

#[test]
fn transceive_bytes_handles_missing_and_present_tags() {
    let _guard = test_guard().lock().unwrap();
    reset_test_world();

    let connstring = ConnectionString::new("pn71xx").unwrap();
    let mut device = open_device(&connstring);
    let tx = [0x30u8, 0x04];
    let mut rx = [0u8; 8];

    let missing = device.transceive_bytes(&tx, &mut rx, 250).unwrap_err();
    assert_eq!(missing.device_code(), Some(NFC_EINVARG));

    emit_tag_arrival_for_tests(make_tag(TARGET_TYPE_ISO14443_3A, &[0x01], 0));
    with_backend_state_mut(|state| {
        state.transceive_result = 4;
        state.transceive_response = vec![0xDE, 0xAD, 0xBE, 0xEF];
    });

    let received = device.transceive_bytes(&tx, &mut rx, 250).unwrap();
    assert_eq!(received, 4);
    assert_eq!(&rx[..4], &[0xDE, 0xAD, 0xBE, 0xEF]);

    let state = backend_state_snapshot();
    assert_eq!(state.last_transceive_handle, Some(0x1234));
    assert_eq!(state.last_transceive_tx, tx);
    assert_eq!(state.last_transceive_timeout, Some(250));
}

#[test]
fn target_is_present_follows_tag_cache() {
    let _guard = test_guard().lock().unwrap();
    reset_test_world();

    let connstring = ConnectionString::new("pn71xx").unwrap();
    let mut device = open_device(&connstring);
    assert!(!device.target_is_present(None).unwrap());

    emit_tag_arrival_for_tests(make_tag(TARGET_TYPE_ISO14443_3A, &[0x01], 0));
    assert!(device.target_is_present(None).unwrap());

    emit_tag_departure_for_tests();
    assert!(!device.target_is_present(None).unwrap());
}

#[test]
fn device_get_information_about_returns_expected_string() {
    let _guard = test_guard().lock().unwrap();
    reset_test_world();

    let connstring = ConnectionString::new("pn71xx").unwrap();
    let mut device = open_device(&connstring);
    assert_eq!(
        device.info_ops().unwrap().information_about().unwrap(),
        "PN71XX nfc driver using libnfc-nci userspace library"
    );
}

#[test]
fn field_property_and_deselect_drive_discovery_transitions() {
    let _guard = test_guard().lock().unwrap();
    reset_test_world();

    let connstring = ConnectionString::new("pn71xx").unwrap();
    let mut device = open_device(&connstring);
    emit_tag_arrival_for_tests(make_tag(TARGET_TYPE_ISO14443_3A, &[0x11], 0));

    device
        .property_ops()
        .unwrap()
        .set_property_bool(proximate_driver::Property::ActivateField, false)
        .unwrap();
    assert_eq!(
        runtime_snapshot().session,
        Pn71xxSession::Idle { device_id: 0 }
    );
    assert!(current_tag_snapshot().is_none());

    device
        .property_ops()
        .unwrap()
        .set_property_bool(proximate_driver::Property::ActivateField, true)
        .unwrap();
    assert_eq!(
        runtime_snapshot().session,
        Pn71xxSession::Discovering { device_id: 0 }
    );

    emit_tag_arrival_for_tests(make_tag(TARGET_TYPE_ISO14443_3A, &[0x22], 0));
    device.session_ops().unwrap().deselect_target().unwrap();
    assert!(current_tag_snapshot().is_none());

    let backend = backend_state_snapshot();
    assert_eq!(backend.disable_calls, 2);
    assert_eq!(backend.enable_calls, 3);
    assert_eq!(backend.last_discovery_args, Some((0x07, 1, 0, 1)));
}

#[test]
fn powerdown_releases_nci_and_initiator_init_reestablishes_it() {
    let _guard = test_guard().lock().unwrap();
    reset_test_world();

    let connstring = ConnectionString::new("pn71xx").unwrap();
    let mut device = open_device(&connstring);
    device.session_ops().unwrap().powerdown().unwrap();

    assert_eq!(
        runtime_snapshot().session,
        Pn71xxSession::PoweredDown { device_id: 0 }
    );
    let powered_down = backend_state_snapshot();
    assert_eq!(powered_down.disable_calls, 1);
    assert_eq!(powered_down.deregister_calls, 1);
    assert_eq!(powered_down.deinitialize_calls, 1);

    assert_eq!(device.passive_scan_ops().unwrap().init().unwrap(), 0);
    assert_eq!(
        runtime_snapshot().session,
        Pn71xxSession::Discovering { device_id: 0 }
    );
    let restored = backend_state_snapshot();
    assert_eq!(restored.initialize_calls, 2);
    assert_eq!(restored.register_calls, 2);
    assert_eq!(restored.enable_calls, 2);
}

#[test]
fn communication_timeout_is_applied_and_unimplemented_properties_are_rejected() {
    let _guard = test_guard().lock().unwrap();
    reset_test_world();

    let connstring = ConnectionString::new("pn71xx").unwrap();
    let mut device = open_device(&connstring);
    {
        let mut properties = device.property_ops().unwrap();
        properties
            .set_timeout(
                proximate_driver::TimeoutProperty::Communication,
                OperationTimeout::try_milliseconds(777).unwrap(),
            )
            .unwrap();
        assert_eq!(
            properties.set_timeout(
                proximate_driver::TimeoutProperty::Atr,
                OperationTimeout::try_milliseconds(12).unwrap(),
            ),
            Err(Error::UnsupportedOperation("pn71xx_timeout"))
        );
        assert_eq!(
            properties.set_property_bool(proximate_driver::Property::HandleCrc, false),
            Err(Error::UnsupportedOperation("pn71xx_property_bool"))
        );
        assert_eq!(
            properties.set_timeout(
                proximate_driver::TimeoutProperty::Communication,
                OperationTimeout::DEFAULT,
            ),
            Err(Error::InvalidArgument("timeout"))
        );
    }

    emit_tag_arrival_for_tests(make_tag(TARGET_TYPE_ISO14443_3A, &[0x33], 0));
    with_backend_state_mut(|state| {
        state.transceive_result = 1;
        state.transceive_response = vec![0x90];
    });
    let mut rx = [0; 1];
    assert_eq!(device.transceive_bytes(&[0x00], &mut rx, -1).unwrap(), 1);
    assert_eq!(backend_state_snapshot().last_transceive_timeout, Some(777));

    assert_eq!(device.transceive_bytes(&[0x00], &mut rx, 0).unwrap(), 1);
    assert_eq!(backend_state_snapshot().last_transceive_timeout, Some(0));

    assert_eq!(device.transceive_bytes(&[0x00], &mut rx, -1).unwrap(), 1);
    assert_eq!(backend_state_snapshot().last_transceive_timeout, Some(777));
}

#[test]
fn continuous_poll_is_aborted_and_leaves_discovery_idle() {
    let _guard = test_guard().lock().unwrap();
    reset_test_world();

    let connstring = ConnectionString::new("pn71xx").unwrap();
    let mut device = open_device(&connstring);
    let abort = device.command_abort_handle().unwrap();
    let worker = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(10));
        abort.abort()
    });

    let modulations = [modulation(ModulationType::Iso14443A, BaudRate::Br106)];
    assert_eq!(
        device.poll_target(&modulations, u8::MAX, 1),
        Err(Error::Aborted("pn71xx_poll_target"))
    );
    worker.join().unwrap().unwrap();
    assert_eq!(
        runtime_snapshot().session,
        Pn71xxSession::Idle { device_id: 0 }
    );
    assert_eq!(backend_state_snapshot().disable_calls, 1);
}

#[test]
fn command_abort_authority_is_revoked_when_device_is_closed() {
    let _guard = test_guard().lock().unwrap();
    reset_test_world();

    let connstring = ConnectionString::new("pn71xx").unwrap();
    let device = open_device(&connstring);
    let abort = device.command_abort_handle().unwrap();
    drop(device);

    assert_eq!(abort.abort(), Err(Error::TargetReleased("abort_command")));
}

#[test]
fn infinite_select_waits_for_a_tag_and_can_be_disabled() {
    let _guard = test_guard().lock().unwrap();
    reset_test_world();

    let connstring = ConnectionString::new("pn71xx").unwrap();
    let mut device = open_device(&connstring);
    device
        .property_ops()
        .unwrap()
        .set_property_bool(proximate_driver::Property::InfiniteSelect, true)
        .unwrap();
    let worker = std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(10));
        emit_tag_arrival_for_tests(make_tag(TARGET_TYPE_ISO14443_3A, &[0x44], 0));
    });

    let selected = device
        .select_passive_target(modulation(ModulationType::Iso14443A, BaudRate::Br106), None)
        .unwrap();
    worker.join().unwrap();
    assert!(selected.is_some());

    emit_tag_departure_for_tests();
    device
        .property_ops()
        .unwrap()
        .set_property_bool(proximate_driver::Property::InfiniteSelect, false)
        .unwrap();
    assert_eq!(
        device
            .select_passive_target(modulation(ModulationType::Iso14443A, BaudRate::Br106), None,)
            .unwrap(),
        None
    );
}

#[test]
fn reported_capabilities_only_include_implemented_initiator_paths() {
    let _guard = test_guard().lock().unwrap();
    reset_test_world();

    let connstring = ConnectionString::new("pn71xx").unwrap();
    let mut device = open_device(&connstring);
    let mut properties = device.property_ops().unwrap();
    assert_eq!(
        properties.supported_modulations(Mode::Initiator).unwrap(),
        vec![
            ModulationType::Iso14443A,
            ModulationType::Felica,
            ModulationType::Iso14443B,
            ModulationType::Iso14443Bi,
            ModulationType::Iso14443B2Sr,
            ModulationType::Iso14443B2Ct,
            ModulationType::Jewel,
        ]
    );
    assert_eq!(
        properties.supported_modulations(Mode::Target),
        Err(Error::UnsupportedOperation("pn71xx_target_mode"))
    );
    assert_eq!(
        properties.supported_baud_rates(Mode::Initiator, ModulationType::Dep),
        Err(Error::DeviceOperationFailed {
            operation: "pn71xx_get_supported_baud_rate",
            code: NFC_EINVARG,
        })
    );
}
