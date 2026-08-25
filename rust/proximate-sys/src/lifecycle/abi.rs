use crate::c_abi::types::{
    nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_mode, nfc_modulation, nfc_modulation_type,
    nfc_property, nfc_target,
};
use crate::c_boundary::NFC_BUFSIZE_CONNSTRING;
use libc::{c_char, c_int};

pub(crate) const DEVICE_NAME_LENGTH: usize = 256;
pub(crate) const MAX_USER_DEFINED_DEVICES: usize = 4;
pub(crate) const NFC_DRIVER_NAME_MAX: usize = 64;

#[allow(non_camel_case_types)]
pub type nfc_connstring = [c_char; NFC_BUFSIZE_CONNSTRING];

/// Opaque C ABI marker. The pointed-to allocation is a private `AbiContext`.
#[allow(non_camel_case_types)]
#[repr(C)]
pub struct nfc_context {
    _opaque: [u8; 0],
}

/// Opaque C ABI marker. The pointed-to allocation is a private `AbiDevice`.
#[allow(non_camel_case_types)]
#[repr(C)]
pub struct nfc_device {
    _opaque: [u8; 0],
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub(crate) struct scan_type_enum(c_int);

impl scan_type_enum {
    pub(crate) const NOT_INTRUSIVE: Self = Self(0);
    pub(crate) const INTRUSIVE: Self = Self(1);
    pub(crate) const NOT_AVAILABLE: Self = Self(2);

    #[cfg(test)]
    pub(crate) const fn from_raw(raw: c_int) -> Self {
        Self(raw)
    }
}

type ScanFn = Option<unsafe extern "C" fn(*const nfc_context, *mut nfc_connstring, usize) -> usize>;
type OpenFn = Option<unsafe extern "C" fn(*const nfc_context, *const c_char) -> *mut nfc_device>;
type CloseFn = Option<unsafe extern "C" fn(*mut nfc_device)>;
type StrerrorFn = Option<unsafe extern "C" fn(*const nfc_device) -> *const c_char>;
type DeviceIntFn = Option<unsafe extern "C" fn(*mut nfc_device) -> c_int>;
type SelectPassiveFn = Option<
    unsafe extern "C" fn(
        *mut nfc_device,
        nfc_modulation,
        *const u8,
        usize,
        *mut nfc_target,
    ) -> c_int,
>;
type PollFn = Option<
    unsafe extern "C" fn(
        *mut nfc_device,
        *const nfc_modulation,
        usize,
        u8,
        u8,
        *mut nfc_target,
    ) -> c_int,
>;
type SelectDepFn = Option<
    unsafe extern "C" fn(
        *mut nfc_device,
        nfc_dep_mode,
        nfc_baud_rate,
        *const nfc_dep_info,
        *mut nfc_target,
        c_int,
    ) -> c_int,
>;
type TargetPresentFn = Option<unsafe extern "C" fn(*mut nfc_device, *const nfc_target) -> c_int>;
type TransceiveBytesFn =
    Option<unsafe extern "C" fn(*mut nfc_device, *const u8, usize, *mut u8, usize, c_int) -> c_int>;
type TransceiveBitsFn = Option<
    unsafe extern "C" fn(*mut nfc_device, *const u8, usize, *const u8, *mut u8, *mut u8) -> c_int,
>;
type TransceiveBytesTimedFn = Option<
    unsafe extern "C" fn(*mut nfc_device, *const u8, usize, *mut u8, usize, *mut u32) -> c_int,
>;
type TransceiveBitsTimedFn = Option<
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
type TargetInitFn =
    Option<unsafe extern "C" fn(*mut nfc_device, *mut nfc_target, *mut u8, usize, c_int) -> c_int>;
type TargetSendBytesFn =
    Option<unsafe extern "C" fn(*mut nfc_device, *const u8, usize, c_int) -> c_int>;
type TargetReceiveBytesFn =
    Option<unsafe extern "C" fn(*mut nfc_device, *mut u8, usize, c_int) -> c_int>;
type TargetSendBitsFn =
    Option<unsafe extern "C" fn(*mut nfc_device, *const u8, usize, *const u8) -> c_int>;
type TargetReceiveBitsFn =
    Option<unsafe extern "C" fn(*mut nfc_device, *mut u8, usize, *mut u8) -> c_int>;
type SetPropertyBoolFn = Option<unsafe extern "C" fn(*mut nfc_device, nfc_property, bool) -> c_int>;
type SetPropertyIntFn = Option<unsafe extern "C" fn(*mut nfc_device, nfc_property, c_int) -> c_int>;
type SupportedModulationsFn = Option<
    unsafe extern "C" fn(*mut nfc_device, nfc_mode, *mut *const nfc_modulation_type) -> c_int,
>;
type SupportedBaudRatesFn = Option<
    unsafe extern "C" fn(
        *mut nfc_device,
        nfc_mode,
        nfc_modulation_type,
        *mut *const nfc_baud_rate,
    ) -> c_int,
>;
type InformationFn = Option<unsafe extern "C" fn(*mut nfc_device, *mut *mut c_char) -> c_int>;

/// Registration descriptor consumed by `nfc_register_driver`.
///
/// The descriptor is snapshotted at registration; the C function addresses
/// must remain executable until the next `nfc_exit` clears the registry.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy)]
#[repr(C)]
pub struct nfc_driver {
    pub(crate) name: *const c_char,
    pub(crate) scan_type: scan_type_enum,
    pub(crate) scan: ScanFn,
    pub(crate) open: OpenFn,
    pub(crate) close: CloseFn,
    pub(crate) strerror: StrerrorFn,
    pub(crate) initiator_init: DeviceIntFn,
    pub(crate) initiator_init_secure_element: DeviceIntFn,
    pub(crate) initiator_select_passive_target: SelectPassiveFn,
    pub(crate) initiator_poll_target: PollFn,
    pub(crate) initiator_select_dep_target: SelectDepFn,
    pub(crate) initiator_deselect_target: DeviceIntFn,
    pub(crate) initiator_transceive_bytes: TransceiveBytesFn,
    pub(crate) initiator_transceive_bits: TransceiveBitsFn,
    pub(crate) initiator_transceive_bytes_timed: TransceiveBytesTimedFn,
    pub(crate) initiator_transceive_bits_timed: TransceiveBitsTimedFn,
    pub(crate) initiator_target_is_present: TargetPresentFn,
    pub(crate) target_init: TargetInitFn,
    pub(crate) target_send_bytes: TargetSendBytesFn,
    pub(crate) target_receive_bytes: TargetReceiveBytesFn,
    pub(crate) target_send_bits: TargetSendBitsFn,
    pub(crate) target_receive_bits: TargetReceiveBitsFn,
    pub(crate) device_set_property_bool: SetPropertyBoolFn,
    pub(crate) device_set_property_int: SetPropertyIntFn,
    pub(crate) get_supported_modulation: SupportedModulationsFn,
    pub(crate) get_supported_baud_rate: SupportedBaudRatesFn,
    pub(crate) device_get_information_about: InformationFn,
    pub(crate) abort_command: DeviceIntFn,
    pub(crate) idle: DeviceIntFn,
    pub(crate) powerdown: DeviceIntFn,
}
