use super::operations::{nfc_target_init, nfc_target_receive_bytes, nfc_target_send_bytes};
use crate::bridge::status::{NFC_EINVARG, NFC_ESOFT};
use crate::ffi_catch_unwind_int;
use crate::ffi_support::as_mut;
use crate::ffi_types::nfc_target;
use crate::lifecycle::nfc_device;
use libc::{c_int, c_void, size_t};

pub(super) const ISO7816_SHORT_C_APDU_MAX_LEN: usize = 261;
pub(super) const ISO7816_SHORT_R_APDU_MAX_LEN: usize = 258;

#[allow(non_camel_case_types)]
pub type nfc_emulation_io_fn = Option<
    unsafe extern "C" fn(
        emulator: *mut nfc_emulator,
        data_in: *const u8,
        data_in_len: size_t,
        data_out: *mut u8,
        data_out_len: size_t,
    ) -> c_int,
>;

#[repr(C)]
pub struct nfc_emulator {
    pub target: *mut nfc_target,
    pub state_machine: *mut nfc_emulation_state_machine,
    pub user_data: *mut c_void,
}

#[repr(C)]
pub struct nfc_emulation_state_machine {
    pub io: nfc_emulation_io_fn,
    pub data: *mut c_void,
}

pub(crate) unsafe fn nfc_emulate_target(
    device: *mut nfc_device,
    emulator: *mut nfc_emulator,
    timeout: c_int,
) -> c_int {
    ffi_catch_unwind_int("nfc_emulate_target", NFC_ESOFT, || unsafe {
        let Some(emulator_ref) = as_mut(emulator) else {
            return NFC_EINVARG;
        };
        let Some(state_machine) = as_mut(emulator_ref.state_machine) else {
            return NFC_EINVARG;
        };
        let Some(callback) = state_machine.io else {
            return NFC_EINVARG;
        };
        if emulator_ref.target.is_null() {
            return NFC_EINVARG;
        }

        let mut rx = [0u8; ISO7816_SHORT_R_APDU_MAX_LEN];
        let mut tx = [0u8; ISO7816_SHORT_C_APDU_MAX_LEN];

        let init_len = nfc_target_init(
            device,
            emulator_ref.target,
            rx.as_mut_ptr(),
            rx.len(),
            timeout,
        );
        if init_len < 0 {
            return init_len;
        }

        let mut rx_len = init_len as usize;
        let mut io_res = init_len;
        while io_res >= 0 {
            io_res = callback(emulator, rx.as_ptr(), rx_len, tx.as_mut_ptr(), tx.len());
            if io_res > 0 {
                let sent = nfc_target_send_bytes(device, tx.as_ptr(), io_res as usize, timeout);
                if sent < 0 {
                    return sent;
                }
            }
            if io_res >= 0 {
                let received = nfc_target_receive_bytes(device, rx.as_mut_ptr(), rx.len(), timeout);
                if received < 0 {
                    return received;
                }
                rx_len = received as usize;
            }
        }

        io_res
    })
}
