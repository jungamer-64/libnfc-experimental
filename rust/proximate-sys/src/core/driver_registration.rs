use super::log_general_debug;
use crate::c_boundary::external_registry::push_driver;
use crate::c_boundary::status::NFC_ESOFT;
use crate::ffi_catch_unwind_int;
use crate::lifecycle::nfc_driver;
use libc::c_int;

pub(crate) unsafe fn nfc_register_driver(driver: *const nfc_driver) -> c_int {
    ffi_catch_unwind_int("nfc_register_driver", NFC_ESOFT, || unsafe {
        if driver.is_null() {
            log_general_debug("nfc_register_driver: NULL driver");
        }
        push_driver(driver)
    })
}
