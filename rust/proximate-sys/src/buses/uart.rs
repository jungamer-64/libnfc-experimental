// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Rust-backed UART helper preserving the legacy `uart_*` ABI.

#![allow(non_camel_case_types)]

#[cfg(any(all(not(test), unix, not(target_os = "linux")), all(not(test), windows)))]
use crate::buses::{allocate_c_string_array, c_path_to_string};
#[cfg(any(
    all(not(test), unix, not(target_os = "linux")),
    all(not(test), windows)
))]
use crate::buses::{claimed_serial_port, invalid_serial_port};
#[cfg(any(all(not(test), unix, not(target_os = "linux")), all(not(test), windows)))]
use crate::ffi_support::{as_mut, as_ref};
use libc::{c_char, c_int, c_void};
#[cfg(all(not(test), unix, not(target_os = "linux")))]
use std::cmp::min;
#[cfg(all(not(test), unix, not(target_os = "linux")))]
use std::fs;
#[cfg(any(all(not(test), unix, not(target_os = "linux")), all(not(test), windows)))]
use std::ptr;
#[cfg(all(not(test), unix, not(target_os = "linux")))]
use std::time::Duration;

const NFC_SUCCESS: c_int = 0;
const NFC_EIO: c_int = -1;
const NFC_ETIMEOUT: c_int = -6;
const NFC_EOPABORTED: c_int = -7;

pub type serial_port = *mut c_void;

#[cfg(all(not(test), target_os = "linux"))]
mod linux_impl {
    use super::*;
    use crate::buses::{
        allocate_c_string_array, c_path_to_string, claimed_serial_port, invalid_serial_port,
    };
    use ::proximate::ffi_internal_native::uart::{
        UartHandle, UartIoError, UartOpenError, list_ports as list_internal_ports,
    };

    unsafe fn handle<'a>(port: serial_port) -> Option<&'a mut UartHandle> {
        unsafe { (port.cast::<UartHandle>()).as_mut() }
    }

    pub unsafe fn uart_open(port_name: *const c_char) -> serial_port {
        let Some(port_name) = (unsafe { c_path_to_string(port_name) }) else {
            return invalid_serial_port();
        };
        match UartHandle::open(&port_name) {
            Ok(handle) => Box::into_raw(Box::new(handle)).cast::<c_void>(),
            Err(UartOpenError::InvalidPort) => invalid_serial_port(),
            Err(UartOpenError::ClaimedPort) => claimed_serial_port(),
        }
    }

    pub unsafe fn uart_close(port: serial_port) {
        let raw = port.cast::<UartHandle>();
        if raw.is_null() {
            return;
        }
        unsafe { drop(Box::from_raw(raw)) };
    }

    pub unsafe fn uart_flush_input(port: serial_port, wait: bool) {
        let Some(port) = (unsafe { handle(port) }) else {
            return;
        };
        let _ = port.flush_input(wait);
    }

    pub unsafe fn uart_set_speed(port: serial_port, speed: u32) {
        let Some(port) = (unsafe { handle(port) }) else {
            return;
        };
        let _ = port.set_speed(speed);
    }

    pub unsafe fn uart_get_speed(port: serial_port) -> u32 {
        let Some(port) = (unsafe { handle(port) }) else {
            return 0;
        };
        port.get_speed()
    }

    pub unsafe fn uart_receive(
        port: serial_port,
        rx: *mut u8,
        rx_len: usize,
        abort_p: *mut c_void,
        timeout: c_int,
    ) -> c_int {
        let Some(port) = (unsafe { handle(port) }) else {
            return NFC_EIO;
        };
        if rx.is_null() && rx_len != 0 {
            return NFC_EIO;
        }

        let abort_fd = if abort_p.is_null() {
            None
        } else {
            Some(unsafe { *(abort_p.cast::<c_int>()) })
        };
        let rx = if rx_len == 0 {
            &mut [][..]
        } else {
            unsafe { std::slice::from_raw_parts_mut(rx, rx_len) }
        };
        match port.receive(rx, abort_fd, timeout) {
            Ok(()) => NFC_SUCCESS,
            Err(UartIoError::Io) => NFC_EIO,
            Err(UartIoError::Timeout) => NFC_ETIMEOUT,
            Err(UartIoError::Aborted) => NFC_EOPABORTED,
        }
    }

