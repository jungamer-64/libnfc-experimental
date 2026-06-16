use crate::c_abi::types::{
    nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_modulation, nfc_modulation_type, nfc_property,
    nfc_target,
};
use crate::c_boundary::NFC_BUFSIZE_CONNSTRING;
use libc::{c_char, c_int, c_uint, c_void};

/// cbindgen:no-export
pub(crate) const DEVICE_NAME_LENGTH: usize = 256;
/// cbindgen:no-export
pub(crate) const MAX_USER_DEFINED_DEVICES: usize = 4;
/// cbindgen:no-export
pub(crate) const NFC_DRIVER_NAME_MAX: usize = 64;

#[allow(non_camel_case_types)]
pub type nfc_connstring = [c_char; NFC_BUFSIZE_CONNSTRING];

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
#[allow(dead_code)]
pub(crate) enum scan_type_enum {
    NOT_INTRUSIVE = 0,
    #[expect(clippy::upper_case_acronyms)]
    INTRUSIVE = 1,
    NOT_AVAILABLE = 2,
}

#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_scan_fn =
    Option<unsafe extern "C" fn(*const nfc_context, *mut nfc_connstring, usize) -> usize>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_open_fn =
    Option<unsafe extern "C" fn(*const nfc_context, *const c_char) -> *mut nfc_device>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_close_fn = Option<unsafe extern "C" fn(*mut nfc_device)>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_strerror_fn =
    Option<unsafe extern "C" fn(*const nfc_device) -> *const c_char>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_initiator_init_fn =
    Option<unsafe extern "C" fn(*mut nfc_device) -> c_int>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_initiator_init_secure_element_fn =
    Option<unsafe extern "C" fn(*mut nfc_device) -> c_int>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_initiator_select_passive_target_fn = Option<
    unsafe extern "C" fn(
        *mut nfc_device,
        nfc_modulation,
        *const u8,
        usize,
        *mut nfc_target,
    ) -> c_int,
>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_initiator_poll_target_fn = Option<
    unsafe extern "C" fn(
        *mut nfc_device,
        *const nfc_modulation,
        usize,
        u8,
        u8,
        *mut nfc_target,
    ) -> c_int,
>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_initiator_select_dep_target_fn = Option<
    unsafe extern "C" fn(
        *mut nfc_device,
        nfc_dep_mode,
        nfc_baud_rate,
        *const nfc_dep_info,
        *mut nfc_target,
        c_int,
    ) -> c_int,
>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_initiator_deselect_target_fn =
    Option<unsafe extern "C" fn(*mut nfc_device) -> c_int>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_initiator_transceive_bytes_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, *const u8, usize, *mut u8, usize, c_int) -> c_int>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_initiator_transceive_bits_fn = Option<
    unsafe extern "C" fn(*mut nfc_device, *const u8, usize, *const u8, *mut u8, *mut u8) -> c_int,
>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_initiator_transceive_bytes_timed_fn = Option<
    unsafe extern "C" fn(*mut nfc_device, *const u8, usize, *mut u8, usize, *mut u32) -> c_int,
>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_initiator_transceive_bits_timed_fn = Option<
    unsafe extern "C" fn(
        *mut nfc_device,
        *const u8,
        usize,
        *const u8,
        *mut u8,
        *mut u8,
        *mut u32,
    ) -> c_int,
>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_initiator_target_is_present_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, *const nfc_target) -> c_int>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_target_init_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, *mut nfc_target, *mut u8, usize, c_int) -> c_int>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_target_send_bytes_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, *const u8, usize, c_int) -> c_int>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_target_receive_bytes_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, *mut u8, usize, c_int) -> c_int>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_target_send_bits_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, *const u8, usize, *const u8) -> c_int>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_target_receive_bits_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, *mut u8, usize, *mut u8) -> c_int>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_device_set_property_bool_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, nfc_property, bool) -> c_int>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_device_set_property_int_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, nfc_property, c_int) -> c_int>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_get_supported_modulation_fn = Option<
    unsafe extern "C" fn(
        *mut nfc_device,
        crate::c_abi::types::nfc_mode,
        *mut *const nfc_modulation_type,
    ) -> c_int,
>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_get_supported_baud_rate_fn = Option<
    unsafe extern "C" fn(
        *mut nfc_device,
        crate::c_abi::types::nfc_mode,
        nfc_modulation_type,
        *mut *const nfc_baud_rate,
    ) -> c_int,
