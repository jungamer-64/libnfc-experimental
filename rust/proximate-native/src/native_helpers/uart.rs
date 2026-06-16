use std::fs;
#[cfg(target_os = "linux")]
use std::os::fd::{BorrowedFd, FromRawFd, OwnedFd};
use std::time::Duration;

#[cfg(target_os = "linux")]
use rustix::event::{PollFd, PollFlags, Timespec, poll};
#[cfg(target_os = "linux")]
use rustix::fs::{FlockOperation, Mode, OFlags, flock, open};
#[cfg(target_os = "linux")]
use rustix::io::{ioctl_fionread, read, write};
#[cfg(target_os = "linux")]
use rustix::termios::{OptionalActions, QueueSelector, Termios, tcflush, tcgetattr, tcsetattr};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UartOpenError {
    InvalidPort,
    ClaimedPort,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UartIoError {
    Io,
    Timeout,
    Aborted,
}

pub fn list_ports() -> Vec<String> {
    let mut ports = Vec::new();
    let Ok(entries) = fs::read_dir("/dev") else {
        return ports;
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
            ports.push(format!("/dev/{name}"));
        }
    }

    ports.sort();
    ports
}

fn serial_name_prefixes() -> &'static [&'static str] {
    &["ttyUSB", "ttyS", "ttyACM", "ttyAMA", "ttyO"]
}

fn serial_name_must_end_with_digit() -> bool {
    true
}

#[cfg(target_os = "linux")]
pub struct UartHandle {
    fd: OwnedFd,
    original_termios: Termios,
    configured_termios: Termios,
}

#[cfg(target_os = "linux")]
impl UartHandle {
    pub fn open(path: &str) -> Result<Self, UartOpenError> {
        let fd = open(
            path,
            OFlags::RDWR | OFlags::NONBLOCK | OFlags::NOCTTY,
            Mode::empty(),
        )
        .map_err(|_| UartOpenError::InvalidPort)?;

        flock(&fd, FlockOperation::NonBlockingLockExclusive)
            .map_err(|_| UartOpenError::ClaimedPort)?;

        let original_termios = tcgetattr(&fd).map_err(|_| UartOpenError::InvalidPort)?;
        let mut configured_termios = original_termios.clone();
        configured_termios.make_raw();
        configured_termios.special_codes[rustix::termios::SpecialCodeIndex::VMIN] = 0;
        configured_termios.special_codes[rustix::termios::SpecialCodeIndex::VTIME] = 0;
        tcsetattr(&fd, OptionalActions::Now, &configured_termios)
            .map_err(|_| UartOpenError::InvalidPort)?;

        Ok(Self {
            fd,
            original_termios,
            configured_termios,
        })
    }

    pub fn flush_input(&mut self, wait: bool) -> Result<(), UartIoError> {
        if wait {
            std::thread::sleep(Duration::from_millis(50));
        }

        tcflush(&self.fd, QueueSelector::IFlush).map_err(|_| UartIoError::Io)?;
        let mut available = ioctl_fionread(&self.fd).unwrap_or(0) as usize;
        while available > 0 {
            let mut scratch = vec![0u8; available.min(256)];
            match read(&self.fd, scratch.as_mut_slice()) {
                Ok(0) => break,
                Ok(read_len) => {
                    if read_len >= available {
                        break;
                    }
                    available -= read_len;
                }
                Err(_) => return Err(UartIoError::Io),
            }
        }
        Ok(())
    }

    pub fn set_speed(&mut self, speed: u32) -> Result<(), UartIoError> {
        self.configured_termios
            .set_speed(speed)
            .map_err(|_| UartIoError::Io)?;
        tcsetattr(&self.fd, OptionalActions::Drain, &self.configured_termios)
            .map_err(|_| UartIoError::Io)
    }

    pub fn get_speed(&self) -> u32 {
        self.configured_termios.output_speed()
    }

    pub fn send(&self, tx: &[u8], timeout_ms: i32) -> Result<(), UartIoError> {
        let mut sent = 0usize;
        while sent < tx.len() {
            wait_for(&self.fd, None, PollFlags::OUT, timeout_ms)?;
            let len = write(&self.fd, &tx[sent..]).map_err(|_| UartIoError::Io)?;
            if len == 0 {
                return Err(UartIoError::Io);
            }
            sent += len;
        }
        Ok(())
    }