    pub unsafe fn uart_send(
        port: serial_port,
        tx: *const u8,
        tx_len: usize,
        timeout: c_int,
    ) -> c_int {
        let Some(port) = (unsafe { handle(port) }) else {
            return NFC_EIO;
        };
        if tx.is_null() && tx_len != 0 {
            return NFC_EIO;
        }

        let tx = if tx_len == 0 {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(tx, tx_len) }
        };
        match port.send(tx, timeout) {
            Ok(()) => NFC_SUCCESS,
            Err(_) => NFC_EIO,
        }
    }

    pub unsafe fn uart_list_ports() -> *mut *mut c_char {
        let ports = list_internal_ports()
            .into_iter()
            .map(|port| port.into_bytes())
            .collect::<Vec<_>>();
        unsafe { allocate_c_string_array(&ports) }
    }
}

#[cfg(all(not(test), target_os = "linux"))]
pub use linux_impl::{
    uart_close, uart_flush_input, uart_get_speed, uart_list_ports, uart_open, uart_receive,
    uart_send, uart_set_speed,
};

#[cfg(all(not(test), unix, not(target_os = "linux")))]
#[repr(C)]
struct SerialPortUnix {
    fd: c_int,
    termios_backup: libc::termios,
    termios_new: libc::termios,
}

#[cfg(all(not(test), unix, not(target_os = "linux")))]
unsafe fn serial_port_unix<'a>(port: serial_port) -> Option<&'a mut SerialPortUnix> {
    unsafe { as_mut(port.cast::<SerialPortUnix>()) }
}

#[cfg(all(not(test), unix, not(target_os = "linux")))]
fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

#[cfg(all(not(test), unix, not(target_os = "linux")))]
fn baud_to_speed_t(speed: u32) -> Option<libc::speed_t> {
    match speed {
        9600 => Some(libc::B9600),
        19200 => Some(libc::B19200),
        38400 => Some(libc::B38400),
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "macos"
        ))]
        57600 => Some(libc::B57600),
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "macos"
        ))]
        115200 => Some(libc::B115200),
        #[cfg(any(target_os = "linux", target_os = "android"))]
        230400 => Some(libc::B230400),
        #[cfg(any(target_os = "linux", target_os = "android"))]
        460800 => Some(libc::B460800),
        _ => None,
    }
}

#[cfg(all(not(test), unix, not(target_os = "linux")))]
fn speed_t_to_baud(speed: libc::speed_t) -> u32 {
    match speed {
        x if x == libc::B9600 => 9600,
        x if x == libc::B19200 => 19200,
        x if x == libc::B38400 => 38400,
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "macos"
        ))]
        x if x == libc::B57600 => 57600,
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "macos"
        ))]
        x if x == libc::B115200 => 115200,
        #[cfg(any(target_os = "linux", target_os = "android"))]
        x if x == libc::B230400 => 230400,
        #[cfg(any(target_os = "linux", target_os = "android"))]
        x if x == libc::B460800 => 460800,
        _ => 0,
    }
}

#[cfg(all(not(test), unix, not(target_os = "linux")))]
fn serial_name_prefixes() -> &'static [&'static str] {
    #[cfg(target_os = "macos")]
    {
        &["tty.SLAB_USBtoUART", "tty.usbserial", "tty.usbmodem"]
    }
    #[cfg(any(target_os = "freebsd", target_os = "openbsd", target_os = "dragonfly"))]
    {
        &["cuaU", "cuau"]
    }
    #[cfg(target_os = "netbsd")]
    {
        &["tty0", "ttyC", "ttyS", "ttyU", "ttyY"]
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        &["ttyUSB", "ttyS", "ttyACM", "ttyAMA", "ttyO"]
    }
}

#[cfg(all(not(test), unix, not(target_os = "linux")))]
fn serial_name_must_end_with_digit() -> bool {
    !cfg!(target_os = "macos")
}

#[cfg(all(not(test), unix, not(target_os = "linux")))]
pub unsafe fn uart_open(port_name: *const c_char) -> serial_port {
    let Some(port_name) = (unsafe { c_path_to_string(port_name) }) else {
        return invalid_serial_port();
    };

    let raw = unsafe {
        libc::open(
            port_name.as_ptr().cast::<c_char>(),
            libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK,
        )
    };
    if raw < 0 {
        return invalid_serial_port();
    }

    let mut backup = unsafe { std::mem::zeroed::<libc::termios>() };
    if unsafe { libc::tcgetattr(raw, &mut backup) } != 0 {
        unsafe { libc::close(raw) };
        return invalid_serial_port();
    }

    if unsafe { libc::lockf(raw, libc::F_TLOCK, 0) } != 0 {
        unsafe { libc::close(raw) };
        return claimed_serial_port();
    }

    let mut current = backup;
    current.c_cflag = libc::CS8 | libc::CLOCAL | libc::CREAD;
    current.c_iflag = libc::IGNPAR;
    current.c_oflag = 0;
    current.c_lflag = 0;
    current.c_cc[libc::VMIN] = 0;
    current.c_cc[libc::VTIME] = 0;

    if unsafe { libc::tcsetattr(raw, libc::TCSANOW, &current) } != 0 {
        unsafe {
            libc::lockf(raw, libc::F_ULOCK, 0);
            libc::close(raw);
        }
        return invalid_serial_port();
    }

    Box::into_raw(Box::new(SerialPortUnix {
        fd: raw,
        termios_backup: backup,
        termios_new: current,
    }))
    .cast::<c_void>()
}