>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_device_get_information_about_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, *mut *mut c_char) -> c_int>;
#[allow(non_camel_case_types)]
pub(crate) type nfc_driver_device_control_fn =
    Option<unsafe extern "C" fn(*mut nfc_device) -> c_int>;

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct nfc_driver {
    pub(crate) name: *const c_char,
    pub(crate) scan_type: scan_type_enum,
    pub(crate) scan: nfc_driver_scan_fn,
    pub(crate) open: nfc_driver_open_fn,
    pub(crate) close: nfc_driver_close_fn,
    pub(crate) strerror: nfc_driver_strerror_fn,
    pub(crate) initiator_init: nfc_driver_initiator_init_fn,
    pub(crate) initiator_init_secure_element: nfc_driver_initiator_init_secure_element_fn,
    pub(crate) initiator_select_passive_target: nfc_driver_initiator_select_passive_target_fn,
    pub(crate) initiator_poll_target: nfc_driver_initiator_poll_target_fn,
    pub(crate) initiator_select_dep_target: nfc_driver_initiator_select_dep_target_fn,
    pub(crate) initiator_deselect_target: nfc_driver_initiator_deselect_target_fn,
    pub(crate) initiator_transceive_bytes: nfc_driver_initiator_transceive_bytes_fn,
    pub(crate) initiator_transceive_bits: nfc_driver_initiator_transceive_bits_fn,
    pub(crate) initiator_transceive_bytes_timed: nfc_driver_initiator_transceive_bytes_timed_fn,
    pub(crate) initiator_transceive_bits_timed: nfc_driver_initiator_transceive_bits_timed_fn,
    pub(crate) initiator_target_is_present: nfc_driver_initiator_target_is_present_fn,
    pub(crate) target_init: nfc_driver_target_init_fn,
    pub(crate) target_send_bytes: nfc_driver_target_send_bytes_fn,
    pub(crate) target_receive_bytes: nfc_driver_target_receive_bytes_fn,
    pub(crate) target_send_bits: nfc_driver_target_send_bits_fn,
    pub(crate) target_receive_bits: nfc_driver_target_receive_bits_fn,
    pub(crate) device_set_property_bool: nfc_driver_device_set_property_bool_fn,
    pub(crate) device_set_property_int: nfc_driver_device_set_property_int_fn,
    pub(crate) get_supported_modulation: nfc_driver_get_supported_modulation_fn,
    pub(crate) get_supported_baud_rate: nfc_driver_get_supported_baud_rate_fn,
    pub(crate) device_get_information_about: nfc_driver_device_get_information_about_fn,
    pub(crate) abort_command: nfc_driver_device_control_fn,
    pub(crate) idle: nfc_driver_device_control_fn,
    pub(crate) powerdown: nfc_driver_device_control_fn,
}

unsafe impl Sync for nfc_driver {}

#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[repr(C)]
pub(crate) struct nfc_user_defined_device {
    pub(crate) name: [c_char; DEVICE_NAME_LENGTH],
    pub(crate) connstring: [c_char; NFC_BUFSIZE_CONNSTRING],
    pub(crate) optional: bool,
}

#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[repr(C)]
pub struct nfc_context {
    pub(crate) allow_autoscan: bool,
    pub(crate) allow_intrusive_scan: bool,
    pub(crate) log_level: u32,
    pub(crate) user_defined_devices: [nfc_user_defined_device; MAX_USER_DEFINED_DEVICES],
    pub(crate) user_defined_device_count: c_uint,
    pub(crate) runtime_data: *mut c_void,
}

#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[repr(C)]
pub struct nfc_device {
    pub(crate) context: *const nfc_context,
    pub(crate) driver: *const nfc_driver,
    pub(crate) driver_data: *mut c_void,
    pub(crate) chip_data: *mut c_void,
    pub(crate) name: [c_char; DEVICE_NAME_LENGTH],
    pub(crate) connstring: [c_char; NFC_BUFSIZE_CONNSTRING],
    pub(crate) bCrc: bool,
    pub(crate) bPar: bool,
    pub(crate) bEasyFraming: bool,
    pub(crate) bInfiniteSelect: bool,
    pub(crate) bAutoIso14443_4: bool,
    pub(crate) btSupportByte: u8,
    pub(crate) last_error: c_int,
}
