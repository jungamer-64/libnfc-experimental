// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Rust-backed SPI helper preserving the legacy `spi_*` ABI.

#![allow(non_camel_case_types)]

#[cfg(test)]
use crate::buses::claimed_spi_port;
use crate::buses::{allocate_c_string_array, c_path_to_string, invalid_spi_port};
use libc::{c_char, c_int, c_void};
use std::ptr;

const NFC_SUCCESS: c_int = 0;
const NFC_EIO: c_int = -1;

pub type spi_port = *mut c_void;

#[cfg(all(not(test), target_os = "linux"))]
mod linux_impl {
    use super::*;
    use ::proximate_platform::spi::{
        SpiHandle, SpiIoError, SpiOpenError, list_ports as list_internal_ports,
    };

    unsafe fn port_ref<'a>(port: spi_port) -> Option<&'a mut SpiHandle> {
        unsafe { (port.cast::<SpiHandle>()).as_mut() }
    }

    pub unsafe fn spi_open(port_name: *const c_char) -> spi_port {
        let Some(port_name) = (unsafe { c_path_to_string(port_name) }) else {
            return invalid_spi_port();
        };
        match SpiHandle::open(&port_name) {
            Ok(handle) => Box::into_raw(Box::new(handle)).cast::<c_void>(),
            Err(SpiOpenError::InvalidPort) => invalid_spi_port(),
        }
    }

    pub unsafe fn spi_close(port: spi_port) {
        let raw = port.cast::<SpiHandle>();
        if raw.is_null() {
            return;
        }
        unsafe { drop(Box::from_raw(raw)) };
    }

    pub unsafe fn spi_set_speed(port: spi_port, speed: u32) {
        let Some(port) = (unsafe { port_ref(port) }) else {
            return;
        };
        let _ = port.set_speed(speed);
    }

    pub unsafe fn spi_set_mode(port: spi_port, mode: u32) {
        let Some(port) = (unsafe { port_ref(port) }) else {
            return;
        };
        let _ = port.set_mode(mode);
    }

    pub unsafe fn spi_get_speed(port: spi_port) -> u32 {
        let Some(port) = (unsafe { port_ref(port) }) else {
            return 0;
        };
        port.get_speed()
    }

    pub unsafe fn spi_send_receive(
        port: spi_port,
        tx: *const u8,
        tx_len: usize,
        rx: *mut u8,
        rx_len: usize,
        lsb_first: bool,
    ) -> c_int {
        let Some(port) = (unsafe { port_ref(port) }) else {
            return NFC_EIO;
        };
        let tx = if tx_len == 0 {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(tx, tx_len) }
        };
        let rx = if rx_len == 0 {
            &mut [][..]
        } else {
            unsafe { std::slice::from_raw_parts_mut(rx, rx_len) }
        };
        match port.send_receive(tx, rx, lsb_first) {
            Ok(()) => NFC_SUCCESS,
            Err(SpiIoError::Io) => NFC_EIO,
        }
    }

    pub unsafe fn spi_receive(
        port: spi_port,
        rx: *mut u8,
        rx_len: usize,
        lsb_first: bool,
    ) -> c_int {
        unsafe { spi_send_receive(port, ptr::null(), 0, rx, rx_len, lsb_first) }
    }

    pub unsafe fn spi_send(port: spi_port, tx: *const u8, tx_len: usize, lsb_first: bool) -> c_int {
        unsafe { spi_send_receive(port, tx, tx_len, ptr::null_mut(), 0, lsb_first) }
    }

    pub unsafe fn spi_list_ports() -> *mut *mut c_char {
        let ports = list_internal_ports()
            .into_iter()
            .map(|port| port.into_bytes())
            .collect::<Vec<_>>();
        unsafe { allocate_c_string_array(&ports) }
    }

    #[allow(dead_code)]
    pub const SPI_MODE_0_VALUE: u32 = 0;
}

#[cfg(all(not(test), target_os = "linux"))]
pub use linux_impl::{
    spi_close, spi_get_speed, spi_list_ports, spi_open, spi_receive, spi_send, spi_send_receive,
    spi_set_mode, spi_set_speed,
};

#[cfg(all(not(test), not(target_os = "linux")))]
pub unsafe fn spi_open(_port_name: *const c_char) -> spi_port {
    invalid_spi_port()
}

#[cfg(all(not(test), not(target_os = "linux")))]
pub unsafe fn spi_close(_port: spi_port) {}

#[cfg(all(not(test), not(target_os = "linux")))]
pub unsafe fn spi_set_speed(_port: spi_port, _speed: u32) {}

#[cfg(all(not(test), not(target_os = "linux")))]
pub unsafe fn spi_set_mode(_port: spi_port, _mode: u32) {}

#[cfg(all(not(test), not(target_os = "linux")))]
pub unsafe fn spi_get_speed(_port: spi_port) -> u32 {
    0
}

#[cfg(all(not(test), not(target_os = "linux")))]
pub unsafe fn spi_receive(
    _port: spi_port,
    _rx: *mut u8,
    _rx_len: usize,
    _lsb_first: bool,
) -> c_int {
    NFC_EIO
}

