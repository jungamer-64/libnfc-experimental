use libc::c_char;
use std::ffi::CStr;
#[cfg(not(test))]
use std::io::{self, Write};
#[cfg(not(test))]
use std::sync::atomic::{AtomicU32, Ordering};

const DEFAULT_LOG_LEVEL: u32 = if cfg!(libnfc_debug) { 3 } else { 1 };

#[cfg(not(test))]
static CURRENT_LOG_LEVEL: AtomicU32 = AtomicU32::new(DEFAULT_LOG_LEVEL);

#[cfg(test)]
thread_local! {
    static CURRENT_LOG_LEVEL: std::cell::Cell<u32> =
        const { std::cell::Cell::new(DEFAULT_LOG_LEVEL) };
    static TEST_RENDERED_LOGS: std::cell::RefCell<Vec<Vec<u8>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

#[inline]
pub(crate) fn default_log_level() -> u32 {
    DEFAULT_LOG_LEVEL
}

#[inline]
pub(crate) fn log_init(log_level: u32) {
    #[cfg(not(test))]
    CURRENT_LOG_LEVEL.store(log_level, Ordering::Relaxed);
    #[cfg(test)]
    CURRENT_LOG_LEVEL.with(|cell| cell.set(log_level));
}

#[inline]
pub(crate) fn log_exit() {}

#[inline]
#[cfg(not(test))]
fn current_log_level() -> u32 {
    CURRENT_LOG_LEVEL.load(Ordering::Relaxed)
}

#[inline]
#[cfg(test)]
fn current_log_level() -> u32 {
    CURRENT_LOG_LEVEL.with(|cell| cell.get())
}

#[inline]
fn group_log_level(log_level: u32, group: u8) -> u32 {
    let shift = (group as u32).saturating_mul(2);
    if shift >= u32::BITS {
        return 0;
    }
    (log_level >> shift) & 0x0000_0003
}

#[inline]
fn should_log(log_level: u32, group: u8, priority: u8) -> bool {
    if log_level == 0 {
        return false;
    }

    let priority = u32::from(priority);
    (log_level & 0x0000_0003) >= priority || group_log_level(log_level, group) >= priority
}

#[inline]
fn priority_bytes(priority: u8) -> &'static [u8] {
    match priority {
        1 => b"error",
        2 => b"info",
        3 => b"debug",
        _ => b"unknown",
    }
}

fn render_line(priority: u8, category: &[u8], message: &[u8]) -> Vec<u8> {
    let mut rendered =
        Vec::with_capacity(priority_bytes(priority).len() + category.len() + message.len() + 3);
    rendered.extend_from_slice(priority_bytes(priority));
    rendered.push(b'\t');
    rendered.extend_from_slice(category);
    rendered.push(b'\t');
    rendered.extend_from_slice(message);
    rendered.push(b'\n');
    rendered
}

#[cfg(not(test))]
fn record_rendered_line(rendered: &[u8]) {
    let mut stderr = io::stderr().lock();
    let _ = stderr.write_all(rendered);
    let _ = stderr.flush();
}

#[cfg(test)]
fn record_rendered_line(rendered: &[u8]) {
    TEST_RENDERED_LOGS.with(|cell| {
        cell.borrow_mut().push(rendered.to_vec());
    });
}

pub(crate) fn log_message_bytes(group: u8, category: &[u8], priority: u8, message: &[u8]) {
    let log_level = current_log_level();
    if !should_log(log_level, group, priority) {
        return;
    }

    let rendered = render_line(priority, category, message);
    record_rendered_line(&rendered);
}

pub(crate) unsafe fn log_message_ptrs(
    group: u8,
    category: *const c_char,
    priority: u8,
    message: *const c_char,
) {
    let category_bytes = if category.is_null() {
        b""
    } else {
        unsafe { CStr::from_ptr(category) }.to_bytes()
    };
    let message_bytes = if message.is_null() {
        b""
    } else {
        unsafe { CStr::from_ptr(message) }.to_bytes()
    };
    log_message_bytes(group, category_bytes, priority, message_bytes);
}

#[cfg(test)]
fn message_slice(rendered: &[u8]) -> &[u8] {
    let Some(first_tab) = rendered.iter().position(|byte| *byte == b'\t') else {
        return trim_trailing_newline(rendered);
    };
    let remaining = &rendered[first_tab + 1..];
    let Some(second_tab_rel) = remaining.iter().position(|byte| *byte == b'\t') else {
        return trim_trailing_newline(rendered);
    };
    let start = first_tab + second_tab_rel + 2;
    trim_trailing_newline(&rendered[start..])
}

#[cfg(test)]
fn trim_trailing_newline(rendered: &[u8]) -> &[u8] {
    rendered.strip_suffix(b"\n").unwrap_or(rendered)
}

#[cfg(test)]
pub(crate) fn test_get_rendered_logs() -> Vec<Vec<u8>> {
    TEST_RENDERED_LOGS.with(|cell| cell.borrow().clone())
}

