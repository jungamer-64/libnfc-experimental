use crate::ffi_types::{
    nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_mode, nfc_modulation, nfc_modulation_type,
    nfc_property, nfc_target,
};
use crate::lifecycle::nfc_device;
use libc::{c_char, c_int, size_t};

#[allow(non_upper_case_globals)]
pub const Diagnose: u8 = 0x00;
pub const PN53x_EXTENDED_FRAME__DATA_MAX_LEN: usize = 264;
pub const PN53x_ACK_FRAME__LEN: usize = 6;
pub const PN53X_REG_CIU_TxMode: u16 = 0x6302;

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub enum pn532_sam_mode {
    PSM_NORMAL = 0x01,
    PSM_VIRTUAL_CARD = 0x02,
    PSM_WIRED_CARD = 0x03,
    PSM_DUAL_CARD = 0x04,
}

#[allow(non_camel_case_types)]
pub type pn53x_io_send_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, *const u8, usize, c_int) -> c_int>;
#[allow(non_camel_case_types)]
pub type pn53x_io_receive_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, *mut u8, usize, c_int) -> c_int>;

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct pn53x_io {
    pub send: pn53x_io_send_fn,
    pub receive: pn53x_io_receive_fn,
}

unsafe extern "C" {
    pub static pn53x_ack_frame: [u8; PN53x_ACK_FRAME__LEN];
    pub static pn53x_nack_frame: [u8; PN53x_ACK_FRAME__LEN];

    pub fn pn53x_init(device: *mut nfc_device) -> c_int;
    pub fn pn53x_check_communication(device: *mut nfc_device) -> c_int;
    pub fn pn53x_set_property_int(
        device: *mut nfc_device,
        property: nfc_property,
        value: c_int,
    ) -> c_int;
    pub fn pn53x_set_property_bool(
        device: *mut nfc_device,
        property: nfc_property,
        enabled: bool,
    ) -> c_int;
    pub fn pn53x_idle(device: *mut nfc_device) -> c_int;
    pub fn pn53x_initiator_init(device: *mut nfc_device) -> c_int;
    pub fn pn532_initiator_init_secure_element(device: *mut nfc_device) -> c_int;
    pub fn pn53x_initiator_select_passive_target(
        device: *mut nfc_device,
        modulation: nfc_modulation,
        init_data: *const u8,
        init_data_len: size_t,
        target: *mut nfc_target,
    ) -> c_int;
    pub fn pn53x_initiator_poll_target(
        device: *mut nfc_device,
        modulations: *const nfc_modulation,
        modulations_len: size_t,
        poll_nr: u8,
        period: u8,
        target: *mut nfc_target,
    ) -> c_int;
    pub fn pn53x_initiator_select_dep_target(
        device: *mut nfc_device,
        mode: nfc_dep_mode,
        baud_rate: nfc_baud_rate,
        initiator: *const nfc_dep_info,
        target: *mut nfc_target,
        timeout: c_int,
    ) -> c_int;
    pub fn pn53x_initiator_transceive_bits(
        device: *mut nfc_device,
        tx: *const u8,
        tx_bits_len: size_t,
        tx_parity: *const u8,
        rx: *mut u8,
        rx_parity: *mut u8,
    ) -> c_int;
    pub fn pn53x_initiator_transceive_bytes(
        device: *mut nfc_device,
        tx: *const u8,
        tx_len: size_t,
        rx: *mut u8,
        rx_len: size_t,
        timeout: c_int,
    ) -> c_int;
    pub fn pn53x_initiator_transceive_bits_timed(
        device: *mut nfc_device,
        tx: *const u8,
        tx_bits_len: size_t,
        tx_parity: *const u8,
        rx: *mut u8,
        rx_parity: *mut u8,
        cycles: *mut u32,
    ) -> c_int;
    pub fn pn53x_initiator_transceive_bytes_timed(
        device: *mut nfc_device,
        tx: *const u8,
        tx_len: size_t,
        rx: *mut u8,
        rx_len: size_t,
        cycles: *mut u32,
    ) -> c_int;
    pub fn pn53x_initiator_deselect_target(device: *mut nfc_device) -> c_int;
    pub fn pn53x_initiator_target_is_present(
        device: *mut nfc_device,
        target: *const nfc_target,
    ) -> c_int;
    pub fn pn53x_target_init(
        device: *mut nfc_device,
        target: *mut nfc_target,
        rx: *mut u8,
        rx_len: size_t,
        timeout: c_int,
    ) -> c_int;
    pub fn pn53x_target_receive_bits(
        device: *mut nfc_device,
        rx: *mut u8,
        rx_len: size_t,
        rx_parity: *mut u8,
    ) -> c_int;
    pub fn pn53x_target_receive_bytes(
        device: *mut nfc_device,
        rx: *mut u8,
        rx_len: size_t,
        timeout: c_int,
    ) -> c_int;
    pub fn pn53x_target_send_bits(
        device: *mut nfc_device,
        tx: *const u8,
        tx_bits_len: size_t,
        tx_parity: *const u8,
    ) -> c_int;
    pub fn pn53x_target_send_bytes(
        device: *mut nfc_device,
        tx: *const u8,
        tx_len: size_t,
        timeout: c_int,
    ) -> c_int;
    pub fn pn53x_strerror(device: *const nfc_device) -> *const c_char;
    pub fn pn53x_PowerDown(device: *mut nfc_device) -> c_int;
    pub fn pn53x_check_ack_frame(
        device: *mut nfc_device,
        rx_frame: *const u8,
        rx_frame_len: size_t,
    ) -> c_int;
    pub fn pn53x_build_frame(
        frame: *mut u8,
        frame_len: *mut size_t,
        data: *const u8,
        data_len: size_t,
    ) -> c_int;
    pub fn pn53x_get_supported_modulation(
        device: *mut nfc_device,
        mode: nfc_mode,
        supported_modulation_types: *mut *const nfc_modulation_type,
    ) -> c_int;
    pub fn pn53x_get_supported_baud_rate(
        device: *mut nfc_device,
        mode: nfc_mode,
        modulation_type: nfc_modulation_type,
        supported_baud_rates: *mut *const nfc_baud_rate,
    ) -> c_int;
    pub fn pn53x_get_information_about(device: *mut nfc_device, buffer: *mut *mut c_char) -> c_int;
    pub fn pn53x_data_new(device: *mut nfc_device, io: *const pn53x_io) -> *mut libc::c_void;
    pub fn pn53x_data_free(device: *mut nfc_device);
}
