// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Rust-backed I2C helper preserving the legacy `i2c_*` ABI.

#![allow(non_camel_case_types)]

use crate::buses::{
    allocate_c_string_array, c_path_to_string, invalid_i2c_address, invalid_i2c_bus,
};
use libc::{c_char, c_int, c_void, ssize_t};

const NFC_SUCCESS: c_int = 0;
const NFC_EIO: c_int = -1;
const NFC_EINVARG: c_int = -2;

pub type i2c_device = *mut c_void;

#[cfg(all(not(test), target_os = "linux"))]
mod linux_impl {
    use super::*;
    use ::proximate_platform::i2c::{
        I2cHandle, I2cIoError, I2cOpenError, list_ports as list_internal_ports,
    };

    unsafe fn device_ref<'a>(device: i2c_device) -> Option<&'a mut I2cHandle> {
        unsafe { (device.cast::<I2cHandle>()).as_mut() }
    }

    pub unsafe fn i2c_open(bus_name: *const c_char, address: u32) -> i2c_device {
        let Some(bus_name) = (unsafe { c_path_to_string(bus_name) }) else {
            return invalid_i2c_bus();
        };
        match I2cHandle::open(&bus_name, address) {
            Ok(handle) => Box::into_raw(Box::new(handle)).cast::<c_void>(),
            Err(I2cOpenError::InvalidBus) => invalid_i2c_bus(),
            Err(I2cOpenError::InvalidAddress) => invalid_i2c_address(),
        }
    }

    pub unsafe fn i2c_close(device: i2c_device) {
        let raw = device.cast::<I2cHandle>();
        if raw.is_null() {
            return;
        }
        unsafe { drop(Box::from_raw(raw)) };
    }

    pub unsafe fn i2c_read(device: i2c_device, rx: *mut u8, rx_len: usize) -> ssize_t {
        let Some(device) = (unsafe { device_ref(device) }) else {
            return NFC_EIO as ssize_t;
        };
        let rx = unsafe { std::slice::from_raw_parts_mut(rx, rx_len) };
        match device.read(rx) {
            Ok(()) => rx_len as ssize_t,
            Err(I2cIoError::Io) => NFC_EIO as ssize_t,
            Err(I2cIoError::InvalidArgument) => NFC_EINVARG as ssize_t,
        }
    }

    pub unsafe fn i2c_write(device: i2c_device, tx: *const u8, tx_len: usize) -> c_int {
        let Some(device) = (unsafe { device_ref(device) }) else {
            return NFC_EIO;
        };
        let tx = unsafe { std::slice::from_raw_parts(tx, tx_len) };
        match device.write(tx) {
            Ok(()) => NFC_SUCCESS,
            Err(_) => NFC_EIO,
        }
    }

    pub unsafe fn i2c_list_ports() -> *mut *mut c_char {
        let ports = list_internal_ports()
            .into_iter()
            .map(|port| port.into_bytes())
            .collect::<Vec<_>>();
        unsafe { allocate_c_string_array(&ports) }
    }
}

#[cfg(all(not(test), target_os = "linux"))]
pub use linux_impl::{i2c_close, i2c_list_ports, i2c_open, i2c_read, i2c_write};

#[cfg(all(not(test), not(target_os = "linux")))]
pub unsafe fn i2c_open(_bus_name: *const c_char, _address: u32) -> i2c_device {
    invalid_i2c_bus()
}

#[cfg(all(not(test), not(target_os = "linux")))]
pub unsafe fn i2c_close(_device: i2c_device) {}

#[cfg(all(not(test), not(target_os = "linux")))]
pub unsafe fn i2c_read(_device: i2c_device, _rx: *mut u8, _rx_len: usize) -> ssize_t {
    NFC_EIO as ssize_t
}

#[cfg(all(not(test), not(target_os = "linux")))]
pub unsafe fn i2c_write(_device: i2c_device, _tx: *const u8, _tx_len: usize) -> c_int {
    NFC_EIO
}

#[cfg(all(not(test), not(target_os = "linux")))]
pub unsafe fn i2c_list_ports() -> *mut *mut c_char {
    let values: Vec<Vec<u8>> = Vec::new();
    unsafe { allocate_c_string_array(&values) }
}

#[cfg(test)]
mod tests_backend {
    use super::*;
    use std::collections::{HashMap, VecDeque};
    use std::sync::{Mutex, OnceLock};

    #[derive(Clone, Debug, Default)]
    pub(crate) struct FakeI2cDevice {
        pub listed: bool,
        pub fail_address: bool,
        pub rx: VecDeque<u8>,
        pub tx: Vec<Vec<u8>>,
        pub read_error: Option<ssize_t>,
        pub write_error: Option<c_int>,
    }

    #[derive(Default)]
    struct State {
        buses: HashMap<String, FakeI2cDevice>,
    }

    struct Handle {
        name: String,
    }

    static STATE: OnceLock<Mutex<State>> = OnceLock::new();