#[cfg(all(not(test), not(target_os = "linux")))]
pub unsafe fn spi_send(_port: spi_port, _tx: *const u8, _tx_len: usize, _lsb_first: bool) -> c_int {
    NFC_EIO
}

#[cfg(all(not(test), not(target_os = "linux")))]
pub unsafe fn spi_send_receive(
    _port: spi_port,
    _tx: *const u8,
    _tx_len: usize,
    _rx: *mut u8,
    _rx_len: usize,
    _lsb_first: bool,
) -> c_int {
    NFC_EIO
}

#[cfg(all(not(test), not(target_os = "linux")))]
pub unsafe fn spi_list_ports() -> *mut *mut c_char {
    let values: Vec<Vec<u8>> = Vec::new();
    unsafe { allocate_c_string_array(&values) }
}

#[cfg(test)]
mod tests_backend {
    use super::*;
    use std::collections::{HashMap, VecDeque};
    use std::sync::{Mutex, OnceLock};

    #[derive(Clone, Debug, Default)]
    pub(crate) struct FakeSpiPort {
        pub listed: bool,
        pub claimed: bool,
        pub speed: u32,
        pub mode: u32,
        pub queued_rx: VecDeque<Vec<u8>>,
        pub tx: Vec<Vec<u8>>,
        pub send_receive_error: Option<c_int>,
    }

    #[derive(Default)]
    struct State {
        ports: HashMap<String, FakeSpiPort>,
    }

    struct Handle {
        name: String,
    }

    static STATE: OnceLock<Mutex<State>> = OnceLock::new();

    fn state() -> &'static Mutex<State> {
        STATE.get_or_init(|| Mutex::new(State::default()))
    }

    fn with_port_mut<R>(port: spi_port, f: impl FnOnce(&mut FakeSpiPort) -> R) -> Result<R, c_int> {
        let Some(handle) = (unsafe { (port.cast::<Handle>()).as_ref() }) else {
            return Err(NFC_EIO);
        };
        let mut guard = state().lock().unwrap();
        let Some(port_state) = guard.ports.get_mut(&handle.name) else {
            return Err(NFC_EIO);
        };
        Ok(f(port_state))
    }

    pub(crate) fn reset() {
        *state().lock().unwrap() = State::default();
    }

    pub(crate) fn add_port(name: &str, listed: bool, claimed: bool) {
        state().lock().unwrap().ports.insert(
            name.to_string(),
            FakeSpiPort {
                listed,
                claimed,
                ..FakeSpiPort::default()
            },
        );
    }

    pub(crate) fn queue_rx(name: &str, bytes: &[u8]) {
        state()
            .lock()
            .unwrap()
            .ports
            .entry(name.to_string())
            .or_default()
            .queued_rx
            .push_back(bytes.to_vec());
    }

    pub(crate) fn take_tx(name: &str) -> Vec<Vec<u8>> {
        let mut guard = state().lock().unwrap();
        std::mem::take(&mut guard.ports.entry(name.to_string()).or_default().tx)
    }

    pub(crate) fn snapshot(name: &str) -> Option<FakeSpiPort> {
        state().lock().unwrap().ports.get(name).cloned()
    }

    pub unsafe fn spi_open(port_name: *const c_char) -> spi_port {
        let Some(port_name) = (unsafe { c_path_to_string(port_name) }) else {
            return invalid_spi_port();
        };
        let guard = state().lock().unwrap();
        let Some(port) = guard.ports.get(&port_name) else {
            return invalid_spi_port();
        };
        if port.claimed {
            return claimed_spi_port();
        }
        drop(guard);
        Box::into_raw(Box::new(Handle { name: port_name })).cast::<c_void>()
    }

    pub unsafe fn spi_close(port: spi_port) {
        if port.is_null() {
            return;
        }
        unsafe {
            drop(Box::from_raw(port.cast::<Handle>()));
        }
    }

    pub unsafe fn spi_set_speed(port: spi_port, speed: u32) {
        let _ = with_port_mut(port, |state| state.speed = speed);
    }

    pub unsafe fn spi_set_mode(port: spi_port, mode: u32) {
        let _ = with_port_mut(port, |state| state.mode = mode);
    }

    pub unsafe fn spi_get_speed(port: spi_port) -> u32 {
        with_port_mut(port, |state| state.speed).unwrap_or(0)
    }

    fn bit_reversal(byte: u8) -> u8 {
        let mut value = byte;
        value = ((value & 0xaa) >> 1) | ((value & 0x55) << 1);
        value = ((value & 0xcc) >> 2) | ((value & 0x33) << 2);
        ((value & 0xf0) >> 4) | ((value & 0x0f) << 4)
    }

    pub unsafe fn spi_send_receive(
        port: spi_port,
        tx: *const u8,
        tx_len: usize,
        rx: *mut u8,
        rx_len: usize,
        lsb_first: bool,
    ) -> c_int {
        with_port_mut(port, |state| {
            if let Some(code) = state.send_receive_error.take() {
                return code;
            }

            if tx_len > 0 {
                let mut tx_bytes = unsafe { std::slice::from_raw_parts(tx, tx_len) }.to_vec();
                if lsb_first {
                    for byte in &mut tx_bytes {
                        *byte = bit_reversal(*byte);
                    }
                }
                state.tx.push(tx_bytes);
            }

            if rx_len > 0 {
                let Some(next) = state.queued_rx.pop_front() else {
                    return NFC_EIO;
                };
                if next.len() != rx_len {
                    return NFC_EIO;
                }
                for (index, byte) in next.iter().copied().enumerate() {
                    unsafe {
                        *rx.add(index) = if lsb_first { bit_reversal(byte) } else { byte };
                    }
                }
            }

            NFC_SUCCESS
        })
        .unwrap_or(NFC_EIO)
    }

    pub unsafe fn spi_receive(
        port: spi_port,
        rx: *mut u8,
        rx_len: usize,
        lsb_first: bool,
    ) -> c_int {
        unsafe { spi_send_receive(port, ptr::null(), 0, rx, rx_len, lsb_first) }
    }

    pub unsafe fn spi_send(port: spi_port, tx: *const u8, tx_len: usize, lsb_first: bool) -> c_int {
        unsafe { spi_send_receive(port, tx, tx_len, ptr::null_mut(), 0, lsb_first) }
    }

    pub unsafe fn spi_list_ports() -> *mut *mut c_char {
        let guard = state().lock().unwrap();
        let mut ports = guard
            .ports
            .iter()
            .filter_map(|(name, port)| port.listed.then_some(name.as_bytes().to_vec()))
            .collect::<Vec<_>>();
        ports.sort();
        unsafe { allocate_c_string_array(&ports) }
    }
}

