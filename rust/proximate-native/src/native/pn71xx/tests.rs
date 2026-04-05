use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use proximate_driver::{
    BaudRate, ConnectionString, Context, Driver, Error, Modulation, ModulationType, Target,
    TargetInfo,
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
use super::runtime::{current_tag_snapshot, runtime_snapshot};
use crate::nci::TagInfo;

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
        passive_scan.poll_target(modulations, poll_nr, period)
    }

    fn transceive_bytes(&mut self, tx: &[u8], rx: &mut [u8], timeout: i32) -> Result<usize, Error> {
        let mut initiator_io = self.initiator_io_ops()?;
        initiator_io.transceive_bytes(tx, rx, timeout)
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
    proximate_driver::Device::from_handle(driver.open(&Context::new(), connstring).unwrap())
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
    assert!(!runtime.initialized);
    assert!(!runtime.callbacks_registered);
    assert!(!runtime.discovery_enabled);
    assert!(runtime.active_device.is_none());

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
            Modulation {
                modulation_type: ModulationType::Iso14443A,
                baud_rate: BaudRate::Br106,
            },
        ),
        (
            make_tag(TARGET_TYPE_ISO14443_3A, &[0x10, 0x11, 0x12, 0x13], 0),
            Modulation {
                modulation_type: ModulationType::Iso14443A,
                baud_rate: BaudRate::Br106,
            },
        ),
        (
            make_tag(TARGET_TYPE_ISO14443_3B, &[0x21, 0x22, 0x23, 0x24], 0),
            Modulation {
                modulation_type: ModulationType::Iso14443B,
                baud_rate: BaudRate::Br106,
            },
        ),
        (
            make_tag(TARGET_TYPE_ISO14443_3B, &[0x31, 0x32, 0x33, 0x34], 0),
            Modulation {
                modulation_type: ModulationType::Iso14443Bi,
                baud_rate: BaudRate::Br106,
            },
        ),
        (
            make_tag(
                TARGET_TYPE_ISO14443_3B,
                &[0x41, 0x42, 0x43, 0x44, 0x45, 0x46],
                0,
            ),
            Modulation {
                modulation_type: ModulationType::Iso14443B2Sr,
                baud_rate: BaudRate::Br106,
            },
        ),
        (
            make_tag(TARGET_TYPE_ISO14443_3B, &[0x51, 0x52, 0x53, 0x54], 0),
            Modulation {
                modulation_type: ModulationType::Iso14443B2Ct,
                baud_rate: BaudRate::Br106,
            },
        ),
        (
            make_tag(
                TARGET_TYPE_FELICA,
                &[0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68],
                0,
            ),
            Modulation {
                modulation_type: ModulationType::Felica,
                baud_rate: BaudRate::Br212,
            },
        ),
        (
            make_tag(
                TARGET_TYPE_ISO14443_3A,
                &[0x71, 0x72, 0x73, 0x74],
                NFA_PROTOCOL_T1T,
            ),
            Modulation {
                modulation_type: ModulationType::Jewel,
                baud_rate: BaudRate::Br106,
            },
        ),
    ];

    for (tag, modulation) in cases {
        emit_tag_arrival_for_tests(tag);
        let target = device
            .select_passive_target(modulation, None)
            .unwrap()
            .expect("target should be present");

        match target.info {
            TargetInfo::Iso14443A { uid, sak, ats, .. } => {
                assert_eq!(uid, tag.uid[..tag.uid_length as usize].to_vec());
                if tag.technology == TARGET_TYPE_MIFARE_CLASSIC {
                    assert_eq!(sak, 0x08);
                    assert!(ats.is_empty());
                } else {
                    assert_eq!(sak, 0x20);
                    assert_eq!(ats, DESFIRE_ATS.to_vec());
                }
            }
            TargetInfo::Iso14443B { pupi, .. } => assert_eq!(pupi, [0x21, 0x22, 0x23, 0x24]),
            TargetInfo::Iso14443Bi { div, .. } => assert_eq!(div, [0x31, 0x32, 0x33, 0x34]),
            TargetInfo::Iso14443B2Sr { uid } => {
                assert_eq!(&uid[..6], &[0x41, 0x42, 0x43, 0x44, 0x45, 0x46])
            }
            TargetInfo::Iso14443B2Ct { uid, .. } => assert_eq!(uid, [0x51, 0x52, 0x53, 0x54]),
            TargetInfo::Felica { id, .. } => {
                assert_eq!(id, [0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68])
            }
            TargetInfo::Jewel { id, .. } => assert_eq!(id, [0x71, 0x72, 0x73, 0x74]),
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

    let modulations = [Modulation {
        modulation_type: ModulationType::Iso14443A,
        baud_rate: BaudRate::Br106,
    }];
    let target = device
        .poll_target(&modulations, 2, 1)
        .unwrap()
        .expect("target should appear");
    worker.join().unwrap();
    match target.info {
        TargetInfo::Iso14443A { uid, .. } => assert_eq!(uid, vec![0xAA, 0xBB]),
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
    assert_eq!(state.last_transceive_timeout, Some(500));
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
