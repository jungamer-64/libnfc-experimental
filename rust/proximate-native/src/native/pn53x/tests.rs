use super::*;
use std::collections::VecDeque;

fn cascade_iso14443a_uid(uid: &[u8]) -> Vec<u8> {
    match uid.len() {
        4 => uid.to_vec(),
        7 => {
            let mut cascaded = Vec::with_capacity(8);
            cascaded.extend_from_slice(&[0x88, uid[0], uid[1], uid[2]]);
            cascaded.extend_from_slice(&uid[3..]);
            cascaded
        }
        10 => {
            let mut cascaded = Vec::with_capacity(12);
            cascaded.extend_from_slice(&[0x88, uid[0], uid[1], uid[2]]);
            cascaded.extend_from_slice(&[0x88, uid[3], uid[4], uid[5]]);
            cascaded.extend_from_slice(&uid[6..]);
            cascaded
        }
        _ => Vec::new(),
    }
}

fn default_initiator_payload(modulation: Modulation) -> &'static [u8] {
    match modulation.modulation_type {
        ModulationType::Iso14443B => &[0x00],
        ModulationType::Iso14443Bi => &[0x01, 0x0b, 0x3f, 0x80],
        ModulationType::Felica => &[0x00, 0xff, 0xff, 0x01, 0x00],
        _ => &[],
    }
}