#[cfg(all(not(test), unix, not(target_os = "linux")))]
pub unsafe fn uart_close(port: serial_port) {
    let Some(port) = (unsafe { serial_port_unix(port) }) else {
        return;
    };

    unsafe {
        libc::tcsetattr(port.fd, libc::TCSANOW, &port.termios_backup);
        libc::lockf(port.fd, libc::F_ULOCK, 0);
        libc::close(port.fd);
        drop(Box::from_raw(port));
    }
}

#[cfg(all(not(test), unix, not(target_os = "linux")))]
pub unsafe fn uart_flush_input(port: serial_port, wait: bool) {
    let Some(port) = (unsafe { serial_port_unix(port) }) else {
        return;
    };

    if wait {
        sleep_ms(50);
    }

    unsafe {
        libc::tcflush(port.fd, libc::TCIFLUSH);
    }

    let mut available = 0;
    if unsafe { libc::ioctl(port.fd, libc::FIONREAD, &mut available) } != 0 || available <= 0 {
        return;
    }

    let mut scratch = vec![0u8; available as usize];
    unsafe {
        libc::read(
            port.fd,
            scratch.as_mut_ptr().cast::<c_void>(),
            scratch.len(),
        );
    }
}

#[cfg(all(not(test), unix, not(target_os = "linux")))]
pub unsafe fn uart_set_speed(port: serial_port, speed: u32) {
    let Some(port) = (unsafe { serial_port_unix(port) }) else {
        return;
    };
    let Some(termios_speed) = baud_to_speed_t(speed) else {
        return;
    };

    unsafe {
        libc::cfsetispeed(&mut port.termios_new, termios_speed);
        libc::cfsetospeed(&mut port.termios_new, termios_speed);
        libc::tcsetattr(port.fd, libc::TCSADRAIN, &port.termios_new);
    }
}

#[cfg(all(not(test), unix, not(target_os = "linux")))]
pub unsafe fn uart_get_speed(port: serial_port) -> u32 {
    let Some(port) = (unsafe { serial_port_unix(port) }) else {
        return 0;
    };

    let speed = unsafe { libc::cfgetispeed(&port.termios_new) };
    speed_t_to_baud(speed)
}

#[cfg(all(not(test), unix, not(target_os = "linux")))]
pub unsafe fn uart_receive(
    port: serial_port,
    rx: *mut u8,
    rx_len: usize,
    abort_p: *mut c_void,
    timeout: c_int,
) -> c_int {
    let Some(port) = (unsafe { serial_port_unix(port) }) else {
        return NFC_EIO;
    };
    if rx.is_null() && rx_len != 0 {
        return NFC_EIO;
    }

    let abort_fd = if abort_p.is_null() {
        -1
    } else {
        unsafe { *(abort_p.cast::<c_int>()) }
    };

    let mut received = 0usize;
    while received < rx_len {
        let mut pollfds = [libc::pollfd {
            fd: port.fd,
            events: libc::POLLIN,
            revents: 0,
        }; 2];
        let nfds = if abort_fd > 0 {
            pollfds[1] = libc::pollfd {
                fd: abort_fd,
                events: libc::POLLIN | libc::POLLERR | libc::POLLHUP,
                revents: 0,
            };
            2
        } else {
            1
        };

        let rc = loop {
            let result = unsafe { libc::poll(pollfds.as_mut_ptr(), nfds as libc::nfds_t, timeout) };
            if result < 0 && unsafe { *libc::__errno_location() } == libc::EINTR {
                continue;
            }
            break result;
        };

        if rc < 0 {
            return NFC_EIO;
        }
        if rc == 0 {
            return NFC_ETIMEOUT;
        }
        if abort_fd > 0 && pollfds[1].revents != 0 {
            unsafe {
                libc::close(abort_fd);
            }
            return NFC_EOPABORTED;
        }
        if (pollfds[0].revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL)) != 0 {
            return NFC_EIO;
        }
        if (pollfds[0].revents & libc::POLLIN) == 0 {
            continue;
        }

        let mut available = 0;
        if unsafe { libc::ioctl(port.fd, libc::FIONREAD, &mut available) } != 0 {
            return NFC_EIO;
        }
        let want = min((rx_len - received) as c_int, available.max(1)) as usize;
        let rc = unsafe { libc::read(port.fd, rx.add(received).cast::<c_void>(), want) };
        if rc <= 0 {
            return NFC_EIO;
        }
        received += rc as usize;
    }

    NFC_SUCCESS
}