#[cfg(test)]
pub(crate) fn test_get_last_log() -> Option<String> {
    TEST_RENDERED_LOGS.with(|cell| {
        cell.borrow()
            .last()
            .map(|entry| String::from_utf8_lossy(message_slice(entry)).into_owned())
    })
}

#[cfg(test)]
pub(crate) fn test_get_logs() -> Vec<String> {
    TEST_RENDERED_LOGS.with(|cell| {
        cell.borrow()
            .iter()
            .map(|entry| String::from_utf8_lossy(message_slice(entry)).into_owned())
            .collect()
    })
}

#[cfg(test)]
pub(crate) fn test_clear_rendered_logs() {
    TEST_RENDERED_LOGS.with(|cell| cell.borrow_mut().clear());
}

#[cfg(test)]
pub(crate) fn test_reset_log_level() {
    log_init(DEFAULT_LOG_LEVEL);
}

#[cfg(test)]
mod tests {
    use super::*;

    const GROUP_GENERAL: u8 = 1;
    const GROUP_DRIVER: u8 = 4;

    fn reset_test_logger() {
        test_reset_log_level();
        test_clear_rendered_logs();
    }

    #[test]
    fn default_log_level_filters_messages() {
        reset_test_logger();

        log_message_bytes(GROUP_GENERAL, b"libnfc.common", 1, b"error enabled");
        log_message_bytes(GROUP_GENERAL, b"libnfc.common", 2, b"info maybe enabled");
        log_message_bytes(GROUP_GENERAL, b"libnfc.common", 3, b"debug maybe enabled");

        let rendered = test_get_rendered_logs();
        assert!(
            rendered
                .iter()
                .any(|entry| entry == b"error\tlibnfc.common\terror enabled\n")
        );

        if DEFAULT_LOG_LEVEL >= 2 {
            assert!(
                rendered
                    .iter()
                    .any(|entry| entry == b"info\tlibnfc.common\tinfo maybe enabled\n")
            );
        } else {
            assert!(
                !rendered
                    .iter()
                    .any(|entry| entry == b"info\tlibnfc.common\tinfo maybe enabled\n")
            );
        }

        if DEFAULT_LOG_LEVEL >= 3 {
            assert!(
                rendered
                    .iter()
                    .any(|entry| entry == b"debug\tlibnfc.common\tdebug maybe enabled\n")
            );
        } else {
            assert!(
                !rendered
                    .iter()
                    .any(|entry| entry == b"debug\tlibnfc.common\tdebug maybe enabled\n")
            );
        }
    }

    #[test]
    fn group_specific_log_level_overrides_global_level() {
        reset_test_logger();
        log_init(769);

        log_message_bytes(GROUP_DRIVER, b"libnfc.driver", 3, b"group debug");
        log_message_bytes(GROUP_GENERAL, b"libnfc.general", 3, b"general debug");

        let rendered = test_get_rendered_logs();
        assert!(
            rendered
                .iter()
                .any(|entry| entry == b"debug\tlibnfc.driver\tgroup debug\n")
        );
        assert!(
            !rendered
                .iter()
                .any(|entry| entry == b"debug\tlibnfc.general\tgeneral debug\n")
        );
    }

    #[test]
    fn log_message_ptrs_accepts_null_and_empty_messages() {
        reset_test_logger();
        log_init(3);

        unsafe {
            log_message_ptrs(
                GROUP_GENERAL,
                c"libnfc.general".as_ptr(),
                2,
                std::ptr::null(),
            );
            log_message_ptrs(GROUP_GENERAL, c"libnfc.general".as_ptr(), 2, c"".as_ptr());
        }

        let rendered = test_get_rendered_logs();
        assert_eq!(rendered[0], b"info\tlibnfc.general\t\n");
        assert_eq!(rendered[1], b"info\tlibnfc.general\t\n");
    }

    #[test]
    fn log_message_ptrs_preserves_non_utf8_payloads() {
        reset_test_logger();
        log_init(3);

        let category = b"libnfc.raw\0";
        let message = [0x66u8, 0x6f, 0xff, 0x80, 0x00];
        unsafe {
            log_message_ptrs(
                GROUP_GENERAL,
                category.as_ptr().cast(),
                3,
                message.as_ptr().cast(),
            );
        }

        let rendered = test_get_rendered_logs();
        assert_eq!(rendered.len(), 1);
        assert_eq!(rendered[0], b"debug\tlibnfc.raw\tfo\xff\x80\n");
    }

    #[test]
    fn rendered_line_matches_legacy_format() {
        reset_test_logger();
        log_init(3);

        log_message_bytes(GROUP_GENERAL, b"libnfc.common", 3, b"hello");

        let rendered = test_get_rendered_logs();
        assert_eq!(rendered, vec![b"debug\tlibnfc.common\thello\n".to_vec()]);
    }
}
