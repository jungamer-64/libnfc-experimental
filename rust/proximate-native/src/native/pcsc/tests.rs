use super::fake::{FakeCardState, FakePcscBackend, FakePcscCard};
use super::*;
use std::sync::Mutex;

fn iso14443a_status() -> PcscCardStatus {
    PcscCardStatus {
        present: true,
        atr: vec![0x3B, 0x83, 0x80, 0x01, 0xAA, 0xBB, 0xCC, 0xDD],
        protocol: Some(PcscProtocol::T0),
    }
}

#[test]
fn scan_filters_out_acr122_readers() {
    let backend = Arc::new(
        FakePcscBackend::default()
            .with_reader("ACS ACR122U PICC Interface 00 00", FakeCardState::default())
            .with_reader("Feitian R502 CL Reader 0", FakeCardState::default()),
    );
    let driver = PcscDriver::with_backend(backend);
    let context = Context::new();

    let proximate_driver::DriverScan::Complete(devices) = driver.scan(&context).unwrap() else {
        panic!("PC/SC unexpectedly unavailable");
    };
    assert_eq!(devices.len(), 1);
    assert_eq!(
        devices[0].connstring.as_str(),
        "pcsc:Feitian R502 CL Reader 0"
    );
}

fn pcsc_status(value: ::pcsc::ffi::LONG) -> i32 {
    value as u32 as i32
}

#[test]
fn scan_distinguishes_no_readers_from_service_unavailability() {
    let context = Context::new();
    let no_readers = PcscDriver::with_backend(Arc::new(
        FakePcscBackend::default()
            .with_list_error(pcsc_status(::pcsc::ffi::SCARD_E_NO_READERS_AVAILABLE)),
    ));
    assert_eq!(
        no_readers.scan(&context),
        Ok(proximate_driver::DriverScan::Complete(Vec::new()))
    );

    for status in [
        pcsc_status(::pcsc::ffi::SCARD_E_NO_SERVICE),
        pcsc_status(::pcsc::ffi::SCARD_E_SERVICE_STOPPED),
    ] {
        let driver =
            PcscDriver::with_backend(Arc::new(FakePcscBackend::default().with_list_error(status)));
        assert_eq!(
            driver.scan(&context),
            Ok(proximate_driver::DriverScan::Unavailable(
                Error::DeviceOperationFailed {
                    operation: "pcsc_scan",
                    code: status,
                }
            ))
        );
    }
}

#[test]
fn scan_propagates_unexpected_pcsc_status() {
    let context = Context::new();
    let status = pcsc_status(::pcsc::ffi::SCARD_E_INVALID_HANDLE);
    let driver =
        PcscDriver::with_backend(Arc::new(FakePcscBackend::default().with_list_error(status)));
    assert_eq!(
        driver.scan(&context),
        Err(Error::DeviceOperationFailed {
            operation: "pcsc_scan",
            code: status,
        })
    );
}

#[test]
fn open_resolves_index_connstrings() {
    let backend = Arc::new(
        FakePcscBackend::default()
            .with_reader("Reader A", FakeCardState::default())
            .with_reader("Reader B", FakeCardState::default()),
    );
    let driver = PcscDriver::with_backend(backend);
    let context = Context::new();

    let connstring = ConnectionString::new("pcsc:1").unwrap();
    let device = driver.open(&context, &connstring).unwrap();
    assert_eq!(device.connstring().as_str(), "pcsc:Reader B");
}

#[test]
fn select_passive_target_builds_iso14443a_target() {
    let mut state = FakeCardState::default();
    state.status_responses.push_back(Ok(iso14443a_status()));
    state
        .attributes
        .insert(PcscAttribute::IccTypePerAtr, Ok(vec![ICC_TYPE_14443A]));
    state
        .transmit_responses
        .push_back(Ok(vec![0x01, 0x02, 0x03, 0x04, 0x90, 0x00]));
    let state = Arc::new(Mutex::new(state));
    let card = Box::new(FakePcscCard {
        state: Arc::clone(&state),
    });
    let device = PcscDevice::new(
        "Reader A".into(),
        ConnectionString::new("pcsc:Reader A").unwrap(),
        card,
        PcscShareMode::Direct,
        PcscProtocols::T0,
    );
    let mut device = proximate_driver::Device::from_backend(Box::new(device));

    let target = device
        .passive_scan_ops()
        .unwrap()
        .select_passive_target(
            Modulation::try_new(ModulationType::Iso14443A, BaudRate::Br106).unwrap(),
            None,
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        target.modulation().modulation_type(),
        ModulationType::Iso14443A
    );
    match target.info() {
        TargetInfo::Iso14443A { uid, .. } => {
            assert_eq!(uid.as_slice(), &[0x01, 0x02, 0x03, 0x04])
        }
        _ => panic!("unexpected target info"),
    }
    assert_eq!(
        state.lock().unwrap().transmit_calls,
        vec![(vec![0xFF, 0xCA, 0x00, 0x00, 0x00], 258)]
    );
}

#[test]
fn feitian_transceive_routes_through_apdu_translation() {
    let mut state = FakeCardState::default();
    state.transmit_responses.push_back(Ok(vec![0x90, 0x00]));
    let state = Arc::new(Mutex::new(state));
    let card = Box::new(FakePcscCard {
        state: Arc::clone(&state),
    });
    let device = PcscDevice::new(
        "Feitian Reader".into(),
        ConnectionString::new("pcsc:Feitian Reader").unwrap(),
        card,
        PcscShareMode::Direct,
        PcscProtocols::T0,
    );
    let mut device = proximate_driver::Device::from_backend(Box::new(device));
    let mut rx = [0u8; 8];
    let size = device
        .initiator_io_ops()
        .unwrap()
        .transceive_bytes(
            &[0x30, 0x04],
            &mut rx,
            OperationTimeout::try_milliseconds(75).unwrap(),
        )
        .unwrap();
    assert_eq!(size, 2);
    assert_eq!(&rx[..size], &[0x90, 0x00]);
    assert_eq!(
        state.lock().unwrap().transmit_calls,
        vec![(vec![0xFF, 0xB0, 0x00, 0x04, 0x10], 10)]
    );
}

#[test]
fn information_about_formats_vendor_attributes() {
    let mut state = FakeCardState::default();
    state
        .attributes
        .insert(PcscAttribute::VendorName, Ok(b"Model\0".to_vec()));
    state
        .attributes
        .insert(PcscAttribute::VendorIfdType, Ok(b"Vendor\0".to_vec()));
    state
        .attributes
        .insert(PcscAttribute::VendorIfdVersion, Ok(b"1.0\0".to_vec()));
    state
        .attributes
        .insert(PcscAttribute::VendorIfdSerialNo, Ok(b"ABC123\0".to_vec()));
    let card = Box::new(FakePcscCard {
        state: Arc::new(Mutex::new(state)),
    });
    let mut device = PcscDevice::new(
        "Reader".into(),
        ConnectionString::new("pcsc:Reader").unwrap(),
        card,
        PcscShareMode::Direct,
        PcscProtocols::T0,
    );

    assert_eq!(
        device.information_about().unwrap(),
        "Model 1.0 (Vendor)\nserial: ABC123\n"
    );
}