trait TestPn53xOps:
    DeviceMeta + PropertyBackend + InitiatorBackend + TargetBackend + Pn53xBackend
{
    fn pn53x_read_register(&mut self, register: u16) -> Result<u8, Error> {
        self.pn53x_read_register_driver(register)
    }

    fn pn53x_write_register(
        &mut self,
        register: u16,
        symbol_mask: u8,
        value: u8,
    ) -> Result<(), Error> {
        self.pn53x_write_register_driver(register, symbol_mask, value)
    }

    fn pn53x_transceive(&mut self, tx: &[u8], rx: &mut [u8], timeout: i32) -> Result<usize, Error> {
        self.pn53x_transceive_driver(tx, rx, timeout)
    }

    fn pn532_sam_configuration(&mut self, mode: u8, timeout: i32) -> Result<i32, Error> {
        self.pn532_sam_configuration_driver(mode, timeout)
    }

    fn initiator_init(&mut self) -> Result<i32, Error> {
        for (property, value) in [
            (Property::ActivateField, false),
            (Property::ActivateField, true),
            (Property::InfiniteSelect, true),
            (Property::AutoIso14443_4, true),
            (Property::ForceIso14443A, true),
            (Property::ForceSpeed106, true),
            (Property::AcceptInvalidFrames, false),
            (Property::AcceptMultipleFrames, false),
        ] {
            self.set_property_bool(property, value)?;
        }
        self.initiator_init_driver()
    }

    fn abort_command(&mut self) -> Result<(), Error> {
        self.abort_command_driver()
    }

    fn select_passive_target(
        &mut self,
        modulation: Modulation,
        init_data: Option<&[u8]>,
    ) -> Result<Option<Target>, Error> {
        let payload = if init_data.is_some_and(|value| !value.is_empty()) {
            if modulation.modulation_type == ModulationType::Iso14443A {
                cascade_iso14443a_uid(init_data.expect("checked above"))
            } else {
                init_data.expect("checked above").to_vec()
            }
        } else {
            default_initiator_payload(modulation).to_vec()
        };
        self.select_passive_target_driver(modulation, &payload)
    }

    fn select_dep_target(
        &mut self,
        mode: DepMode,
        baud_rate: BaudRate,
        initiator: Option<&DepInfo>,
        timeout: i32,
    ) -> Result<Option<Target>, Error> {
        self.select_dep_target_driver(mode, baud_rate, initiator, timeout)
    }

    fn deselect_target(&mut self) -> Result<(), Error> {
        self.deselect_target_driver()
    }

    fn target_is_present(&mut self, target: Option<&Target>) -> Result<bool, Error> {
        self.target_is_present_driver(target)
    }

    fn transceive_bytes(&mut self, tx: &[u8], rx: &mut [u8], timeout: i32) -> Result<usize, Error> {
        self.transceive_bytes_driver(tx, rx, timeout)
    }

    fn transceive_bytes_timed(&mut self, tx: &[u8], rx: &mut [u8]) -> Result<(usize, u32), Error> {
        self.transceive_bytes_timed_driver(tx, rx)
    }

    fn target_init(
        &mut self,
        target: &mut Target,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, Error> {
        for (property, value) in [
            (Property::AcceptInvalidFrames, false),
            (Property::AcceptMultipleFrames, false),
            (Property::HandleCrc, true),
            (Property::HandleParity, true),
            (Property::AutoIso14443_4, true),
            (Property::EasyFraming, true),
            (Property::ActivateCrypto1, false),
            (Property::ActivateField, false),
        ] {
            self.set_property_bool(property, value)?;
        }
        self.target_init_driver(target, rx, timeout)
    }

    fn target_send_bytes(&mut self, tx: &[u8], timeout: i32) -> Result<usize, Error> {
        self.target_send_bytes_driver(tx, timeout)
    }

    fn target_receive_bytes(&mut self, rx: &mut [u8], timeout: i32) -> Result<usize, Error> {
        self.target_receive_bytes_driver(rx, timeout)
    }

    fn transceive_bits(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        self.transceive_bits_driver(tx, tx_bits_len, tx_parity, rx, rx_parity)
    }

    fn target_receive_bits(
        &mut self,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, Error> {
        self.target_receive_bits_driver(rx, rx_parity)
    }

    fn transceive_bits_timed(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), Error> {
        self.transceive_bits_timed_driver(tx, tx_bits_len, tx_parity, rx, rx_parity)
    }

    fn target_send_bits(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
    ) -> Result<usize, Error> {
        self.target_send_bits_driver(tx, tx_bits_len, tx_parity)
    }
}

impl<T> TestPn53xOps for Pn53xDevice<T> where T: Pn53xTransport + Send + 'static {}

#[derive(Default)]
struct FakeTransport {
    sent: Vec<Vec<u8>>,
    received: VecDeque<Vec<u8>>,
    wake_up_calls: usize,
    abort_calls: usize,
}

impl Pn53xTransport for FakeTransport {
    fn send(&mut self, payload: &[u8], _timeout_ms: i32) -> Result<(), Error> {
        self.sent.push(payload.to_vec());
        Ok(())
    }

    fn receive(&mut self, buffer: &mut [u8], _timeout_ms: i32) -> Result<usize, Error> {
        let payload = self
            .received
            .pop_front()
            .ok_or_else(|| status_error("receive", NFC_ETIMEOUT))?;
        if payload.len() > buffer.len() {
            return Err(Error::BufferTooSmall {
                needed: payload.len(),
                available: buffer.len(),
            });
        }
        buffer[..payload.len()].copy_from_slice(&payload);
        Ok(payload.len())
    }

    fn abort_command(&mut self) -> Result<(), Error> {
        self.abort_calls += 1;
        Ok(())
    }

    fn wake_up(&mut self) -> Result<(), Error> {
        self.wake_up_calls += 1;
        Ok(())
    }
}

fn response_frame(command: u8, payload: &[u8]) -> Vec<u8> {
    let body_len = payload.len() + 2;
    let len = body_len as u8;
    let mut frame = vec![
        0x00,
        0x00,
        0xff,
        len,
        (!len).wrapping_add(1),
        PN53X_TO_HOST_TFI,
        command.wrapping_add(1),
    ];
    frame.extend_from_slice(payload);
    let dcs = frame[5..]
        .iter()
        .fold(0u8, |sum, byte| sum.wrapping_add(*byte))
        .wrapping_neg();
    frame.push(dcs);
    frame.push(0x00);
    frame
}

fn queue_probe_responses(transport: &mut FakeTransport) {
    transport.received.push_back(PN53X_ACK_FRAME.to_vec());
    transport
        .received
        .push_back(response_frame(PN532_SAM_CONFIGURATION, &[]));
    transport.received.push_back(PN53X_ACK_FRAME.to_vec());
    transport
        .received
        .push_back(response_frame(0x02, &[0x32, 0x01, 0x06, 0x07]));
}

fn queue_command_response(transport: &mut FakeTransport, command: u8, payload: &[u8]) {
    transport.received.push_back(PN53X_ACK_FRAME.to_vec());
    transport
        .received
        .push_back(response_frame(command, payload));
}

fn probed_device() -> Pn53xDevice<FakeTransport> {
    let mut transport = FakeTransport::default();
    queue_probe_responses(&mut transport);
    let connstring = ConnectionString::new("pn532_uart:/dev/null:115200").unwrap();
    Pn53xDevice::probe_with_profile(
        "PN532",
        connstring,
        Pn53xProfile::pn532("pn532_uart"),
        transport,
        25,
    )
    .unwrap()
}

#[test]
fn hidden_pn53x_helpers_route_through_shared_core() {
    let mut device = probed_device();
    queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0x12]);
    assert_eq!(device.pn53x_read_register(0x6302).unwrap(), 0x12);

    queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0x12]);
    queue_command_response(&mut device.transport, PN53X_WRITE_REGISTER, &[]);
    device.pn53x_write_register(0x6302, 0x0f, 0x05).unwrap();

    queue_command_response(&mut device.transport, 0x40, &[0xaa, 0xbb]);
    let mut rx = [0u8; 4];
    assert_eq!(
        device
            .pn53x_transceive(&[0x40, 0xde, 0xad], &mut rx, 25)
            .unwrap(),
        2
    );
    assert_eq!(&rx[..2], &[0xaa, 0xbb]);

    queue_command_response(&mut device.transport, PN532_SAM_CONFIGURATION, &[]);
    assert_eq!(device.pn532_sam_configuration(0x03, 25).unwrap(), 0);
}