#[cfg(all(not(test), unix, not(target_os = "linux")))]
pub unsafe fn uart_send(port: serial_port, tx: *const u8, tx_len: usize, _timeout: c_int) -> c_int {
    let Some(port) = (unsafe { serial_port_unix(port) }) else {
        return NFC_EIO;
    };
    if tx.is_null() && tx_len != 0 {
        return NFC_EIO;
    }

    let rc = unsafe { libc::write(port.fd, tx.cast::<c_void>(), tx_len) };
    if rc == tx_len as isize {
        NFC_SUCCESS
    } else {
        NFC_EIO
    }
}

#[cfg(all(not(test), unix, not(target_os = "linux")))]
pub unsafe fn uart_list_ports() -> *mut *mut c_char {
    let mut matches = Vec::new();
    let Ok(entries) = fs::read_dir("/dev") else {
        return unsafe { allocate_c_string_array(&matches) };
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if serial_name_must_end_with_digit()
            && name
                .bytes()
                .last()
                .is_none_or(|byte| !byte.is_ascii_digit())
        {
            continue;
        }
        if serial_name_prefixes()
            .iter()
            .any(|prefix| name.starts_with(prefix))
        {
            matches.push(format!("/dev/{name}").into_bytes());
        }
    }

    unsafe { allocate_c_string_array(&matches) }
}

#[cfg(all(not(test), windows))]
mod windows_impl {
    use super::{
        NFC_EIO, NFC_EOPABORTED, NFC_ETIMEOUT, NFC_SUCCESS, claimed_serial_port,
        invalid_serial_port, serial_port,
    };
    use crate::buses::{allocate_c_string_array, c_path_to_string};
    use libc::{c_char, c_int, c_void};
    use std::ffi::CString;
    use std::ptr;

    type BOOL = i32;
    type DWORD = u32;
    type HANDLE = *mut c_void;
    type LPCSTR = *const i8;
    type LPVOID = *mut c_void;