#[cfg(test)]
pub(crate) use tests_backend::{
    add_port as test_add_port, queue_rx as test_queue_rx, reset as test_reset,
    snapshot as test_snapshot, take_tx as test_take_tx,
};
#[cfg(test)]
pub use tests_backend::{
    spi_close, spi_get_speed, spi_list_ports, spi_open, spi_receive, spi_send, spi_send_receive,
    spi_set_mode, spi_set_speed,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi_support::bounded_strlen;
    use std::ffi::CString;
    use std::sync::{Mutex, OnceLock};

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn collect_ports(ptr: *mut *mut c_char) -> Vec<String> {
        let mut results = Vec::new();
        if ptr.is_null() {
            return results;
        }
        let mut index = 0usize;
        loop {
            let entry = unsafe { *ptr.add(index) };
            if entry.is_null() {
                break;
            }
            let len = bounded_strlen(entry, 256);
            let bytes = unsafe { std::slice::from_raw_parts(entry.cast::<u8>(), len) };
            results.push(String::from_utf8_lossy(bytes).into_owned());
            unsafe { crate::release_allocated_ptr(entry.cast::<c_void>()) };
            index += 1;
        }
        unsafe { crate::release_allocated_ptr(ptr.cast::<c_void>()) };
        results
    }

    #[test]
    fn list_open_and_mode_speed_roundtrip() {
        let _guard = test_lock();
        test_reset();
        test_add_port("/dev/spidev0.0", true, false);
        test_add_port("/dev/spidev0.1", false, false);

        let ports = collect_ports(unsafe { spi_list_ports() });
        assert_eq!(ports, vec!["/dev/spidev0.0"]);

        let path = CString::new("/dev/spidev0.0").unwrap();
        let handle = unsafe { spi_open(path.as_ptr()) };
        unsafe {
            spi_set_speed(handle, 1_000_000);
            spi_set_mode(handle, 0);
        }
        let snapshot = test_snapshot("/dev/spidev0.0").unwrap();
        assert_eq!(snapshot.speed, 1_000_000);
        assert_eq!(snapshot.mode, 0);
        assert_eq!(unsafe { spi_get_speed(handle) }, 1_000_000);
        unsafe { spi_close(handle) };
    }

    #[test]
    fn send_receive_and_lsb_first() {
        let _guard = test_lock();
        test_reset();
        test_add_port("/dev/spidev0.0", true, false);
        test_queue_rx("/dev/spidev0.0", &[0b0100_0000, 0b1000_0000]);

        let path = CString::new("/dev/spidev0.0").unwrap();
        let handle = unsafe { spi_open(path.as_ptr()) };
        let tx = [0b0000_0001, 0b0000_0010];
        let mut rx = [0u8; 2];
        assert_eq!(
            unsafe {
                spi_send_receive(
                    handle,
                    tx.as_ptr(),
                    tx.len(),
                    rx.as_mut_ptr(),
                    rx.len(),
                    true,
                )
            },
            NFC_SUCCESS
        );
        assert_eq!(
            test_take_tx("/dev/spidev0.0")[0],
            vec![0b1000_0000, 0b0100_0000]
        );
        assert_eq!(rx, [0b0000_0010, 0b0000_0001]);
        unsafe { spi_close(handle) };
    }
}