#[test]
fn build_frame_supports_standard_frames() {
    let frame = build_frame(&[0x02, 0x03, 0x04]).unwrap();
    assert_eq!(
        frame,
        vec![
            0x00, 0x00, 0xff, 0x04, 0xfc, 0xD4, 0x02, 0x03, 0x04, 0x23, 0x00
        ]
    );
}

#[test]
fn build_frame_supports_extended_frames() {
    let payload = vec![0xAB; 255];
    let frame = build_frame(&payload).unwrap();
    assert_eq!(
        &frame[..9],
        &[0x00, 0x00, 0xff, 0xff, 0xff, 0x01, 0x00, 0xff, 0xD4]
    );
    assert_eq!(frame.len(), payload.len() + 11);
    assert_eq!(*frame.last().unwrap(), 0x00);
}

#[test]
fn build_frame_rejects_empty_payloads() {
    assert_eq!(build_frame(&[]), Err(Error::InvalidArgument("payload")));
}

#[test]
fn build_frame_rejects_oversized_payloads() {
    let payload = vec![0xAA; PN53X_EXTENDED_FRAME_DATA_MAX_LEN + 1];
    assert_eq!(
        build_frame(&payload),
        Err(Error::BufferTooSmall {
            needed: payload.len(),
            available: PN53X_EXTENDED_FRAME_DATA_MAX_LEN,
        })
    );
}

#[test]
fn ack_frame_helper_matches_prefix() {
    assert!(is_ack_frame(&PN53X_ACK_FRAME));
    assert!(is_ack_frame(&[0x00, 0x00, 0xff, 0x00, 0xff, 0x00, 0x90]));
    assert!(!is_ack_frame(&[0x00, 0x00, 0xff, 0x01, 0xff, 0x00]));
}

#[test]
fn parse_response_frame_validates_payload_and_command() {
    let frame = response_frame(0x02, &[0x32, 0x01, 0x06, 0x07]);
    assert_eq!(
        parse_response_frame(&frame, 0x02).unwrap(),
        vec![0x32, 0x01, 0x06, 0x07]
    );
}