    const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;
    const GENERIC_READ: DWORD = 0x8000_0000;
    const GENERIC_WRITE: DWORD = 0x4000_0000;
    const OPEN_EXISTING: DWORD = 3;
    const PURGE_RXABORT: DWORD = 0x0002;
    const PURGE_RXCLEAR: DWORD = 0x0008;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct DCB {
        dcblength: DWORD,
        baud_rate: DWORD,
        flags: DWORD,
        reserved: u16,
        xon_lim: u16,
        xoff_lim: u16,
        byte_size: u8,
        parity: u8,
        stop_bits: u8,
        xon_char: i8,
        xoff_char: i8,
        error_char: i8,
        eof_char: i8,
        evt_char: i8,
        reserved1: u16,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct COMMTIMEOUTS {
        read_interval_timeout: DWORD,
        read_total_timeout_multiplier: DWORD,
        read_total_timeout_constant: DWORD,
        write_total_timeout_multiplier: DWORD,
        write_total_timeout_constant: DWORD,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct COMMCONFIG {
        dw_size: DWORD,
        w_version: u16,
        w_reserved: u16,
        dcb: DCB,
        dw_provider_sub_type: DWORD,
        dw_provider_offset: DWORD,
        dw_provider_size: DWORD,
        wc_provider_data: [u16; 1],
    }

    #[repr(C)]
    struct SerialPortWindows {
        handle: HANDLE,
        dcb: DCB,
        timeouts: COMMTIMEOUTS,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateFileA(
            lpFileName: LPCSTR,
            dwDesiredAccess: DWORD,
            dwShareMode: DWORD,
            lpSecurityAttributes: LPVOID,
            dwCreationDisposition: DWORD,
            dwFlagsAndAttributes: DWORD,
            hTemplateFile: HANDLE,
        ) -> HANDLE;
        fn CloseHandle(hObject: HANDLE) -> BOOL;
        fn BuildCommDCBA(lpDef: LPCSTR, lpDCB: *mut DCB) -> BOOL;
        fn SetCommState(hFile: HANDLE, lpDCB: *const DCB) -> BOOL;
        fn GetCommState(hFile: HANDLE, lpDCB: *mut DCB) -> BOOL;
        fn SetCommTimeouts(hFile: HANDLE, lpCommTimeouts: *const COMMTIMEOUTS) -> BOOL;
        fn PurgeComm(hFile: HANDLE, dwFlags: DWORD) -> BOOL;
        fn ReadFile(
            hFile: HANDLE,
            lpBuffer: LPVOID,
            nNumberOfBytesToRead: DWORD,
            lpNumberOfBytesRead: *mut DWORD,
            lpOverlapped: LPVOID,
        ) -> BOOL;
        fn WriteFile(
            hFile: HANDLE,
            lpBuffer: *const c_void,
            nNumberOfBytesToWrite: DWORD,
            lpNumberOfBytesWritten: *mut DWORD,
            lpOverlapped: LPVOID,
        ) -> BOOL;
        fn GetDefaultCommConfigA(
            lpszName: LPCSTR,
            lpCC: *mut COMMCONFIG,
            lpdwSize: *mut DWORD,
        ) -> BOOL;
    }

    unsafe fn port_ref<'a>(port: serial_port) -> Option<&'a mut SerialPortWindows> {
        unsafe { (port.cast::<SerialPortWindows>()).as_mut() }
    }

    pub unsafe fn uart_open(port_name: *const c_char) -> serial_port {
        let Some(port_name) = (unsafe { c_path_to_string(port_name) }) else {
            return invalid_serial_port();
        };
        let path = format!("\\\\.\\{}", port_name.to_ascii_uppercase());
        let Ok(path_c) = CString::new(path) else {
            return invalid_serial_port();
        };

        let handle = unsafe {
            CreateFileA(
                path_c.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                ptr::null_mut(),
                OPEN_EXISTING,
                0,
                ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return invalid_serial_port();
        }

        let mut state = Box::new(SerialPortWindows {
            handle,
            dcb: DCB::default(),
            timeouts: COMMTIMEOUTS::default(),
        });
        state.dcb.dcblength = std::mem::size_of::<DCB>() as DWORD;

        let Ok(config) = CString::new("baud=9600 data=8 parity=N stop=1") else {
            unsafe { CloseHandle(handle) };
            return invalid_serial_port();
        };

        if unsafe { BuildCommDCBA(config.as_ptr(), &mut state.dcb) } == 0
            || unsafe { SetCommState(state.handle, &state.dcb) } == 0
        {
            unsafe { CloseHandle(handle) };
            return invalid_serial_port();
        }

        state.timeouts = COMMTIMEOUTS {
            read_interval_timeout: 30,
            read_total_timeout_multiplier: 0,
            read_total_timeout_constant: 30,
            write_total_timeout_multiplier: 30,
            write_total_timeout_constant: 0,
        };

        if unsafe { SetCommTimeouts(state.handle, &state.timeouts) } == 0 {
            unsafe { CloseHandle(handle) };
            return invalid_serial_port();
        }

        unsafe {
            PurgeComm(state.handle, PURGE_RXABORT | PURGE_RXCLEAR);
        }
        Box::into_raw(state).cast::<c_void>()
    }

    pub unsafe fn uart_close(port: serial_port) {
        let Some(port) = (unsafe { port_ref(port) }) else {
            return;
        };
        unsafe {
            if port.handle != INVALID_HANDLE_VALUE {
                CloseHandle(port.handle);
            }
            drop(Box::from_raw(port));
        }
    }

    pub unsafe fn uart_flush_input(port: serial_port, _wait: bool) {
        let Some(port) = (unsafe { port_ref(port) }) else {
            return;
        };
        unsafe {
            PurgeComm(port.handle, PURGE_RXABORT | PURGE_RXCLEAR);
        }
    }

    pub unsafe fn uart_set_speed(port: serial_port, speed: u32) {
        let Some(port) = (unsafe { port_ref(port) }) else {
            return;
        };
        match speed {
            9600 | 19200 | 38400 | 57600 | 115200 | 230400 | 460800 => {}
            _ => return,
        }
        port.dcb.baud_rate = speed;
        unsafe {
            SetCommState(port.handle, &port.dcb);
            PurgeComm(port.handle, PURGE_RXABORT | PURGE_RXCLEAR);
        }
    }

    pub unsafe fn uart_get_speed(port: serial_port) -> u32 {
        let Some(port) = (unsafe { port_ref(port) }) else {
            return 0;
        };
        let mut dcb = port.dcb;
        if unsafe { GetCommState(port.handle, &mut dcb) } == 0 {
            return port.dcb.baud_rate;
        }
        dcb.baud_rate
    }

    pub unsafe fn uart_receive(
        port: serial_port,
        rx: *mut u8,
        rx_len: usize,
        abort_p: *mut c_void,
        timeout: c_int,
    ) -> c_int {
        let Some(port) = (unsafe { port_ref(port) }) else {
            return NFC_EIO;
        };

        let timeouts = COMMTIMEOUTS {
            read_interval_timeout: 0,
            read_total_timeout_multiplier: 0,
            read_total_timeout_constant: timeout.max(0) as DWORD,
            write_total_timeout_multiplier: 0,
            write_total_timeout_constant: timeout.max(0) as DWORD,
        };
        if unsafe { SetCommTimeouts(port.handle, &timeouts) } == 0 {
            return NFC_EIO;
        }

        let abort_flag = abort_p.cast::<bool>();
        let mut total = 0usize;
        while total < rx_len {
            let mut read = 0u32;
            let ok = unsafe {
                ReadFile(
                    port.handle,
                    rx.add(total).cast::<c_void>(),
                    (rx_len - total) as DWORD,
                    &mut read,
                    ptr::null_mut(),
                )
            };
            if ok == 0 {
                return NFC_EIO;
            }
            if read == 0 {
                return NFC_ETIMEOUT;
            }
            total += read as usize;

            if !abort_flag.is_null() && unsafe { *abort_flag } && total == 0 {
                return NFC_EOPABORTED;
            }
        }

        NFC_SUCCESS
    }

    pub unsafe fn uart_send(
        port: serial_port,
        tx: *const u8,
        tx_len: usize,
        timeout: c_int,
    ) -> c_int {
        let Some(port) = (unsafe { port_ref(port) }) else {
            return NFC_EIO;
        };
        let timeouts = COMMTIMEOUTS {
            read_interval_timeout: 0,
            read_total_timeout_multiplier: 0,
            read_total_timeout_constant: timeout.max(0) as DWORD,
            write_total_timeout_multiplier: 0,
            write_total_timeout_constant: timeout.max(0) as DWORD,
        };
        if unsafe { SetCommTimeouts(port.handle, &timeouts) } == 0 {
            return NFC_EIO;
        }

        let mut written = 0u32;
        if unsafe {
            WriteFile(
                port.handle,
                tx.cast::<c_void>(),
                tx_len as DWORD,
                &mut written,
                ptr::null_mut(),
            )
        } == 0
            || written == 0
        {
            return NFC_EIO;
        }
        NFC_SUCCESS
    }

    pub unsafe fn uart_list_ports() -> *mut *mut c_char {
        let mut ports = Vec::new();
        for index in 1..=255u32 {
            let candidate = format!("COM{index}");
            let Ok(candidate_c) = CString::new(candidate.clone()) else {
                continue;
            };
            let mut cc = COMMCONFIG::default();
            let mut cc_size = std::mem::size_of::<COMMCONFIG>() as DWORD;
            if unsafe { GetDefaultCommConfigA(candidate_c.as_ptr(), &mut cc, &mut cc_size) } != 0 {
                ports.push(candidate.into_bytes());
            }
        }

        unsafe { allocate_c_string_array(&ports) }
    }

    #[allow(dead_code)]
    pub const fn claimed_serial_port_value() -> serial_port {
        claimed_serial_port()
    }
}

#[cfg(all(not(test), windows))]
pub use windows_impl::{
    uart_close, uart_flush_input, uart_get_speed, uart_list_ports, uart_open, uart_receive,
    uart_send, uart_set_speed,
};

#[cfg(all(not(test), not(any(unix, windows))))]
compile_error!("uart helper is only implemented for unix and windows targets");

#[cfg(test)]
mod tests_backend {
    use super::{NFC_EIO, NFC_EOPABORTED, NFC_ETIMEOUT, NFC_SUCCESS, serial_port};
    use crate::buses::{allocate_c_string_array, claimed_serial_port, invalid_serial_port};
    use libc::{c_char, c_int, c_void};
    use std::collections::{HashMap, VecDeque};
    use std::sync::{Mutex, OnceLock};

    #[derive(Clone, Debug, Default)]
    pub(crate) struct FakeSerialPort {
        pub listed: bool,
        pub claimed: bool,
        pub speed: u32,
        pub stale_rx: VecDeque<u8>,
        pub rx: VecDeque<u8>,
        pub tx: Vec<Vec<u8>>,
        pub flush_count: usize,
        pub waited_flush_count: usize,
        pub receive_error: Option<c_int>,
        pub send_error: Option<c_int>,
    }

    #[derive(Default)]
    struct State {
        ports: HashMap<String, FakeSerialPort>,
    }

    struct Handle {
        name: String,
    }

    static STATE: OnceLock<Mutex<State>> = OnceLock::new();

    fn state() -> &'static Mutex<State> {
        STATE.get_or_init(|| Mutex::new(State::default()))
    }

    fn with_port_mut<R>(
        port: serial_port,
        f: impl FnOnce(&mut FakeSerialPort) -> R,
    ) -> Result<R, c_int> {
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
            FakeSerialPort {
                listed,
                claimed,
                ..FakeSerialPort::default()
            },
        );
    }

    pub(crate) fn queue_rx(name: &str, bytes: &[u8]) {
        let mut guard = state().lock().unwrap();
        let entry = guard.ports.entry(name.to_string()).or_default();
        entry.rx.extend(bytes.iter().copied());
    }

    pub(crate) fn queue_stale_rx(name: &str, bytes: &[u8]) {
        let mut guard = state().lock().unwrap();
        let entry = guard.ports.entry(name.to_string()).or_default();
        entry.stale_rx.extend(bytes.iter().copied());
    }

    pub(crate) fn take_tx(name: &str) -> Vec<Vec<u8>> {
        let mut guard = state().lock().unwrap();
        let entry = guard.ports.entry(name.to_string()).or_default();
        std::mem::take(&mut entry.tx)
    }

    pub(crate) fn snapshot(name: &str) -> Option<FakeSerialPort> {
        state().lock().unwrap().ports.get(name).cloned()
    }

    pub unsafe fn uart_open(port_name: *const c_char) -> serial_port {
        let Some(port_name) = (unsafe { crate::buses::c_path_to_string(port_name) }) else {
            return invalid_serial_port();
        };
        let guard = state().lock().unwrap();
        let Some(port) = guard.ports.get(&port_name) else {
            return invalid_serial_port();
        };
        if port.claimed {
            return claimed_serial_port();
        }
        drop(guard);
        Box::into_raw(Box::new(Handle { name: port_name })).cast::<c_void>()
    }

    pub unsafe fn uart_close(port: serial_port) {
        if port.is_null() {
            return;
        }
        unsafe {
            drop(Box::from_raw(port.cast::<Handle>()));
        }
    }

    pub unsafe fn uart_flush_input(port: serial_port, wait: bool) {
        let _ = with_port_mut(port, |state| {
            state.flush_count += 1;
            if wait {
                state.waited_flush_count += 1;
            }
            state.stale_rx.clear();
        });
    }

    pub unsafe fn uart_set_speed(port: serial_port, speed: u32) {
        let _ = with_port_mut(port, |state| state.speed = speed);
    }

    pub unsafe fn uart_get_speed(port: serial_port) -> u32 {
        with_port_mut(port, |state| state.speed).unwrap_or(0)
    }

    pub unsafe fn uart_receive(
        port: serial_port,
        rx: *mut u8,
        rx_len: usize,
        abort_p: *mut c_void,
        _timeout: c_int,
    ) -> c_int {
        if !abort_p.is_null() {
            let fd = unsafe { *(abort_p.cast::<c_int>()) };
            if fd > 0 {
                if unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
                    unsafe {
                        libc::close(fd);
                    }
                    return NFC_EOPABORTED;
                }
                let mut pollfd = libc::pollfd {
                    fd,
                    events: libc::POLLERR | libc::POLLHUP | libc::POLLNVAL,
                    revents: 0,
                };
                if unsafe { libc::poll(&mut pollfd, 1, 0) } > 0 && pollfd.revents != 0 {
                    unsafe {
                        libc::close(fd);
                    }
                    return NFC_EOPABORTED;
                }
            }
        }

        with_port_mut(port, |state| {
            if let Some(code) = state.receive_error.take() {
                return code;
            }
            let available = state.stale_rx.len() + state.rx.len();
            if available < rx_len {
                return NFC_ETIMEOUT;
            }
            for index in 0..rx_len {
                let next = state
                    .stale_rx
                    .pop_front()
                    .or_else(|| state.rx.pop_front())
                    .unwrap();
                unsafe {
                    *rx.add(index) = next;
                }
            }
            NFC_SUCCESS
        })
        .unwrap_or(NFC_EIO)
    }

