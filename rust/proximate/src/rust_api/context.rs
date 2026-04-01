use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::NFC_BUFSIZE_CONNSTRING;

use super::ConnectionString;

const DEVICE_NAME_LENGTH: usize = 256;
const MAX_USER_DEFINED_DEVICES: usize = 4;
const USER_DEFINED_DEFAULT_DEVICE_NAME: &str = "user defined default device";
const USER_DEFINED_DEVICE_NAME: &str = "user defined device";
const LIBNFC_CONFFILE_NAME: &str = "libnfc.conf";
const LIBNFC_DEVICECONFDIR_NAME: &str = "devices.d";
const MAX_CONFIG_LINE_BYTES: usize = 8192;
const CONFIG_MAX_DEVICES_MESSAGE: &str = "Configuration exceeded maximum user-defined devices.";
const DEFAULT_NON_WINDOWS_CONFDIR: &str = "/usr/local/etc/nfc";
const DEFAULT_WINDOWS_CONFDIR: &str = "./config";

thread_local! {
    static TEST_CONF_ROOT: std::cell::RefCell<Option<Option<PathBuf>>> =
        const { std::cell::RefCell::new(None) };
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserDefinedDevice {
    pub name: String,
    pub connstring: ConnectionString,
    pub optional: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextConfig {
    pub allow_autoscan: bool,
    pub allow_intrusive_scan: bool,
    pub log_level: u32,
    pub user_defined_devices: Vec<UserDefinedDevice>,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            allow_autoscan: true,
            allow_intrusive_scan: false,
            log_level: if cfg!(libnfc_debug) { 3 } else { 1 },
            user_defined_devices: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[doc(hidden)]
pub struct ContextSources {
    pub config_file: Option<ContextConfig>,
    pub default_device: Option<UserDefinedDevice>,
    pub selected_device: Option<UserDefinedDevice>,
    pub allow_autoscan: Option<bool>,
    pub allow_intrusive_scan: Option<bool>,
    pub log_level: Option<u32>,
    pub max_user_defined_devices: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Context {
    pub config: ContextConfig,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub enum ContextDiagnosticCategory {
    General,
    Config,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub enum ContextDiagnosticPriority {
    Error,
    Info,
    Debug,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct ContextDiagnostic {
    pub category: ContextDiagnosticCategory,
    pub priority: ContextDiagnosticPriority,
    pub message: String,
}

impl ContextDiagnostic {
    fn general_error(message: impl Into<String>) -> Self {
        Self {
            category: ContextDiagnosticCategory::General,
            priority: ContextDiagnosticPriority::Error,
            message: message.into(),
        }
    }

    fn config_error(message: impl Into<String>) -> Self {
        Self {
            category: ContextDiagnosticCategory::Config,
            priority: ContextDiagnosticPriority::Error,
            message: message.into(),
        }
    }

    fn config_info(message: impl Into<String>) -> Self {
        Self {
            category: ContextDiagnosticCategory::Config,
            priority: ContextDiagnosticPriority::Info,
            message: message.into(),
        }
    }

    fn config_debug(message: impl Into<String>) -> Self {
        Self {
            category: ContextDiagnosticCategory::Config,
            priority: ContextDiagnosticPriority::Debug,
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct ContextLoadOutcome {
    pub context: Context,
    pub diagnostics: Vec<ContextDiagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct ContextLoadFailure {
    pub diagnostics: Vec<ContextDiagnostic>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct RawUserDefinedDevice {
    name: Vec<u8>,
    connstring: Vec<u8>,
    optional: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ParsedContextConfig {
    allow_autoscan: bool,
    allow_intrusive_scan: bool,
    log_level: u32,
    user_defined_devices: Vec<RawUserDefinedDevice>,
}

impl ParsedContextConfig {
    fn with_defaults() -> Self {
        let defaults = ContextConfig::default();
        Self {
            allow_autoscan: defaults.allow_autoscan,
            allow_intrusive_scan: defaults.allow_intrusive_scan,
            log_level: defaults.log_level,
            user_defined_devices: Vec::new(),
        }
    }

    fn into_context_config(self) -> ContextConfig {
        let user_defined_devices = self
            .user_defined_devices
            .into_iter()
            .filter_map(|device| {
                if device.connstring.is_empty() {
                    return None;
                }

                let connstring =
                    ConnectionString::new(String::from_utf8_lossy(&device.connstring).into_owned())
                        .ok()?;
                Some(UserDefinedDevice {
                    name: String::from_utf8_lossy(&device.name).into_owned(),
                    connstring,
                    optional: device.optional,
                })
            })
            .collect();

        ContextConfig {
            allow_autoscan: self.allow_autoscan,
            allow_intrusive_scan: self.allow_intrusive_scan,
            log_level: self.log_level,
            user_defined_devices,
        }
    }
}

#[derive(Clone, Copy)]
enum UserDeviceField {
    Name,
    Connstring,
    Optional,
}

impl Context {
    pub fn new() -> Self {
        Self {
            config: ContextConfig::default(),
        }
    }

    pub fn with_config(config: ContextConfig) -> Self {
        Self { config }
    }

    pub fn load() -> Self {
        Self::load_with_diagnostics()
            .map(|outcome| outcome.context)
            .unwrap_or_default()
    }

    pub fn load_from_dir(path: &Path) -> Self {
        Self::load_from_dir_with_diagnostics(path)
            .map(|outcome| outcome.context)
            .unwrap_or_default()
    }

    #[doc(hidden)]
    pub fn from_sources(sources: ContextSources) -> Self {
        let mut config = ContextConfig::default();

        if let Some(default_device) = sources.default_device {
            config.user_defined_devices.push(default_device);
        }

        if let Some(file_config) = sources.config_file {
            let ContextConfig {
                allow_autoscan,
                allow_intrusive_scan,
                log_level,
                user_defined_devices,
            } = file_config;
            config.allow_autoscan = allow_autoscan;
            config.allow_intrusive_scan = allow_intrusive_scan;
            config.log_level = log_level;
            config.user_defined_devices.extend(user_defined_devices);
        }

        if let Some(selected_device) = sources.selected_device {
            config.user_defined_devices = vec![selected_device];
        }

        if let Some(allow_autoscan) = sources.allow_autoscan {
            config.allow_autoscan = allow_autoscan;
        }

        if let Some(allow_intrusive_scan) = sources.allow_intrusive_scan {
            config.allow_intrusive_scan = allow_intrusive_scan;
        }

        if let Some(log_level) = sources.log_level {
            config.log_level = log_level;
        }

        if let Some(max_user_defined_devices) = sources.max_user_defined_devices {
            config
                .user_defined_devices
                .truncate(max_user_defined_devices);
        }

        Self { config }
    }

    #[doc(hidden)]
    pub fn load_with_diagnostics() -> Result<ContextLoadOutcome, ContextLoadFailure> {
        load_context_internal(compiled_conf_root_if_enabled(), cfg!(libnfc_envvars))
    }

    #[doc(hidden)]
    pub fn load_from_dir_with_diagnostics(
        path: &Path,
    ) -> Result<ContextLoadOutcome, ContextLoadFailure> {
        load_context_internal(Some(path.to_path_buf()), cfg!(libnfc_envvars))
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

fn load_context_internal(
    conf_root: Option<PathBuf>,
    apply_env: bool,
) -> Result<ContextLoadOutcome, ContextLoadFailure> {
    let mut diagnostics = Vec::new();
    let mut parsed = ParsedContextConfig::with_defaults();

    if apply_env {
        if let Some(default_device_bytes) = env_var_bytes("LIBNFC_DEFAULT_DEVICE") {
            if default_device_bytes.len() >= NFC_BUFSIZE_CONNSTRING {
                let message =
                    "Failed to copy LIBNFC_DEFAULT_DEVICE environment variable".to_string();
                let diagnostic = ContextDiagnostic::general_error(message.clone());
                return Err(ContextLoadFailure {
                    diagnostics: vec![diagnostic],
                    last_error: Some(message),
                });
            }

            let default_device = String::from_utf8_lossy(&default_device_bytes).into_owned();
            if ConnectionString::new(default_device).is_ok() {
                parsed.user_defined_devices.push(RawUserDefinedDevice {
                    name: truncate_bytes(
                        USER_DEFINED_DEFAULT_DEVICE_NAME.as_bytes(),
                        DEVICE_NAME_LENGTH,
                    ),
                    connstring: truncate_bytes(&default_device_bytes, NFC_BUFSIZE_CONNSTRING),
                    optional: false,
                });
            }
        }
    }

    if let Some(root) = conf_root {
        load_config_from_root(&root, &mut parsed, &mut diagnostics);
    }

    let mut config = parsed.into_context_config();

    if apply_env {
        if let Some(selected_device) =
            env_user_defined_device("LIBNFC_DEVICE", USER_DEFINED_DEVICE_NAME.as_bytes())
        {
            config.user_defined_devices = vec![selected_device];
        }

        if let Some(allow_autoscan) = env_var_bytes("LIBNFC_AUTO_SCAN")
            .as_deref()
            .and_then(parse_boolean_env)
        {
            config.allow_autoscan = allow_autoscan;
        }

        if let Some(allow_intrusive_scan) = env_var_bytes("LIBNFC_INTRUSIVE_SCAN")
            .as_deref()
            .and_then(parse_boolean_env)
        {
            config.allow_intrusive_scan = allow_intrusive_scan;
        }

        if let Some(log_level) = env_var_bytes("LIBNFC_LOG_LEVEL").as_deref().map(atoi_bytes) {
            config.log_level = log_level;
        }
    }

    config
        .user_defined_devices
        .truncate(MAX_USER_DEFINED_DEVICES);

    Ok(ContextLoadOutcome {
        context: Context { config },
        diagnostics,
    })
}

fn compiled_conf_root_if_enabled() -> Option<PathBuf> {
    if cfg!(libnfc_conffiles) {
        configured_conf_root()
    } else {
        None
    }
}

fn compiled_conf_root() -> PathBuf {
    if let Some(path) = option_env!("PROXIMATE_CONFDIR") {
        PathBuf::from(path)
    } else if cfg!(windows) {
        PathBuf::from(DEFAULT_WINDOWS_CONFDIR)
    } else {
        PathBuf::from(DEFAULT_NON_WINDOWS_CONFDIR)
    }
}

fn configured_conf_root() -> Option<PathBuf> {
    TEST_CONF_ROOT.with(|cell| {
        cell.borrow()
            .as_ref()
            .cloned()
            .unwrap_or_else(|| Some(compiled_conf_root()))
    })
}

#[doc(hidden)]
pub fn set_test_conf_root(root: Option<PathBuf>) {
    TEST_CONF_ROOT.with(|cell| {
        *cell.borrow_mut() = Some(root);
    });
}

fn env_user_defined_device(key: &str, name: &[u8]) -> Option<UserDefinedDevice> {
    let value = env_var_bytes(key)?;
    let connstring = ConnectionString::new(String::from_utf8_lossy(&value).into_owned()).ok()?;
    Some(UserDefinedDevice {
        name: String::from_utf8_lossy(name).into_owned(),
        connstring,
        optional: false,
    })
}

fn env_var_bytes(key: &str) -> Option<Vec<u8>> {
    let value = env::var_os(key)?;

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Some(value.as_os_str().as_bytes().to_vec())
    }

    #[cfg(not(unix))]
    {
        Some(value.to_string_lossy().into_owned().into_bytes())
    }
}

fn load_config_from_root(
    root: &Path,
    parsed: &mut ParsedContextConfig,
    diagnostics: &mut Vec<ContextDiagnostic>,
) {
    parse_config_file(
        &root.join(LIBNFC_CONFFILE_NAME),
        conf_keyvalue_context,
        parsed,
        diagnostics,
    );
    load_device_configs(&root.join(LIBNFC_DEVICECONFDIR_NAME), parsed, diagnostics);
}

fn parse_boolean_env(value: &[u8]) -> Option<bool> {
    match value {
        b"yes" | b"true" | b"1" => Some(true),
        b"no" | b"false" | b"0" => Some(false),
        _ => None,
    }
}

fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

fn bytes_to_lossy_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn apply_boolean_bytes(value: &[u8], target: &mut bool) {
    if !(*target) {
        if value == b"yes" || value == b"true" || value == b"1" {
            *target = true;
        }
    } else if value == b"no" || value == b"false" || value == b"0" {
        *target = false;
    }
}

fn atoi_bytes(value: &[u8]) -> u32 {
    let mut index = 0usize;
    let mut sign = 1i64;

    if let Some(first) = value.first().copied() {
        match first {
            b'+' => index = 1,
            b'-' => {
                sign = -1;
                index = 1;
            }
            _ => {}
        }
    }

    let mut parsed = false;
    let mut result = 0i64;
    while index < value.len() && value[index].is_ascii_digit() {
        parsed = true;
        result = result
            .saturating_mul(10)
            .saturating_add((value[index] - b'0') as i64);
        index += 1;
    }

    if !parsed {
        return 0;
    }

    (result.saturating_mul(sign) as i32) as u32
}

fn parse_line(line: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let mut index = 0usize;

    while index < line.len() && is_space(line[index]) {
        index += 1;
    }

    if index >= line.len() || line[index] == b'\n' {
        return None;
    }

    let key_start = index;
    while index < line.len()
        && (line[index].is_ascii_alphanumeric() || matches!(line[index], b'_' | b'.'))
    {
        index += 1;
    }

    if index == key_start || index >= line.len() || line[index] == b'\n' {
        return None;
    }

    let key = line[key_start..index].to_vec();

    while index < line.len() && is_space(line[index]) {
        index += 1;
    }

    if index >= line.len() || line[index] != b'=' {
        return None;
    }
    index += 1;

    if index >= line.len() || line[index] == b'\n' {
        return None;
    }

    while index < line.len() && is_space(line[index]) {
        index += 1;
    }

    if index >= line.len() || line[index] == b'\n' {
        return None;
    }

    if line[index] == b'"' {
        index += 1;
        if index >= line.len() || line[index] == b'\n' {
            return None;
        }

        let value_start = index;
        while index < line.len() && line[index] != b'"' {
            index += 1;
        }

        if index >= line.len() || line[index] != b'"' {
            return None;
        }

        let value = line[value_start..index].to_vec();
        index += 1;

        while index < line.len() && is_space(line[index]) {
            index += 1;
        }

        if index < line.len() && line[index] != b'\n' {
            return None;
        }

        return Some((key, value));
    }

    let value_start = index;
    while index < line.len() && !is_space(line[index]) {
        index += 1;
    }

    let value = line[value_start..index].to_vec();

    if index < line.len() {
        index += 1;
        while index < line.len() && is_space(line[index]) {
            index += 1;
        }

        if index < line.len() {
            return None;
        }
    }

    Some((key, value))
}

fn last_device_name_empty(context: &ParsedContextConfig) -> bool {
    context
        .user_defined_devices
        .last()
        .is_none_or(|device| device.name.is_empty())
}

fn last_device_connstring_empty(context: &ParsedContextConfig) -> bool {
    context
        .user_defined_devices
        .last()
        .is_none_or(|device| device.connstring.is_empty())
}

fn last_device_optional(context: &ParsedContextConfig) -> bool {
    context
        .user_defined_devices
        .last()
        .is_some_and(|device| device.optional)
}

fn push_user_defined_device_slot(
    context: &mut ParsedContextConfig,
    diagnostics: &mut Vec<ContextDiagnostic>,
) -> Option<usize> {
    if context.user_defined_devices.len() >= MAX_USER_DEFINED_DEVICES {
        diagnostics.push(ContextDiagnostic::config_error(CONFIG_MAX_DEVICES_MESSAGE));
        return None;
    }

    context
        .user_defined_devices
        .push(RawUserDefinedDevice::default());
    Some(context.user_defined_devices.len() - 1)
}

fn current_device_index(
    context: &mut ParsedContextConfig,
    field: UserDeviceField,
    diagnostics: &mut Vec<ContextDiagnostic>,
) -> Option<usize> {
    let needs_new_slot = if context.user_defined_devices.is_empty() {
        true
    } else {
        match field {
            UserDeviceField::Name => !last_device_name_empty(context),
            UserDeviceField::Connstring => !last_device_connstring_empty(context),
            UserDeviceField::Optional => last_device_optional(context),
        }
    };

    if needs_new_slot {
        push_user_defined_device_slot(context, diagnostics)
    } else {
        Some(context.user_defined_devices.len() - 1)
    }
}

fn current_device_slot<'a>(
    context: &'a mut ParsedContextConfig,
    field: UserDeviceField,
    diagnostics: &mut Vec<ContextDiagnostic>,
) -> Option<&'a mut RawUserDefinedDevice> {
    let index = current_device_index(context, field, diagnostics)?;
    context.user_defined_devices.get_mut(index)
}

fn truncate_bytes(bytes: &[u8], dst_size: usize) -> Vec<u8> {
    let copy_len = bytes.len().min(dst_size.saturating_sub(1));
    bytes[..copy_len].to_vec()
}

fn conf_keyvalue_context(
    context: &mut ParsedContextConfig,
    key: &[u8],
    value: &[u8],
    diagnostics: &mut Vec<ContextDiagnostic>,
) {
    diagnostics.push(ContextDiagnostic::config_debug(format!(
        "key: [{}], value: [{}]",
        bytes_to_lossy_string(key),
        bytes_to_lossy_string(value)
    )));

    if key == b"allow_autoscan" {
        apply_boolean_bytes(value, &mut context.allow_autoscan);
        return;
    }

    if key == b"allow_intrusive_scan" {
        apply_boolean_bytes(value, &mut context.allow_intrusive_scan);
        return;
    }

    if key == b"log_level" {
        context.log_level = atoi_bytes(value);
        return;
    }

    if key == b"device.name" {
        let Some(device) = current_device_slot(context, UserDeviceField::Name, diagnostics) else {
            return;
        };
        device.name = truncate_bytes(value, DEVICE_NAME_LENGTH);
        return;
    }

    if key == b"device.connstring" {
        let Some(device) = current_device_slot(context, UserDeviceField::Connstring, diagnostics)
        else {
            return;
        };
        device.connstring = truncate_bytes(value, NFC_BUFSIZE_CONNSTRING);
        return;
    }

    if key == b"device.optional" {
        let Some(device) = current_device_slot(context, UserDeviceField::Optional, diagnostics)
        else {
            return;
        };
        if value == b"true" || value == b"True" || value == b"1" {
            device.optional = true;
        }
        return;
    }

    diagnostics.push(ContextDiagnostic::config_info(format!(
        "Unknown key in config line: {} = {}",
        bytes_to_lossy_string(key),
        bytes_to_lossy_string(value)
    )));
}

fn conf_keyvalue_device(
    context: &mut ParsedContextConfig,
    key: &[u8],
    value: &[u8],
    diagnostics: &mut Vec<ContextDiagnostic>,
) {
    let mut prefixed = b"device.".to_vec();
    prefixed.extend_from_slice(key);
    conf_keyvalue_context(context, &prefixed, value, diagnostics);
}

fn parse_config_file(
    filename: &Path,
    conf_keyvalue: fn(&mut ParsedContextConfig, &[u8], &[u8], &mut Vec<ContextDiagnostic>),
    context: &mut ParsedContextConfig,
    diagnostics: &mut Vec<ContextDiagnostic>,
) {
    let Ok(file) = File::open(filename) else {
        diagnostics.push(ContextDiagnostic::config_info(format!(
            "Unable to open file: {}",
            filename.display()
        )));
        return;
    };

    let mut reader = BufReader::new(file);
    let mut line = Vec::with_capacity(256);
    let mut line_number = 0usize;

    loop {
        line.clear();
        let Ok(read_len) = reader.read_until(b'\n', &mut line) else {
            break;
        };

        if read_len == 0 {
            break;
        }

        line_number += 1;

        if line
            .first()
            .copied()
            .is_some_and(|byte| matches!(byte, b'#' | b'\n'))
        {
            continue;
        }

        if line.len() > MAX_CONFIG_LINE_BYTES {
            diagnostics.push(ContextDiagnostic::config_debug(format!(
                "Parse error on line #{}: {}",
                line_number,
                bytes_to_lossy_string(&line)
            )));
            continue;
        }

        if let Some((key, value)) = parse_line(&line) {
            conf_keyvalue(context, &key, &value, diagnostics);
        } else {
            diagnostics.push(ContextDiagnostic::config_debug(format!(
                "Parse error on line #{}: {}",
                line_number,
                bytes_to_lossy_string(&line)
            )));
        }
    }
}

fn load_device_configs(
    dirname: &Path,
    context: &mut ParsedContextConfig,
    diagnostics: &mut Vec<ContextDiagnostic>,
) {
    let Ok(entries) = fs::read_dir(dirname) else {
        diagnostics.push(ContextDiagnostic::config_debug(format!(
            "Unable to open directory: {}",
            dirname.display()
        )));
        return;
    };

    for entry_result in entries {
        let Ok(entry) = entry_result else {
            continue;
        };

        let file_name = entry.file_name();
        let file_name_string = file_name.to_string_lossy();
        if file_name_string.starts_with('.') || !file_name_string.ends_with(".conf") {
            continue;
        }

        let Ok(metadata) = entry.metadata() else {
            continue;
        };

        if metadata.is_file() {
            parse_config_file(&entry.path(), conf_keyvalue_device, context, diagnostics);
        }
    }
}