#[test]
fn exchange_command_wakes_up_and_tracks_last_command() {
    let mut transport = FakeTransport::default();
    queue_probe_responses(&mut transport);

    let mut core = Pn53xCore::default();
    let payload = core
        .get_firmware_version(Pn53xProfile::pn532("pn532_uart"), &mut transport, 25)
        .unwrap();

    assert_eq!(transport.wake_up_calls, 1);
    assert_eq!(transport.sent.len(), 2);
    assert_eq!(payload.ic, 0x32);
    assert_eq!(core.chip_type(), Pn53xType::Pn532);
    assert_eq!(core.last_command(), Some(0x02));
    assert_eq!(core.power_mode(), Pn53xPowerMode::Normal);
}

#[test]
fn probe_builds_pure_rust_device_and_reports_information() {
    let mut transport = FakeTransport::default();
    queue_probe_responses(&mut transport);

    let connstring = ConnectionString::new("pn532_uart:/dev/ttyUSB0:115200").unwrap();
    let mut device = Pn53xDevice::probe_with_profile(
        "PN532",
        connstring,
        Pn53xProfile::pn532("pn532_uart"),
        transport,
        25,
    )
    .unwrap();

    assert_eq!(device.name(), "PN532");
    assert_eq!(device.last_error(), 0);
    assert_eq!(
        device.information_about().unwrap(),
        "PN532 firmware v1.6 support=0x07 via pn532_uart:/dev/ttyUSB0:115200"
    );
}

#[test]
fn device_property_state_and_initiator_defaults_are_pure_rust() {
    let mut transport = FakeTransport::default();
    queue_probe_responses(&mut transport);

    let connstring = ConnectionString::new("pn532_uart:/dev/null:115200").unwrap();
    let mut device = Pn53xDevice::probe_with_profile(
        "PN532",
        connstring,
        Pn53xProfile::pn532("pn532_uart"),
        transport,
        25,
    )
    .unwrap();

    assert_eq!(
        device.property_bool_state(Property::EasyFraming),
        Some(true)
    );
    device
        .set_property_bool(Property::EasyFraming, false)
        .unwrap();
    device
        .set_property_int(Property::TimeoutCommand, 900)
        .unwrap();
    device.initiator_init().unwrap();

    assert_eq!(
        device.property_bool_state(Property::EasyFraming),
        Some(false)
    );
    assert_eq!(
        device.property_bool_state(Property::InfiniteSelect),
        Some(true)
    );
    assert_eq!(
        device.property_bool_state(Property::ForceSpeed106),
        Some(true)
    );
    assert_eq!(device.last_error(), 0);
}

#[test]
fn abort_command_delegates_to_transport() {
    let mut transport = FakeTransport::default();
    queue_probe_responses(&mut transport);

    let connstring = ConnectionString::new("pn532_uart:/dev/null:115200").unwrap();
    let mut device = Pn53xDevice::probe_with_profile(
        "PN532",
        connstring,
        Pn53xProfile::pn532("pn532_uart"),
        transport,
        25,
    )
    .unwrap();
    device.abort_command().unwrap();

    assert_eq!(device.transport.abort_calls, 1);
}

#[test]
fn transport_timeout_is_preserved_as_device_error() {
    let mut transport = FakeTransport::default();
    transport.received.push_back(PN53X_ACK_FRAME.to_vec());

    let mut core = Pn53xCore::default();
    let error = core
        .get_firmware_version(Pn53xProfile::pn532("pn532_uart"), &mut transport, 25)
        .unwrap_err();

    assert_eq!(
        error,
        Error::DeviceOperationFailed {
            operation: "receive",
            code: NFC_ETIMEOUT,
        }
    );
}

#[test]
fn status_constants_match_expected_negative_codes() {
    assert_eq!(NFC_EOPABORTED, -7);
}

#[test]
fn select_passive_target_decodes_iso14443a_and_tracks_current_target() {
    let mut device = probed_device();
    device
        .transport
        .received
        .push_back(PN53X_ACK_FRAME.to_vec());
    device.transport.received.push_back(response_frame(
        PN53X_IN_LIST_PASSIVE_TARGET,
        &[
            0x01, 0x01, 0x04, 0x00, 0x08, 0x04, 0xde, 0xad, 0xbe, 0xef, 0x05, 0x75, 0x77, 0x81,
            0x02,
        ],
    ));

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

    assert_eq!(
        target.info,
        TargetInfo::Iso14443A {
            atqa: [0x04, 0x00],
            sak: 0x08,
            uid: vec![0xde, 0xad, 0xbe, 0xef],
            ats: vec![0x75, 0x77, 0x81, 0x02],
        }
    );
    assert_eq!(device.core.current_target(), Some(&target));
}

