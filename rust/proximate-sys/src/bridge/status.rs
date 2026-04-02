use crate::ffi_support::as_mut;
use crate::lifecycle::nfc_device;
use libc::c_int;
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
}
