use crate::c_api_impl::NFC_BUFSIZE_CONNSTRING;
use crate::ffi_types::{
    nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_modulation, nfc_modulation_type, nfc_property,
    nfc_target,
};
use libc::{c_char, c_int, c_uint, c_void};

/// cbindgen:no-export
pub const DEVICE_NAME_LENGTH: usize = 256;
/// cbindgen:no-export
pub const MAX_USER_DEFINED_DEVICES: usize = 4;
/// cbindgen:no-export
pub const NFC_DRIVER_NAME_MAX: usize = 64;

#[allow(non_camel_case_types)]
pub type nfc_connstring = [c_char; NFC_BUFSIZE_CONNSTRING];

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub enum scan_type_enum {
    NOT_INTRUSIVE = 0,
    INTRUSIVE = 1,
    NOT_AVAILABLE = 2,
}

#[allow(non_camel_case_types)]
pub type nfc_driver_scan_fn =
    Option<unsafe extern "C" fn(*const nfc_context, *mut nfc_connstring, usize) -> usize>;
#[allow(non_camel_case_types)]
pub type nfc_driver_open_fn =
    Option<unsafe extern "C" fn(*const nfc_context, *const c_char) -> *mut nfc_device>;
#[allow(non_camel_case_types)]
pub type nfc_driver_close_fn = Option<unsafe extern "C" fn(*mut nfc_device)>;
#[allow(non_camel_case_types)]
pub type nfc_driver_strerror_fn = Option<unsafe extern "C" fn(*const nfc_device) -> *const c_char>;
#[allow(non_camel_case_types)]
pub type nfc_driver_initiator_init_fn = Option<unsafe extern "C" fn(*mut nfc_device) -> c_int>;
#[allow(non_camel_case_types)]
pub type nfc_driver_initiator_init_secure_element_fn =
    Option<unsafe extern "C" fn(*mut nfc_device) -> c_int>;
#[allow(non_camel_case_types)]
pub type nfc_driver_initiator_select_passive_target_fn = Option<
    unsafe extern "C" fn(
        *mut nfc_device,
        nfc_modulation,
        *const u8,
        usize,
        *mut nfc_target,
    ) -> c_int,
>;
#[allow(non_camel_case_types)]
pub type nfc_driver_initiator_poll_target_fn = Option<
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
pub type nfc_driver_initiator_select_dep_target_fn = Option<
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
pub type nfc_driver_initiator_deselect_target_fn =
    Option<unsafe extern "C" fn(*mut nfc_device) -> c_int>;
#[allow(non_camel_case_types)]
pub type nfc_driver_initiator_transceive_bytes_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, *const u8, usize, *mut u8, usize, c_int) -> c_int>;
#[allow(non_camel_case_types)]
pub type nfc_driver_initiator_transceive_bits_fn = Option<
    unsafe extern "C" fn(*mut nfc_device, *const u8, usize, *const u8, *mut u8, *mut u8) -> c_int,
>;
#[allow(non_camel_case_types)]
pub type nfc_driver_initiator_transceive_bytes_timed_fn = Option<
    unsafe extern "C" fn(*mut nfc_device, *const u8, usize, *mut u8, usize, *mut u32) -> c_int,
>;
#[allow(non_camel_case_types)]
pub type nfc_driver_initiator_transceive_bits_timed_fn = Option<
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
pub type nfc_driver_initiator_target_is_present_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, *const nfc_target) -> c_int>;
#[allow(non_camel_case_types)]
pub type nfc_driver_target_init_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, *mut nfc_target, *mut u8, usize, c_int) -> c_int>;
#[allow(non_camel_case_types)]
pub type nfc_driver_target_send_bytes_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, *const u8, usize, c_int) -> c_int>;
#[allow(non_camel_case_types)]
pub type nfc_driver_target_receive_bytes_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, *mut u8, usize, c_int) -> c_int>;
#[allow(non_camel_case_types)]
pub type nfc_driver_target_send_bits_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, *const u8, usize, *const u8) -> c_int>;
#[allow(non_camel_case_types)]
pub type nfc_driver_target_receive_bits_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, *mut u8, usize, *mut u8) -> c_int>;
#[allow(non_camel_case_types)]
pub type nfc_driver_device_set_property_bool_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, nfc_property, bool) -> c_int>;
#[allow(non_camel_case_types)]
pub type nfc_driver_device_set_property_int_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, nfc_property, c_int) -> c_int>;
#[allow(non_camel_case_types)]
pub type nfc_driver_get_supported_modulation_fn = Option<
    unsafe extern "C" fn(
        *mut nfc_device,
        crate::ffi_types::nfc_mode,
        *mut *const nfc_modulation_type,
    ) -> c_int,
