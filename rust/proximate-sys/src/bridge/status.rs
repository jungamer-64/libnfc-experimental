use crate::ffi_strings::device_error_message_cstr;
use crate::ffi_support::{as_mut, as_ref};
use crate::lifecycle::nfc_device;
use libc::{c_char, c_int};
use proximate_driver as rt;

pub(crate) const NFC_EINVARG: c_int = -2;
pub(crate) const NFC_EDEVNOTSUPP: c_int = -3;
pub(crate) const NFC_ENOTSUCHDEV: c_int = -4;
pub(crate) const NFC_EOVFLOW: c_int = -5;
pub(crate) const NFC_ENOTIMPL: c_int = -8;
pub(crate) const NFC_ESOFT: c_int = -80;

pub(crate) fn error_to_status(error: &rt::Error) -> c_int {
    match error {
        rt::Error::InvalidArgument(_) => NFC_EINVARG,
        rt::Error::InvalidEncoding(_) => NFC_EINVARG,
        rt::Error::BufferTooSmall { .. } => NFC_EOVFLOW,
        rt::Error::InvalidConnectionString(_) => NFC_EINVARG,
        rt::Error::DriverNotFound(_) => NFC_ENOTSUCHDEV,
        rt::Error::DriverOpenFailed(_) => NFC_ESOFT,
        rt::Error::MissingCapability(_) => NFC_EDEVNOTSUPP,
        rt::Error::UnsupportedOperation(_) => NFC_ENOTIMPL,
        rt::Error::DeviceOperationFailed { code, .. } => *code,
    }
}

pub(crate) fn set_device_last_error(device: *mut nfc_device, value: c_int) {
    if let Some(device) = unsafe { as_mut(device) } {
        device.last_error = value;
    }
}

pub(crate) unsafe fn device_last_error(device: *const nfc_device) -> c_int {
    unsafe { as_ref(device) }
        .map(|device| device.last_error)
        .unwrap_or(0)
}

pub(crate) fn reset_device_last_error(device: *mut nfc_device) {
    set_device_last_error(device, 0);
}

pub(crate) fn error_message_ptr(code: c_int) -> *const c_char {
    device_error_message_cstr(code).as_ptr()
}

pub(crate) fn invalid_argument_status(device: *mut nfc_device) -> c_int {
    set_device_last_error(device, NFC_EINVARG);
    NFC_EINVARG
}

pub(crate) fn soft_error_status(device: *mut nfc_device) -> c_int {
    set_device_last_error(device, NFC_ESOFT);
    NFC_ESOFT
}

pub(crate) fn runtime_result_status(
    device: *mut nfc_device,
    error: &rt::Error,
    unsupported_as_zero: bool,
) -> c_int {
    let status = error_to_status(error);
    set_device_last_error(device, status);
    if unsupported_as_zero && status == NFC_EDEVNOTSUPP {
        0
    } else {
        status
    }
}

pub(crate) fn unsupported_driver_operation(device: *mut nfc_device) -> c_int {
    set_device_last_error(device, NFC_EDEVNOTSUPP);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_status_mapping_matches_libnfc_contract() {
        assert_eq!(
            error_to_status(&rt::Error::InvalidArgument("bad")),
            NFC_EINVARG
        );
        assert_eq!(
            error_to_status(&rt::Error::BufferTooSmall {
                needed: 8,
                available: 4,
            }),
            NFC_EOVFLOW
        );
        assert_eq!(
            error_to_status(&rt::Error::DriverNotFound("missing".into())),
            NFC_ENOTSUCHDEV
        );
        assert_eq!(
            error_to_status(&rt::Error::MissingCapability("open")),
            NFC_EDEVNOTSUPP
        );
        assert_eq!(
            error_to_status(&rt::Error::UnsupportedOperation("open")),
            NFC_ENOTIMPL
        );
        assert_eq!(
            error_to_status(&rt::Error::DeviceOperationFailed {
                operation: "x",
                code: -6,
            }),
            -6
        );
    }

    #[test]
    fn status_helpers_update_last_error() {
        let mut device = unsafe { std::mem::zeroed::<nfc_device>() };
        set_device_last_error(&mut device, -7);
        assert_eq!(unsafe { device_last_error(&device) }, -7);

        reset_device_last_error(&mut device);
        assert_eq!(unsafe { device_last_error(&device) }, 0);

        assert_eq!(invalid_argument_status(&mut device), NFC_EINVARG);
        assert_eq!(device.last_error, NFC_EINVARG);

        assert_eq!(soft_error_status(&mut device), NFC_ESOFT);
        assert_eq!(device.last_error, NFC_ESOFT);

        assert_eq!(unsupported_driver_operation(&mut device), 0);
        assert_eq!(device.last_error, NFC_EDEVNOTSUPP);
    }
}
