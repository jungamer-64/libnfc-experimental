use crate::c_boundary::{LOG_PRIORITY_DEBUG, LOG_PRIORITY_NONE};
use crate::logger;
use crate::{emit_log_message, log_error, log_message, set_last_error_message};
use libc::c_char;
use proximate_driver as rt;
use std::ffi::CString;

const LOG_GROUP_CONFIG: u8 = 2;
const LOG_PRIORITY_ERROR: u8 = 1;
const LOG_PRIORITY_INFO: u8 = 2;
const LOG_CATEGORY_CONFIG: *const c_char = b"libnfc.config\0" as *const u8 as *const c_char;

fn record_log_init_for_tests() {
    #[cfg(test)]
    TEST_LIFECYCLE_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        state.log_init_calls += 1;
        state.events.push("log_init");
    });
}

fn record_log_exit_for_tests() {
    #[cfg(test)]
    TEST_LIFECYCLE_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        state.log_exit_calls += 1;
        state.events.push("log_exit");
    });
}

fn context_log_level(context: &rt::Context) -> u32 {
    context.config.log_level
}

fn initialize_context_logging(context: &rt::Context) {
    record_log_init_for_tests();
    logger::log_init(context_log_level(context));
}

pub(crate) fn finalize_context_logging() {
    record_log_exit_for_tests();
    logger::log_exit();
}

fn log_config_diagnostic(priority: u8, message: &str) {
    if let Ok(c_msg) = CString::new(message) {
        unsafe {
            emit_log_message(
                LOG_GROUP_CONFIG,
                LOG_CATEGORY_CONFIG,
                priority,
                c_msg.as_ptr(),
            )
        };
    }
}

pub(crate) fn emit_context_load_diagnostics(diagnostics: &[rt::diagnostics::ContextDiagnostic]) {
    for diagnostic in diagnostics {
        match diagnostic.category {
            rt::diagnostics::ContextDiagnosticCategory::General => match diagnostic.priority {
                rt::diagnostics::ContextDiagnosticPriority::Error => log_error(&diagnostic.message),
                rt::diagnostics::ContextDiagnosticPriority::Info => {
                    log_message(LOG_PRIORITY_INFO, &diagnostic.message)
                }
                rt::diagnostics::ContextDiagnosticPriority::Debug => {
                    log_message(LOG_PRIORITY_DEBUG, &diagnostic.message)
                }
            },
            rt::diagnostics::ContextDiagnosticCategory::Config => {
                let priority = match diagnostic.priority {
                    rt::diagnostics::ContextDiagnosticPriority::Error => LOG_PRIORITY_ERROR,
                    rt::diagnostics::ContextDiagnosticPriority::Info => LOG_PRIORITY_INFO,
                    rt::diagnostics::ContextDiagnosticPriority::Debug => LOG_PRIORITY_DEBUG,
                };
                log_config_diagnostic(priority, &diagnostic.message);
            }
        }
    }
}

pub(crate) fn load_context_outcome() -> Result<rt::diagnostics::ContextLoadOutcome, ()> {
    match rt::diagnostics::load_context_with_diagnostics() {
        Ok(outcome) => {
            emit_context_load_diagnostics(&outcome.diagnostics);
            Ok(outcome)
        }
        Err(failure) => {
            emit_context_load_diagnostics(&failure.diagnostics);
            set_last_error_message(failure.error.message().to_string());
            Err(())
        }
    }
}

fn log_context_state(context: &rt::Context) {
    let first_priority = if cfg!(feature = "debug-logging") {
        LOG_PRIORITY_NONE
    } else {
        LOG_PRIORITY_DEBUG
    };

    log_message(
        first_priority,
        &format!("log_level is set to {}", context.config.log_level),
    );
    log_message(
        LOG_PRIORITY_DEBUG,
        &format!(
            "allow_autoscan is set to {}",
            if context.config.allow_autoscan {
                "true"
            } else {
                "false"
            }
        ),
    );
    log_message(
        LOG_PRIORITY_DEBUG,
        &format!(
            "allow_intrusive_scan is set to {}",
            if context.config.allow_intrusive_scan {
                "true"
            } else {
                "false"
            }
        ),
    );
    log_message(
        LOG_PRIORITY_DEBUG,
        &format!(
            "{} device(s) defined by user",
            context.config.user_defined_devices.len()
        ),
    );

    for (index, device) in context.config.user_defined_devices.iter().enumerate() {
        log_message(
            LOG_PRIORITY_DEBUG,
            &format!(
                "  #{} name: \"{}\", connstring: \"{}\"",
                index,
                device.name,
                device.connstring.as_str()
            ),
        );
    }
}

pub(crate) fn initialize_loaded_context_logging(context: &rt::Context) {
    initialize_context_logging(context);
    log_context_state(context);
}

#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct LifecycleLoggingTestState {
    pub(crate) log_init_calls: usize,
    pub(crate) log_exit_calls: usize,
    pub(crate) context_free_calls: usize,
    pub(crate) events: Vec<&'static str>,
}

#[cfg(test)]
thread_local! {
    static TEST_LIFECYCLE_STATE: std::cell::RefCell<LifecycleLoggingTestState> =
        std::cell::RefCell::new(LifecycleLoggingTestState::default());
}

#[cfg(test)]
pub(crate) fn reset_lifecycle_test_state() {
    TEST_LIFECYCLE_STATE.with(|cell| {
        *cell.borrow_mut() = LifecycleLoggingTestState::default();
    });
}

#[cfg(test)]
pub(crate) fn snapshot_lifecycle_test_state() -> LifecycleLoggingTestState {
    TEST_LIFECYCLE_STATE.with(|cell| cell.borrow().clone())
}

#[cfg(test)]
pub(crate) fn increment_context_free_count_for_tests() {
    TEST_LIFECYCLE_STATE.with(|cell| {
        cell.borrow_mut().context_free_calls += 1;
    });
}