#[test]
fn select_dep_target_and_deselect_share_runtime_logic() {
    let mut device = probed_device();
    queue_command_response(
        &mut device.transport,
        PN53X_IN_JUMP_FOR_DEP,
        &[
            0x00, 0x01, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x22, 0x33,
            0x44, 0x55, 0x66, 0xaa, 0xbb,
        ],
    );
    queue_command_response(&mut device.transport, 0x00, &[0x00]);
    queue_command_response(&mut device.transport, PN53X_IN_DESELECT, &[0x00]);

    let target = device
        .select_dep_target(DepMode::Passive, BaudRate::Br106, None, 250)
        .unwrap()
        .unwrap();
    assert_eq!(
        target.info,
        TargetInfo::Dep(DepInfo {
            nfcid3: [0x11; 10],
            did: 0x22,
            bs: 0x33,
            br: 0x44,
            timeout: 0x55,
            pp: 0x66,
            general_bytes: vec![0xaa, 0xbb],
            mode: DepMode::Passive,
        })
    );
    assert!(device.target_is_present(Some(&target)).unwrap());

    device.deselect_target().unwrap();
    assert!(device.core.current_target().is_none());
}

#[test]
fn transceive_bytes_and_timed_variant_use_shared_timer_register_flow() {
    let mut device = probed_device();
    device
        .transport
        .received
        .push_back(PN53X_ACK_FRAME.to_vec());
    device
        .transport
        .received
        .push_back(response_frame(PN53X_IN_DATA_EXCHANGE, &[0x00, 0x90, 0x00]));

    let mut rx = [0u8; 8];
    let written = device
        .transceive_bytes(&[0x30, 0x04], &mut rx, 250)
        .unwrap();
    assert_eq!(written, 2);
    assert_eq!(&rx[..written], &[0x90, 0x00]);

    device
        .set_property_bool(Property::EasyFraming, false)
        .unwrap();
    device
        .set_property_bool(Property::HandleCrc, false)
        .unwrap();
    queue_command_response(&mut device.transport, PN53X_WRITE_REGISTER, &[]);
    queue_command_response(&mut device.transport, PN53X_WRITE_REGISTER, &[]);
    queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0x01]);
    queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0xaa, 0x00]);
    queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0xf0, 0x00]);

    let (timed_written, elapsed) = device.transceive_bytes_timed(&[0x50], &mut rx).unwrap();
    assert_eq!(timed_written, 1);
    assert_eq!(&rx[..timed_written], &[0xaa]);
    assert_eq!(elapsed, 3568);
}

#[test]
fn target_init_and_target_byte_io_are_shared() {
    let mut device = probed_device();
    device
        .transport
        .received
        .push_back(PN53X_ACK_FRAME.to_vec());
    device
        .transport
        .received
        .push_back(response_frame(PN53X_TG_INIT_AS_TARGET, &[0x04, 0xca, 0xfe]));
    device
        .transport
        .received
        .push_back(PN53X_ACK_FRAME.to_vec());
    device
        .transport
        .received
        .push_back(response_frame(PN53X_TG_SET_DATA, &[0x00]));
    device
        .transport
        .received
        .push_back(PN53X_ACK_FRAME.to_vec());
    device
        .transport
        .received
        .push_back(response_frame(PN53X_TG_GET_DATA, &[0x00, 0xbe, 0xef]));

    let mut target = Target {
        modulation: Modulation {
            modulation_type: ModulationType::Dep,
            baud_rate: BaudRate::Undefined,
        },
        info: TargetInfo::Dep(DepInfo {
            nfcid3: [0x22; 10],
            did: 0x01,
            bs: 0x02,
            br: 0x03,
            timeout: 0x04,
            pp: 0x05,
            general_bytes: vec![0xaa],
            mode: DepMode::Passive,
        }),
    };
    let mut rx = [0u8; 8];
    let init_len = device.target_init(&mut target, &mut rx, 250).unwrap();
    assert_eq!(init_len, 2);
    assert_eq!(&rx[..init_len], &[0xca, 0xfe]);
    assert_eq!(target.modulation.baud_rate, BaudRate::Br106);

    assert_eq!(device.target_send_bytes(&[0x90], 250).unwrap(), 1);
    let recv_len = device.target_receive_bytes(&mut rx, 250).unwrap();
    assert_eq!(recv_len, 2);
    assert_eq!(&rx[..recv_len], &[0xbe, 0xef]);
}