>;
#[allow(non_camel_case_types)]
pub type nfc_driver_get_supported_baud_rate_fn = Option<
    unsafe extern "C" fn(
        *mut nfc_device,
        crate::ffi_types::nfc_mode,
        nfc_modulation_type,
        *mut *const nfc_baud_rate,
    ) -> c_int,
>;
#[allow(non_camel_case_types)]
pub type nfc_driver_device_get_information_about_fn =
    Option<unsafe extern "C" fn(*mut nfc_device, *mut *mut c_char) -> c_int>;
#[allow(non_camel_case_types)]
pub type nfc_driver_device_control_fn = Option<unsafe extern "C" fn(*mut nfc_device) -> c_int>;

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct nfc_driver {
    pub name: *const c_char,
    pub scan_type: scan_type_enum,
    pub scan: nfc_driver_scan_fn,
    pub open: nfc_driver_open_fn,
    pub close: nfc_driver_close_fn,
    pub strerror: nfc_driver_strerror_fn,
    pub initiator_init: nfc_driver_initiator_init_fn,
    pub initiator_init_secure_element: nfc_driver_initiator_init_secure_element_fn,
    pub initiator_select_passive_target: nfc_driver_initiator_select_passive_target_fn,
    pub initiator_poll_target: nfc_driver_initiator_poll_target_fn,
    pub initiator_select_dep_target: nfc_driver_initiator_select_dep_target_fn,
    pub initiator_deselect_target: nfc_driver_initiator_deselect_target_fn,
    pub initiator_transceive_bytes: nfc_driver_initiator_transceive_bytes_fn,
    pub initiator_transceive_bits: nfc_driver_initiator_transceive_bits_fn,
    pub initiator_transceive_bytes_timed: nfc_driver_initiator_transceive_bytes_timed_fn,
    pub initiator_transceive_bits_timed: nfc_driver_initiator_transceive_bits_timed_fn,
    pub initiator_target_is_present: nfc_driver_initiator_target_is_present_fn,
    pub target_init: nfc_driver_target_init_fn,
    pub target_send_bytes: nfc_driver_target_send_bytes_fn,
    pub target_receive_bytes: nfc_driver_target_receive_bytes_fn,
    pub target_send_bits: nfc_driver_target_send_bits_fn,
    pub target_receive_bits: nfc_driver_target_receive_bits_fn,
    pub device_set_property_bool: nfc_driver_device_set_property_bool_fn,
    pub device_set_property_int: nfc_driver_device_set_property_int_fn,
    pub get_supported_modulation: nfc_driver_get_supported_modulation_fn,
    pub get_supported_baud_rate: nfc_driver_get_supported_baud_rate_fn,
    pub device_get_information_about: nfc_driver_device_get_information_about_fn,
    pub abort_command: nfc_driver_device_control_fn,
    pub idle: nfc_driver_device_control_fn,
    pub powerdown: nfc_driver_device_control_fn,
}

unsafe impl Sync for nfc_driver {}

#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[repr(C)]
pub struct nfc_user_defined_device {
    pub name: [c_char; DEVICE_NAME_LENGTH],
    pub connstring: [c_char; NFC_BUFSIZE_CONNSTRING],
    pub optional: bool,
}

#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[repr(C)]
pub struct nfc_context {
    pub allow_autoscan: bool,
    pub allow_intrusive_scan: bool,
    pub log_level: u32,
    pub user_defined_devices: [nfc_user_defined_device; MAX_USER_DEFINED_DEVICES],
    pub user_defined_device_count: c_uint,
    pub runtime_data: *mut c_void,
}

#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[repr(C)]
pub struct nfc_device {
    pub context: *const nfc_context,
    pub driver: *const nfc_driver,
    pub driver_data: *mut c_void,
    pub chip_data: *mut c_void,
    pub name: [c_char; DEVICE_NAME_LENGTH],
    pub connstring: [c_char; NFC_BUFSIZE_CONNSTRING],
    pub bCrc: bool,
    pub bPar: bool,
    pub bEasyFraming: bool,
    pub bInfiniteSelect: bool,
    pub bAutoIso14443_4: bool,
    pub btSupportByte: u8,
    pub last_error: c_int,
}