    pub fn receive(
        &self,
        rx: &mut [u8],
        abort_fd: Option<i32>,
        timeout_ms: i32,
    ) -> Result<(), UartIoError> {
        let mut received = 0usize;
        while received < rx.len() {
            wait_for(&self.fd, abort_fd, PollFlags::IN, timeout_ms)?;

            let available = ioctl_fionread(&self.fd).unwrap_or(1).max(1) as usize;
            let want = (rx.len() - received).min(available);
            let len =
                read(&self.fd, &mut rx[received..received + want]).map_err(|_| UartIoError::Io)?;
            if len == 0 {
                return Err(UartIoError::Io);
            }
            received += len;
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
impl Drop for UartHandle {
    fn drop(&mut self) {
        let _ = tcsetattr(&self.fd, OptionalActions::Now, &self.original_termios);
    }
}

#[cfg(target_os = "linux")]
fn wait_for(
    fd: &OwnedFd,
    abort_fd: Option<i32>,
    flags: PollFlags,
    timeout_ms: i32,
) -> Result<(), UartIoError> {
    let timeout = timeout_spec(timeout_ms);
    if let Some(raw_abort_fd) = abort_fd.filter(|fd| *fd >= 0) {
        let abort_fd = unsafe { BorrowedFd::borrow_raw(raw_abort_fd) };
        let mut pollfds = vec![
            PollFd::new(fd, flags),
            PollFd::new(
                &abort_fd,
                PollFlags::IN | PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL,
            ),
        ];

        let ready = poll(&mut pollfds, timeout.as_ref()).map_err(|_| UartIoError::Io)?;
        if ready == 0 {
            return Err(UartIoError::Timeout);
        }

        if !pollfds[1].revents().is_empty() {
            // Mirror the legacy helper contract: the abort marker owns the old
            // pipe's write-end and should be closed once it has fired.
            unsafe {
                drop(OwnedFd::from_raw_fd(raw_abort_fd));
            }
            return Err(UartIoError::Aborted);
        }

        let revents = pollfds[0].revents();
        if revents.intersects(PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL) {
            return Err(UartIoError::Io);
        }
        if !revents.intersects(flags) {
            return Err(UartIoError::Io);
        }
        return Ok(());
    }

    let mut pollfds = vec![PollFd::new(fd, flags)];
    let ready = poll(&mut pollfds, timeout.as_ref()).map_err(|_| UartIoError::Io)?;
    if ready == 0 {
        return Err(UartIoError::Timeout);
    }

    let revents = pollfds[0].revents();
    if revents.intersects(PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL) {
        return Err(UartIoError::Io);
    }
    if !revents.intersects(flags) {
        return Err(UartIoError::Io);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn timeout_spec(timeout_ms: i32) -> Option<Timespec> {
    if timeout_ms < 0 {
        None
    } else {
        Some(Timespec {
            tv_sec: (timeout_ms / 1000) as i64,
            tv_nsec: ((timeout_ms % 1000) as i64) * 1_000_000,
        })
    }
}

#[cfg(not(target_os = "linux"))]
pub struct UartHandle;

#[cfg(not(target_os = "linux"))]
impl UartHandle {
    pub fn open(_path: &str) -> Result<Self, UartOpenError> {
        Err(UartOpenError::InvalidPort)
    }

    pub fn flush_input(&mut self, _wait: bool) -> Result<(), UartIoError> {
        Err(UartIoError::Io)
    }

    pub fn set_speed(&mut self, _speed: u32) -> Result<(), UartIoError> {
        Err(UartIoError::Io)
    }

    pub fn get_speed(&self) -> u32 {
        0
    }

    pub fn send(&self, _tx: &[u8], _timeout_ms: i32) -> Result<(), UartIoError> {
        Err(UartIoError::Io)
    }

    pub fn receive(
        &self,
        _rx: &mut [u8],
        _abort_fd: Option<i32>,
        _timeout_ms: i32,
    ) -> Result<(), UartIoError> {
        Err(UartIoError::Io)
    }
}