#[test]
fn wrap_and_unwrap_frame_preserve_parity_bits() {
    let wrapped = pn53x_wrap_frame(&[0x93, 0x20], 16, Some(&[1, 0])).unwrap();
    let mut rx = [0u8; 8];
    let mut parity = [0u8; 8];
    let bits = pn53x_unwrap_frame(&wrapped, 18, &mut rx, Some(&mut parity)).unwrap();

    assert_eq!(bits, 16);
    assert_eq!(&rx[..2], &[0x93, 0x20]);
    assert_eq!(&parity[..2], &[1, 0]);
}

#[test]
fn transceive_bits_supports_short_frames_with_register_backed_last_bits() {
    let mut device = probed_device();
    queue_command_response(
        &mut device.transport,
        PN53X_READ_REGISTER,
        &[SYMBOL_TX_CRC_ENABLE],
    );
    queue_command_response(&mut device.transport, PN53X_WRITE_REGISTER, &[]);
    queue_command_response(
        &mut device.transport,
        PN53X_IN_COMMUNICATE_THRU,
        &[0x00, 0x04, 0x00],
    );
    queue_command_response(
        &mut device.transport,
        PN53X_READ_REGISTER,
        &[SYMBOL_TX_CRC_ENABLE],
    );
    let mut rx = [0u8; 8];
    let bits = device
        .transceive_bits(&[0x26], 7, None, &mut rx, None)
        .unwrap();
    assert_eq!(bits, 16);
    assert_eq!(&rx[..2], &[0x04, 0x00]);
}

#[test]
fn target_receive_bits_unwraps_raw_frame_and_parity() {
    let mut device = probed_device();
    device
        .set_property_bool(Property::HandleParity, false)
        .unwrap();
    let wrapped = pn53x_wrap_frame(&[0x93, 0x20], 16, Some(&[1, 0])).unwrap();
    let mut payload = Vec::with_capacity(wrapped.len() + 1);
    payload.push(0x00);
    payload.extend_from_slice(&wrapped);
    queue_command_response(
        &mut device.transport,
        PN53X_TG_GET_INITIATOR_COMMAND,
        &payload,
    );
    queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0x02]);

    let mut rx = [0u8; 8];
    let mut parity = [0u8; 8];
    let bits = device
        .target_receive_bits(&mut rx, Some(&mut parity))
        .unwrap();
    assert_eq!(bits, 16);
    assert_eq!(&rx[..2], &[0x93, 0x20]);
    assert_eq!(&parity[..2], &[1, 0]);
}

#[test]
fn transceive_bits_timed_uses_shared_register_timer_flow() {
    let mut device = probed_device();
    device
        .set_property_bool(Property::EasyFraming, false)
        .unwrap();
    device
        .set_property_bool(Property::HandleParity, false)
        .unwrap();
    device
        .set_property_bool(Property::HandleCrc, false)
        .unwrap();
    let wrapped = pn53x_wrap_frame(&[0x93, 0x20], 16, Some(&[1, 0])).unwrap();
    queue_command_response(&mut device.transport, PN53X_WRITE_REGISTER, &[]);
    queue_command_response(&mut device.transport, PN53X_WRITE_REGISTER, &[]);
    queue_command_response(
        &mut device.transport,
        PN53X_READ_REGISTER,
        &[wrapped.len() as u8],
    );
    let mut fifo_payload = wrapped.clone();
    fifo_payload.push(0x00);
    queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &fifo_payload);
    queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0x02]);
    queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0xf0, 0x00]);

    let mut rx = [0u8; 8];
    let mut parity = [0u8; 8];
    let (bits, elapsed) = device
        .transceive_bits_timed(&[0x26], 7, None, &mut rx, Some(&mut parity))
        .unwrap();
    assert_eq!(bits, 16);
    assert_eq!(&rx[..2], &[0x93, 0x20]);
    assert_eq!(&parity[..2], &[1, 0]);
    assert_eq!(elapsed, 3504);
}