    fn state() -> &'static Mutex<State> {
        STATE.get_or_init(|| Mutex::new(State::default()))
    }

    fn with_bus_mut<R>(
        device: i2c_device,
        f: impl FnOnce(&mut FakeI2cDevice) -> R,
    ) -> Result<R, ssize_t> {
        let Some(handle) = (unsafe { (device.cast::<Handle>()).as_ref() }) else {
            return Err(NFC_EIO as ssize_t);
        };
        let mut guard = state().lock().unwrap();
        let Some(bus_state) = guard.buses.get_mut(&handle.name) else {
            return Err(NFC_EIO as ssize_t);
        };
        Ok(f(bus_state))
    }

    pub(crate) fn reset() {
        *state().lock().unwrap() = State::default();
    }

    pub(crate) fn add_bus(name: &str, listed: bool, fail_address: bool) {
        state().lock().unwrap().buses.insert(
            name.to_string(),
            FakeI2cDevice {
                listed,
                fail_address,
                ..FakeI2cDevice::default()
            },
        );
    }

    pub(crate) fn queue_rx(name: &str, bytes: &[u8]) {
        state()
            .lock()
            .unwrap()
            .buses
            .entry(name.to_string())
            .or_default()
            .rx
            .extend(bytes.iter().copied());
    }

    pub(crate) fn take_tx(name: &str) -> Vec<Vec<u8>> {
        let mut guard = state().lock().unwrap();
        std::mem::take(&mut guard.buses.entry(name.to_string()).or_default().tx)
    }

    pub(crate) fn set_read_error(name: &str, code: ssize_t) {
        state()
            .lock()
            .unwrap()
            .buses
            .entry(name.to_string())
            .or_default()
            .read_error = Some(code);
    }

    pub unsafe fn i2c_open(bus_name: *const c_char, _address: u32) -> i2c_device {
        let Some(bus_name) = (unsafe { c_path_to_string(bus_name) }) else {
            return invalid_i2c_bus();
        };
        let guard = state().lock().unwrap();
        let Some(bus) = guard.buses.get(&bus_name) else {
            return invalid_i2c_bus();
        };
        if bus.fail_address {
            return invalid_i2c_address();
        }
        drop(guard);
        Box::into_raw(Box::new(Handle { name: bus_name })).cast::<c_void>()
    }

    pub unsafe fn i2c_close(device: i2c_device) {
        if device.is_null() {
            return;
        }
        unsafe {
            drop(Box::from_raw(device.cast::<Handle>()));
        }
    }

    pub unsafe fn i2c_read(device: i2c_device, rx: *mut u8, rx_len: usize) -> ssize_t {
        with_bus_mut(device, |state| {
            if let Some(code) = state.read_error.take() {
                return code;
            }
            if state.rx.is_empty() {
                return NFC_EINVARG as ssize_t;
            }
            let available = state.rx.len().min(rx_len);
            for index in 0..available {
                unsafe {
                    *rx.add(index) = state.rx.pop_front().unwrap();
                }
            }
            for index in available..rx_len {
                unsafe {
                    *rx.add(index) = 0;
                }
            }
            rx_len as ssize_t
        })
        .unwrap_or(NFC_EIO as ssize_t)
    }

    pub unsafe fn i2c_write(device: i2c_device, tx: *const u8, tx_len: usize) -> c_int {
        with_bus_mut(device, |state| {
            if let Some(code) = state.write_error.take() {
                return code;
            }
            state
                .tx
                .push(unsafe { std::slice::from_raw_parts(tx, tx_len) }.to_vec());
            NFC_SUCCESS
        })
        .unwrap_or(NFC_EIO as c_int)
    }

    pub unsafe fn i2c_list_ports() -> *mut *mut c_char {
        let guard = state().lock().unwrap();
        let mut buses = guard
            .buses
            .iter()
            .filter_map(|(name, bus)| bus.listed.then_some(name.as_bytes().to_vec()))
            .collect::<Vec<_>>();
        buses.sort();
        unsafe { allocate_c_string_array(&buses) }
    }
}

#[cfg(test)]
pub(crate) use tests_backend::{
    add_bus as test_add_bus, queue_rx as test_queue_rx, reset as test_reset,
    set_read_error as test_set_read_error, take_tx as test_take_tx,
};
#[cfg(test)]
pub use tests_backend::{i2c_close, i2c_list_ports, i2c_open, i2c_read, i2c_write};

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
    fn list_open_and_invalid_address() {
        let _guard = test_lock();
        test_reset();
        test_add_bus("/dev/i2c-1", true, false);
        test_add_bus("/dev/i2c-2", true, true);

        let ports = collect_ports(unsafe { i2c_list_ports() });
        assert_eq!(ports, vec!["/dev/i2c-1", "/dev/i2c-2"]);

        let ok = CString::new("/dev/i2c-1").unwrap();
        let bad = CString::new("/dev/i2c-2").unwrap();
        assert!(!unsafe { i2c_open(ok.as_ptr(), 0x24) }.is_null());
        assert_eq!(
            unsafe { i2c_open(bad.as_ptr(), 0x24) },
            invalid_i2c_address()
        );
    }

    #[test]
    fn read_write_and_errors() {
        let _guard = test_lock();
        test_reset();
        test_add_bus("/dev/i2c-1", true, false);

        let bus = CString::new("/dev/i2c-1").unwrap();
        let handle = unsafe { i2c_open(bus.as_ptr(), 0x24) };
        let tx = [0xaa, 0xbb];
        assert_eq!(
            unsafe { i2c_write(handle, tx.as_ptr(), tx.len()) },
            NFC_SUCCESS
        );
        assert_eq!(test_take_tx("/dev/i2c-1"), vec![vec![0xaa, 0xbb]]);

        test_queue_rx("/dev/i2c-1", &[1, 2, 3]);
        let mut rx = [0u8; 3];
        assert_eq!(unsafe { i2c_read(handle, rx.as_mut_ptr(), rx.len()) }, 3);
        assert_eq!(rx, [1, 2, 3]);

        test_set_read_error("/dev/i2c-1", NFC_EIO as ssize_t);
        assert_eq!(
            unsafe { i2c_read(handle, rx.as_mut_ptr(), 1) },
            NFC_EIO as ssize_t
        );
        unsafe { i2c_close(handle) };
    }
}