    pub unsafe fn uart_send(
        port: serial_port,
        tx: *const u8,
        tx_len: usize,
        _timeout: c_int,
    ) -> c_int {
        with_port_mut(port, |state| {
            if let Some(code) = state.send_error.take() {
                return code;
            }
            let bytes = if tx_len == 0 {
                Vec::new()
            } else {
                unsafe { std::slice::from_raw_parts(tx, tx_len) }.to_vec()
            };
            state.tx.push(bytes);
            NFC_SUCCESS
        })
        .unwrap_or(NFC_EIO)
    }

    pub unsafe fn uart_list_ports() -> *mut *mut c_char {
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
    add_port as test_add_port, queue_rx as test_queue_rx, queue_stale_rx as test_queue_stale_rx,
    reset as test_reset, snapshot as test_snapshot, take_tx as test_take_tx,
};
#[cfg(test)]
pub use tests_backend::{
    uart_close, uart_flush_input, uart_get_speed, uart_list_ports, uart_open, uart_receive,
    uart_send, uart_set_speed,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buses::{claimed_serial_port, invalid_serial_port};
    use crate::ffi_support::bounded_strlen;
    use std::ffi::CString;
    use std::ptr;
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
            unsafe {
                crate::release_allocated_ptr(entry.cast::<c_void>());
            }
            index += 1;
        }
        unsafe {
            crate::release_allocated_ptr(ptr.cast::<c_void>());
        }
        results
    }

    #[test]
    fn open_invalid_and_claimed_ports() {
        let _guard = test_lock();
        test_reset();
        test_add_port("/dev/ttyUSB0", true, false);
        test_add_port("/dev/ttyUSB1", true, true);

        let valid = CString::new("/dev/ttyUSB0").unwrap();
        let claimed = CString::new("/dev/ttyUSB1").unwrap();
        let missing = CString::new("/dev/ttyUSB9").unwrap();

        let valid_handle = unsafe { uart_open(valid.as_ptr()) };
        assert!(!valid_handle.is_null());
        unsafe { uart_close(valid_handle) };

        assert_eq!(
            unsafe { uart_open(claimed.as_ptr()) },
            claimed_serial_port()
        );
        assert_eq!(
            unsafe { uart_open(missing.as_ptr()) },
            invalid_serial_port()
        );
    }

    #[test]
    fn list_ports_and_speed_roundtrip() {
        let _guard = test_lock();
        test_reset();
        test_add_port("/dev/ttyUSB0", true, false);
        test_add_port("/dev/ttyUSB1", false, false);

        let ports = collect_ports(unsafe { uart_list_ports() });
        assert_eq!(ports, vec!["/dev/ttyUSB0"]);

        let name = CString::new("/dev/ttyUSB0").unwrap();
        let handle = unsafe { uart_open(name.as_ptr()) };
        unsafe { uart_set_speed(handle, 115200) };
        assert_eq!(unsafe { uart_get_speed(handle) }, 115200);
        unsafe { uart_close(handle) };
    }

    #[test]
    fn receive_timeout_abort_and_flush() {
        let _guard = test_lock();
        test_reset();
        test_add_port("/dev/ttyUSB0", true, false);

        let name = CString::new("/dev/ttyUSB0").unwrap();
        let handle = unsafe { uart_open(name.as_ptr()) };

        let mut rx = [0u8; 2];
        assert_eq!(
            unsafe { uart_receive(handle, rx.as_mut_ptr(), rx.len(), ptr::null_mut(), 10) },
            NFC_ETIMEOUT
        );

        let mut pipefds = [0i32; 2];
        assert_eq!(unsafe { libc::pipe(pipefds.as_mut_ptr()) }, 0);
        unsafe {
            libc::close(pipefds[0]);
        }
        assert_eq!(
            unsafe {
                uart_receive(
                    handle,
                    rx.as_mut_ptr(),
                    rx.len(),
                    (&mut pipefds[1] as *mut i32).cast::<c_void>(),
                    10,
                )
            },
            NFC_EOPABORTED
        );

        test_queue_stale_rx("/dev/ttyUSB0", &[1, 2, 3]);
        unsafe { uart_flush_input(handle, true) };
        let snapshot = test_snapshot("/dev/ttyUSB0").unwrap();
        assert_eq!(snapshot.flush_count, 1);
        assert_eq!(snapshot.waited_flush_count, 1);
        assert!(snapshot.stale_rx.is_empty());

        unsafe { uart_close(handle) };
    }
}
