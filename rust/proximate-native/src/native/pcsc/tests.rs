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

    let devices = driver.scan(&context).unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].as_str(), "pcsc:Feitian R502 CL Reader 0");
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
    let backend = Arc::new(FakePcscBackend::default().with_reader("Reader A", state));
    let driver = PcscDriver::with_backend(backend);
    let context = Context::new();
    let connstring = ConnectionString::new("pcsc:Reader A").unwrap();
    let mut device = driver.open(&context, &connstring).unwrap();

    let target = device
        .select_passive_target(
            Modulation {
                modulation_type: ModulationType::Iso14443A,
                baud_rate: BaudRate::Br106,
            },
            None,
        )
        .unwrap()
        .unwrap();
    assert_eq!(target.modulation.modulation_type, ModulationType::Iso14443A);
    match target.info {
        TargetInfo::Iso14443A { uid, .. } => assert_eq!(uid, vec![0x01, 0x02, 0x03, 0x04]),
        _ => panic!("unexpected target info"),
    }
}

#[test]
fn feitian_transceive_routes_through_apdu_translation() {
    let mut state = FakeCardState::default();
    state.transmit_responses.push_back(Ok(vec![0x90, 0x00]));
    let card = Box::new(FakePcscCard {
        state: Arc::new(Mutex::new(state)),
    });
    let mut device = PcscDevice::new(
        "Feitian Reader".into(),
        ConnectionString::new("pcsc:Feitian Reader").unwrap(),
        card,
        PcscShareMode::Direct,
        PcscProtocols::T0,
    );
    let mut rx = [0u8; 8];
    let size = device.transceive_bytes(&[0x30, 0x04], &mut rx, 75).unwrap();
    assert_eq!(size, 2);
    assert_eq!(&rx[..size], &[0x90, 0x00]);
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