#[test]
fn target_send_bits_wraps_non_byte_aligned_frames() {
    let mut device = probed_device();
    device
        .set_property_bool(Property::HandleParity, false)
        .unwrap();
    queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0x00]);
    queue_command_response(&mut device.transport, PN53X_WRITE_REGISTER, &[]);
    queue_command_response(
        &mut device.transport,
        PN53X_TG_RESPONSE_TO_INITIATOR,
        &[0x00],
    );
    queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0x00]);

    let sent = device
        .target_send_bits(&[0x93, 0x20], 16, Some(&[1, 0]))
        .unwrap();
    assert_eq!(sent, 16);
}

#[test]
fn timed_bytes_reads_tx_mode_before_register_timed_exchange() {
    let mut device = probed_device();
    device
        .set_property_bool(Property::EasyFraming, false)
        .unwrap();
    let sent_before = device.transport.sent.len();
    queue_command_response(
        &mut device.transport,
        PN53X_READ_REGISTER,
        &[SYMBOL_TX_CRC_ENABLE],
    );
    queue_command_response(&mut device.transport, PN53X_WRITE_REGISTER, &[]);
    queue_command_response(&mut device.transport, PN53X_WRITE_REGISTER, &[]);
    queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0x02]);
    queue_command_response(
        &mut device.transport,
        PN53X_READ_REGISTER,
        &[0x90, 0x00, 0x00],
    );
    queue_command_response(&mut device.transport, PN53X_READ_REGISTER, &[0xf0, 0x00]);

    let mut rx = [0u8; 4];
    let (written, elapsed) = device.transceive_bytes_timed(&[0x00], &mut rx).unwrap();
    assert_eq!(written, 2);
    assert_eq!(&rx[..2], &[0x90, 0x00]);
    assert_eq!(elapsed, 3504);
    assert_eq!(device.transport.sent[sent_before][6], PN53X_READ_REGISTER);
    assert_eq!(
        &device.transport.sent[sent_before][7..9],
        &[
            (PN53X_REG_CIU_TX_MODE >> 8) as u8,
            PN53X_REG_CIU_TX_MODE as u8
        ]
    );
}

#[test]
fn target_is_present_for_dep_uses_shared_diagnose_path() {
    let mut device = probed_device();
    let target = Target {
        modulation: Modulation {
            modulation_type: ModulationType::Dep,
            baud_rate: BaudRate::Br106,
        },
        info: TargetInfo::Dep(DepInfo {
            nfcid3: [0x11; 10],
            did: 0x22,
            bs: 0x33,
            br: 0x44,
            timeout: 0x55,
            pp: 0x66,
            general_bytes: vec![0xaa, 0xbb],
            mode: DepMode::Passive,
        }),
    };
    device.core.remember_target(target.clone());
    queue_command_response(&mut device.transport, 0x00, &[0x00]);

    assert!(device.target_is_present(Some(&target)).unwrap());
}

#[test]
fn target_is_present_for_mifare_classic_reselects_saved_uid() {
    let mut device = probed_device();
    let target = Target {
        modulation: Modulation {
            modulation_type: ModulationType::Iso14443A,
            baud_rate: BaudRate::Br106,
        },
        info: TargetInfo::Iso14443A {
            atqa: [0x00, 0x04],
            sak: 0x08,
            uid: vec![0xde, 0xad, 0xbe, 0xef],
            ats: Vec::new(),
        },
    };
    device.core.remember_target(target.clone());
    queue_command_response(
        &mut device.transport,
        PN53X_IN_LIST_PASSIVE_TARGET,
        &[0x01, 0x01, 0x04, 0x00, 0x08, 0x04, 0xde, 0xad, 0xbe, 0xef],
    );

    assert!(device.target_is_present(Some(&target)).unwrap());
}
