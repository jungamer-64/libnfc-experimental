// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Rust-backed SPI helper preserving the legacy `spi_*` ABI.

#![allow(non_camel_case_types)]

use crate::buses::{allocate_c_string_array, c_path_to_string, claimed_spi_port, invalid_spi_port};
use libc::{c_char, c_int, c_void};
use std::ptr;

const NFC_SUCCESS: c_int = 0;
const NFC_EIO: c_int = -1;
const NFC_ESOFT: c_int = -80;

pub type spi_port = *mut c_void;

#[cfg(all(not(test), target_os = "linux"))]
mod linux_impl {
    use super::*;
    use std::fs;

    const SPI_IOC_MAGIC: u8 = b'k';
    const IOC_NRBITS: u32 = 8;
    const IOC_TYPEBITS: u32 = 8;
    const IOC_SIZEBITS: u32 = 14;
    const IOC_NRSHIFT: u32 = 0;
    const IOC_TYPESHIFT: u32 = IOC_NRSHIFT + IOC_NRBITS;
    const IOC_SIZESHIFT: u32 = IOC_TYPESHIFT + IOC_TYPEBITS;
    const IOC_DIRSHIFT: u32 = IOC_SIZESHIFT + IOC_SIZEBITS;
    const IOC_WRITE: u32 = 1;
    const IOC_READ: u32 = 2;
    const SPI_MODE_0: u32 = 0;

    #[repr(C)]
    struct spi_ioc_transfer {
        tx_buf: u64,
        rx_buf: u64,
        len: u32,
        speed_hz: u32,
        delay_usecs: u16,
        bits_per_word: u8,
        cs_change: u8,
        tx_nbits: u8,
        rx_nbits: u8,
        word_delay_usecs: u8,
        pad: u8,
    }

    #[repr(C)]
    struct SpiPortLinux {
        fd: c_int,
    }

    const fn ioc(dir: u32, ty: u8, nr: u8, size: u32) -> libc::c_ulong {
        ((dir << IOC_DIRSHIFT)
            | ((ty as u32) << IOC_TYPESHIFT)
            | ((nr as u32) << IOC_NRSHIFT)
            | (size << IOC_SIZESHIFT)) as libc::c_ulong
    }

    const fn iow<T>(ty: u8, nr: u8) -> libc::c_ulong {
        ioc(IOC_WRITE, ty, nr, std::mem::size_of::<T>() as u32)
    }

    const fn ior<T>(ty: u8, nr: u8) -> libc::c_ulong {
        ioc(IOC_READ, ty, nr, std::mem::size_of::<T>() as u32)
    }

    const fn iowr<T>(ty: u8, nr: u8) -> libc::c_ulong {
        ioc(
            IOC_READ | IOC_WRITE,
            ty,
            nr,
            std::mem::size_of::<T>() as u32,
        )
    }

    fn spi_ioc_message(count: usize) -> libc::c_ulong {
        iowr_raw(
            SPI_IOC_MAGIC,
            0,
            (std::mem::size_of::<spi_ioc_transfer>() * count) as u32,
        )
    }

    const fn iowr_raw(ty: u8, nr: u8, size: u32) -> libc::c_ulong {
        ioc(IOC_READ | IOC_WRITE, ty, nr, size)
    }

    const SPI_IOC_WR_MODE: libc::c_ulong = iow::<u8>(SPI_IOC_MAGIC, 1);
    const SPI_IOC_WR_MAX_SPEED_HZ: libc::c_ulong = iow::<u32>(SPI_IOC_MAGIC, 4);
    const SPI_IOC_RD_MAX_SPEED_HZ: libc::c_ulong = ior::<u32>(SPI_IOC_MAGIC, 4);

    unsafe fn port_ref<'a>(port: spi_port) -> Option<&'a mut SpiPortLinux> {
        unsafe { (port.cast::<SpiPortLinux>()).as_mut() }
    }

    fn bit_reversal(byte: u8) -> u8 {
        let mut value = byte;
        value = ((value & 0xaa) >> 1) | ((value & 0x55) << 1);
        value = ((value & 0xcc) >> 2) | ((value & 0x33) << 2);
        ((value & 0xf0) >> 4) | ((value & 0x0f) << 4)
    }

