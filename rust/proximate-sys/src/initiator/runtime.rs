use crate::bridge::driver_shim::borrowed_device;
use crate::lifecycle::nfc_device;
use libc::c_int;
use proximate_driver as rt;

pub(super) fn with_device<R>(
    raw: *mut nfc_device,
    f: impl FnOnce(&mut rt::Device) -> Result<R, rt::Error>,
) -> Result<R, rt::Error> {
    let mut device = borrowed_device(raw);
    f(&mut device)
}

pub(super) fn with_initiator<R>(
    raw: *mut nfc_device,
    f: impl FnOnce(&mut rt::InitiatorDevice<'_>) -> Result<R, rt::Error>,
) -> Result<R, rt::Error> {
    with_device(raw, |device| {
        let mut initiator = device.initiator()?;
        f(&mut initiator)
    })
}

pub(super) fn with_target<R>(
    raw: *mut nfc_device,
    f: impl FnOnce(&mut rt::TargetDevice<'_>) -> Result<R, rt::Error>,
) -> Result<R, rt::Error> {
    with_device(raw, |device| {
        let mut target = device.target()?;
        f(&mut target)
    })
}

pub(super) fn set_property_int(
    raw: *mut nfc_device,
    property: rt::Property,
    value: c_int,
) -> Result<(), rt::Error> {
    with_device(raw, |device| device.set_property_int(property, value))
}

pub(super) fn set_property_bool(
    raw: *mut nfc_device,
    property: rt::Property,
    enable: bool,
) -> Result<(), rt::Error> {
    with_device(raw, |device| device.set_property_bool(property, enable))
}

pub(super) fn supported_modulations(
    raw: *mut nfc_device,
    mode: rt::Mode,
) -> Result<Vec<rt::ModulationType>, rt::Error> {
    with_device(raw, |device| device.supported_modulations(mode))
}

pub(super) fn supported_baud_rates(
    raw: *mut nfc_device,
    mode: rt::Mode,
    modulation_type: rt::ModulationType,
) -> Result<Vec<rt::BaudRate>, rt::Error> {
    with_device(raw, |device| {
        device.supported_baud_rates(mode, modulation_type)
    })
}

pub(super) fn information_about(raw: *mut nfc_device) -> Result<String, rt::Error> {
    with_device(raw, |device| device.information_about())
}

pub(super) fn initiator_init(raw: *mut nfc_device) -> Result<i32, rt::Error> {
    with_initiator(raw, |initiator| initiator.init())
}

pub(super) fn initiator_init_secure_element(raw: *mut nfc_device) -> Result<i32, rt::Error> {
    with_initiator(raw, |initiator| initiator.init_secure_element())
}

pub(super) fn select_passive_target(
    raw: *mut nfc_device,
    modulation: rt::Modulation,
    init_data: Option<&[u8]>,
) -> Result<Option<rt::Target>, rt::Error> {
    with_initiator(raw, |initiator| {
        initiator.select_passive_target(modulation, init_data)
    })
}

pub(super) fn list_passive_targets(
    raw: *mut nfc_device,
    modulation: rt::Modulation,
    max_targets: usize,
) -> Result<Vec<rt::Target>, rt::Error> {
    with_initiator(raw, |initiator| {
        initiator.list_passive_targets(modulation, max_targets)
    })
}

pub(super) fn poll_target(
    raw: *mut nfc_device,
    modulations: &[rt::Modulation],
    poll_nr: u8,
    period: u8,
) -> Result<Option<rt::Target>, rt::Error> {
    with_initiator(raw, |initiator| {
        initiator.poll_target(modulations, poll_nr, period)
    })
}

pub(super) fn select_dep_target(
    raw: *mut nfc_device,
    mode: rt::DepMode,
    baud_rate: rt::BaudRate,
    initiator: Option<&rt::DepInfo>,
    timeout: c_int,
) -> Result<Option<rt::Target>, rt::Error> {
    with_initiator(raw, |device| {
        device.select_dep_target(mode, baud_rate, initiator, timeout)
    })
}

pub(super) fn poll_dep_target(
    raw: *mut nfc_device,
    mode: rt::DepMode,
    baud_rate: rt::BaudRate,
    initiator: Option<&rt::DepInfo>,
    timeout: c_int,
) -> Result<Option<rt::Target>, rt::Error> {
    with_initiator(raw, |device| {
        device.poll_dep_target(mode, baud_rate, initiator, timeout)
    })
}

pub(super) fn deselect_target(raw: *mut nfc_device) -> Result<(), rt::Error> {
    with_initiator(raw, |initiator| initiator.deselect_target())
}

pub(super) fn target_is_present(
    raw: *mut nfc_device,
    target: Option<&rt::Target>,
) -> Result<bool, rt::Error> {
    with_initiator(raw, |initiator| initiator.target_is_present(target))
}

pub(super) fn target_init(
    raw: *mut nfc_device,
    target: &mut rt::Target,
    rx: &mut [u8],
    timeout: c_int,
) -> Result<usize, rt::Error> {
    with_target(raw, |device| device.init(target, rx, timeout))
}

pub(super) fn transceive_bytes(
    raw: *mut nfc_device,
    tx: &[u8],
    rx: &mut [u8],
    timeout: c_int,
) -> Result<usize, rt::Error> {
    with_initiator(raw, |initiator| initiator.transceive_bytes(tx, rx, timeout))
}

pub(super) fn transceive_bits(
    raw: *mut nfc_device,
    tx: &[u8],
    tx_bits_len: usize,
    tx_parity: Option<&[u8]>,
    rx: &mut [u8],
    rx_parity: Option<&mut [u8]>,
) -> Result<usize, rt::Error> {
    with_initiator(raw, |initiator| {
        initiator.transceive_bits(tx, tx_bits_len, tx_parity, rx, rx_parity)
    })
}

pub(super) fn transceive_bytes_timed(
    raw: *mut nfc_device,
    tx: &[u8],
    rx: &mut [u8],
) -> Result<(usize, u32), rt::Error> {
    with_initiator(raw, |initiator| initiator.transceive_bytes_timed(tx, rx))
}

pub(super) fn transceive_bits_timed(
    raw: *mut nfc_device,
    tx: &[u8],
    tx_bits_len: usize,
    tx_parity: Option<&[u8]>,
    rx: &mut [u8],
    rx_parity: Option<&mut [u8]>,
) -> Result<(usize, u32), rt::Error> {
    with_initiator(raw, |initiator| {
        initiator.transceive_bits_timed(tx, tx_bits_len, tx_parity, rx, rx_parity)
    })
}

pub(super) fn target_send_bytes(
    raw: *mut nfc_device,
    tx: &[u8],
    timeout: c_int,
) -> Result<usize, rt::Error> {
    with_target(raw, |device| device.send_bytes(tx, timeout))
}

pub(super) fn target_receive_bytes(
    raw: *mut nfc_device,
    rx: &mut [u8],
    timeout: c_int,
) -> Result<usize, rt::Error> {
    with_target(raw, |device| device.receive_bytes(rx, timeout))
}

pub(super) fn target_send_bits(
    raw: *mut nfc_device,
    tx: &[u8],
    tx_bits_len: usize,
    tx_parity: Option<&[u8]>,
) -> Result<usize, rt::Error> {
    with_target(raw, |device| device.send_bits(tx, tx_bits_len, tx_parity))
}

pub(super) fn target_receive_bits(
    raw: *mut nfc_device,
    rx: &mut [u8],
    rx_parity: Option<&mut [u8]>,
) -> Result<usize, rt::Error> {
    with_target(raw, |device| device.receive_bits(rx, rx_parity))
}

pub(super) fn abort_command(raw: *mut nfc_device) -> Result<(), rt::Error> {
    with_initiator(raw, |initiator| initiator.abort_command())
}

pub(super) fn idle(raw: *mut nfc_device) -> Result<(), rt::Error> {
    with_initiator(raw, |initiator| initiator.idle())
}