    pub unsafe fn spi_open(port_name: *const c_char) -> spi_port {
        let Some(port_name) = (unsafe { c_path_to_string(port_name) }) else {
            return invalid_spi_port();
        };

        let fd = unsafe {
            libc::open(
                port_name.as_ptr().cast::<c_char>(),
                libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            return invalid_spi_port();
        }

        Box::into_raw(Box::new(SpiPortLinux { fd })).cast::<c_void>()
    }

    pub unsafe fn spi_close(port: spi_port) {
        let raw = port.cast::<SpiPortLinux>();
        if raw.is_null() {
            return;
        }
        unsafe {
            libc::close((*raw).fd);
            drop(Box::from_raw(raw));
        }
    }

    pub unsafe fn spi_set_speed(port: spi_port, speed: u32) {
        let Some(port) = (unsafe { port_ref(port) }) else {
            return;
        };
        unsafe {
            libc::ioctl(port.fd, SPI_IOC_WR_MAX_SPEED_HZ, &speed);
        }
    }

    pub unsafe fn spi_set_mode(port: spi_port, mode: u32) {
        let Some(port) = (unsafe { port_ref(port) }) else {
            return;
        };
        let mode = mode as u8;
        unsafe {
            libc::ioctl(port.fd, SPI_IOC_WR_MODE, &mode);
        }
    }

    pub unsafe fn spi_get_speed(port: spi_port) -> u32 {
        let Some(port) = (unsafe { port_ref(port) }) else {
            return 0;
        };
        let mut speed = 0u32;
        unsafe {
            libc::ioctl(port.fd, SPI_IOC_RD_MAX_SPEED_HZ, &mut speed);
        }
        speed
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

        let mut tx_storage = Vec::new();
        if tx_len > 0 {
            let source = unsafe { std::slice::from_raw_parts(tx, tx_len) };
            tx_storage = source.to_vec();
            if lsb_first {
                for byte in &mut tx_storage {
                    *byte = bit_reversal(*byte);
                }
            }
        }

        let mut transfers = Vec::with_capacity(2);
        if tx_len > 0 {
            transfers.push(spi_ioc_transfer {
                tx_buf: tx_storage.as_ptr() as u64,
                rx_buf: 0,
                len: tx_len as u32,
                speed_hz: 0,
                delay_usecs: 0,
                bits_per_word: 0,
                cs_change: 0,
                tx_nbits: 0,
                rx_nbits: 0,
                word_delay_usecs: 0,
                pad: 0,
            });
        }
        if rx_len > 0 {
            transfers.push(spi_ioc_transfer {
                tx_buf: 0,
                rx_buf: rx as u64,
                len: rx_len as u32,
                speed_hz: 0,
                delay_usecs: 0,
                bits_per_word: 0,
                cs_change: 0,
                tx_nbits: 0,
                rx_nbits: 0,
                word_delay_usecs: 0,
                pad: 0,
            });
        }

        if !transfers.is_empty() {
            let rc = unsafe {
                libc::ioctl(
                    port.fd,
                    spi_ioc_message(transfers.len()),
                    transfers.as_mut_ptr(),
                )
            };
            if rc != (tx_len + rx_len) as c_int {
                return NFC_EIO;
            }
            if rx_len > 0 && lsb_first {
                let rx_slice = unsafe { std::slice::from_raw_parts_mut(rx, rx_len) };
                for byte in rx_slice {
                    *byte = bit_reversal(*byte);
                }
            }
        }

        NFC_SUCCESS
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
        let mut matches = Vec::new();
        let Ok(entries) = fs::read_dir("/dev") else {
            return unsafe { allocate_c_string_array(&matches) };
        };

        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("spidev")
                || name
                    .bytes()
                    .last()
                    .is_none_or(|byte| !byte.is_ascii_digit())
            {
                continue;
            }
            matches.push(format!("/dev/{name}").into_bytes());
        }

        unsafe { allocate_c_string_array(&matches) }
    }

    #[allow(dead_code)]
    pub const SPI_MODE_0_VALUE: u32 = SPI_MODE_0;
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

    pub(crate) fn set_send_receive_error(name: &str, code: c_int) {
        state()
            .lock()
            .unwrap()
            .ports
            .entry(name.to_string())
            .or_default()
            .send_receive_error = Some(code);
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
    set_send_receive_error as test_set_send_receive_error, snapshot as test_snapshot,
    take_tx as test_take_tx,
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
