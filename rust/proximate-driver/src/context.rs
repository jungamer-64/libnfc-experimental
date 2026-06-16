use std::env;
use std::fmt;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use proximate_types::{ConnectionString, NFC_BUFSIZE_CONNSTRING};

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

#[derive(Clone, Debug, Eq, PartialEq)]
enum ConfRoot {
    Disabled,
    Override(PathBuf),
}

thread_local! {
    static TEST_CONF_ROOT: std::cell::RefCell<Option<ConfRoot>> =
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextLoadError {
    message: String,
}

impl ContextLoadError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ContextLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ContextLoadError {}

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
    fn new(
        category: ContextDiagnosticCategory,
        priority: ContextDiagnosticPriority,
        message: impl Into<String>,
    ) -> Self {
        Self {
            category,
            priority,
            message: message.into(),
        }
    }

    fn general_error(message: impl Into<String>) -> Self {
        Self::new(
            ContextDiagnosticCategory::General,
            ContextDiagnosticPriority::Error,
            message,
        )
    }

    fn general_info(message: impl Into<String>) -> Self {
        Self::new(
            ContextDiagnosticCategory::General,
            ContextDiagnosticPriority::Info,
            message,
        )
    }

    fn config_error(message: impl Into<String>) -> Self {
        Self::new(
            ContextDiagnosticCategory::Config,
            ContextDiagnosticPriority::Error,
            message,
        )
    }

    fn config_info(message: impl Into<String>) -> Self {
        Self::new(
            ContextDiagnosticCategory::Config,
            ContextDiagnosticPriority::Info,
            message,
        )
    }

    fn config_debug(message: impl Into<String>) -> Self {
        Self::new(
            ContextDiagnosticCategory::Config,
            ContextDiagnosticPriority::Debug,
            message,
        )
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
    pub error: ContextLoadError,
}

impl ContextLoadFailure {
    fn new(diagnostics: Vec<ContextDiagnostic>, error: ContextLoadError) -> Self {
        Self { diagnostics, error }
    }
}

impl From<ContextLoadFailure> for ContextLoadError {
    fn from(value: ContextLoadFailure) -> Self {
        value.error
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ContextConfigSource {
    pub allow_autoscan: Option<bool>,
    pub allow_intrusive_scan: Option<bool>,
    pub log_level: Option<u32>,
    pub user_defined_devices: Vec<UserDefinedDevice>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ContextScalarOverrides {
    pub allow_autoscan: Option<bool>,
    pub allow_intrusive_scan: Option<bool>,
    pub log_level: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ContextConfigBuilder {
    config: ContextConfig,
}

impl ContextConfigBuilder {
    pub(crate) fn new() -> Self {
        Self {
            config: ContextConfig::default(),
        }
    }

    pub(crate) fn apply_default_device(&mut self, device: Option<UserDefinedDevice>) {
        if let Some(device) = device {
            self.config.user_defined_devices.push(device);
        }
    }

    pub(crate) fn apply_source(&mut self, source: ContextConfigSource) {
        let ContextConfigSource {
            allow_autoscan,
            allow_intrusive_scan,
            log_level,
            user_defined_devices,
        } = source;

        if let Some(value) = allow_autoscan {
            self.config.allow_autoscan = value;
        }

        if let Some(value) = allow_intrusive_scan {
            self.config.allow_intrusive_scan = value;
        }

        if let Some(value) = log_level {
            self.config.log_level = value;
        }

        self.config
            .user_defined_devices
            .extend(user_defined_devices);
    }

    pub(crate) fn apply_selected_device(&mut self, device: Option<UserDefinedDevice>) {
        if let Some(device) = device {
            self.config.user_defined_devices = vec![device];
        }
    }

    pub(crate) fn apply_scalar_overrides(&mut self, overrides: ContextScalarOverrides) {
        if let Some(value) = overrides.allow_autoscan {
            self.config.allow_autoscan = value;
        }

        if let Some(value) = overrides.allow_intrusive_scan {
            self.config.allow_intrusive_scan = value;
        }

        if let Some(value) = overrides.log_level {
            self.config.log_level = value;
        }
    }

    pub(crate) fn cap_user_defined_devices(
        &mut self,
        max: usize,
        diagnostics: &mut Vec<ContextDiagnostic>,
    ) {
        if self.config.user_defined_devices.len() > max {
            diagnostics.push(ContextDiagnostic::config_error(CONFIG_MAX_DEVICES_MESSAGE));
            self.config.user_defined_devices.truncate(max);
        }
    }

    pub(crate) fn build(self) -> ContextConfig {
        self.config
    }
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

    pub fn try_load() -> Result<Self, ContextLoadError> {
        load_context_with_diagnostics()
            .map(|outcome| outcome.context)
            .map_err(ContextLoadError::from)
    }

    pub fn load() -> Self {
        Self::load_or_default()
    }

    pub fn load_or_default() -> Self {
        Self::try_load().unwrap_or_default()
    }

    pub fn try_load_from_dir(path: &Path) -> Result<Self, ContextLoadError> {
        load_context_from_dir_with_diagnostics(path)
            .map(|outcome| outcome.context)
            .map_err(ContextLoadError::from)
    }

    pub fn load_from_dir(path: &Path) -> Self {
        Self::load_from_dir_or_default(path)
    }

    pub fn load_from_dir_or_default(path: &Path) -> Self {
        Self::try_load_from_dir(path).unwrap_or_default()
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct UserDefinedDeviceDraft {
    name: Option<String>,
    connstring: Option<ConnectionString>,
    optional: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ParsedConfigSource {
    allow_autoscan: Option<bool>,
    allow_intrusive_scan: Option<bool>,
    log_level: Option<u32>,
    user_defined_devices: Vec<UserDefinedDeviceDraft>,
}

impl ParsedConfigSource {
    fn into_source(self) -> ContextConfigSource {
        ContextConfigSource {
            allow_autoscan: self.allow_autoscan,
            allow_intrusive_scan: self.allow_intrusive_scan,
            log_level: self.log_level,
            user_defined_devices: self
                .user_defined_devices
                .into_iter()
                .filter_map(|device| {
                    let connstring = device.connstring?;
                    Some(UserDefinedDevice {
                        name: device.name.unwrap_or_default(),
                        connstring,
                        optional: device.optional,
                    })
                })
                .collect(),
        }
    }
}

#[derive(Clone, Copy)]
enum UserDeviceField {
    Name,
    Connstring,
    Optional,
}

#[derive(Clone, Copy)]
enum ConfigFileKind {
    Context,
    Device,
}

impl ConfigFileKind {
    fn normalize_key(self, key: &str) -> String {
        match self {
            Self::Context => key.to_owned(),
            Self::Device => format!("device.{key}"),
        }
    }
}

#[doc(hidden)]
pub fn load_context_with_diagnostics() -> Result<ContextLoadOutcome, ContextLoadFailure> {
    load_context_internal(compiled_conf_root_if_enabled(), cfg!(libnfc_envvars))
}

#[doc(hidden)]
pub fn load_context_from_dir_with_diagnostics(
    path: &Path,
) -> Result<ContextLoadOutcome, ContextLoadFailure> {
    load_context_internal(Some(path.to_path_buf()), cfg!(libnfc_envvars))
}

fn load_context_internal(
    conf_root: Option<PathBuf>,
    apply_env: bool,
) -> Result<ContextLoadOutcome, ContextLoadFailure> {
    let mut diagnostics = Vec::new();

    let default_device = if apply_env {
        default_device_from_env(&mut diagnostics)?
    } else {
        None
    };

    let config_source = if let Some(root) = conf_root {
        load_config_from_root(&root, &mut diagnostics)
    } else {
        ContextConfigSource::default()
    };

    let selected_device = if apply_env {
        env_user_defined_device("LIBNFC_DEVICE", USER_DEFINED_DEVICE_NAME, &mut diagnostics)
    } else {
        None
    };

    let scalar_overrides = if apply_env {
        env_scalar_overrides(&mut diagnostics)
    } else {
        ContextScalarOverrides::default()
    };

    let mut builder = ContextConfigBuilder::new();
    builder.apply_default_device(default_device);
    builder.apply_source(config_source);
    builder.apply_selected_device(selected_device);
    builder.apply_scalar_overrides(scalar_overrides);
    builder.cap_user_defined_devices(MAX_USER_DEFINED_DEVICES, &mut diagnostics);

    Ok(ContextLoadOutcome {
        context: Context {
            config: builder.build(),
        },
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
    TEST_CONF_ROOT.with(|cell| match cell.borrow().clone() {
        None => Some(compiled_conf_root()),
        Some(ConfRoot::Disabled) => None,
        Some(ConfRoot::Override(path)) => Some(path),
    })
}

#[doc(hidden)]
pub fn set_test_conf_root(root: Option<PathBuf>) {
    TEST_CONF_ROOT.with(|cell| {
        *cell.borrow_mut() = Some(match root {
            Some(path) => ConfRoot::Override(path),
            None => ConfRoot::Disabled,
        });
    });
}

fn default_device_from_env(
    diagnostics: &mut Vec<ContextDiagnostic>,
) -> Result<Option<UserDefinedDevice>, ContextLoadFailure> {
    let Some(value) = env_var_bytes("LIBNFC_DEFAULT_DEVICE") else {
        return Ok(None);
    };

    if value.len() >= NFC_BUFSIZE_CONNSTRING {
        let message = "Failed to copy LIBNFC_DEFAULT_DEVICE environment variable";
        let diagnostic = ContextDiagnostic::general_error(message);
        return Err(ContextLoadFailure::new(
            vec![diagnostic],
            ContextLoadError::new(message),
        ));
    }

    Ok(user_defined_device_from_bytes(
        "LIBNFC_DEFAULT_DEVICE",
        USER_DEFINED_DEFAULT_DEVICE_NAME,
        &value,
        diagnostics,
    ))
}

fn env_user_defined_device(
    key: &str,
    name: &str,
    diagnostics: &mut Vec<ContextDiagnostic>,
) -> Option<UserDefinedDevice> {
    let value = env_var_bytes(key)?;
    user_defined_device_from_bytes(key, name, &value, diagnostics)
}

fn user_defined_device_from_bytes(
    source_name: &str,
    name: &str,
    value: &[u8],
    diagnostics: &mut Vec<ContextDiagnostic>,
) -> Option<UserDefinedDevice> {
    let value = bytes_to_lossy_string(value);
    match ConnectionString::new(value.clone()) {
        Ok(connstring) => Some(UserDefinedDevice {
            name: name.to_owned(),
            connstring,
            optional: false,
        }),
        Err(_) => {
            diagnostics.push(ContextDiagnostic::general_info(format!(
                "Ignoring invalid {source_name} connection string: {value}"
            )));
            None
        }
    }
}

fn env_scalar_overrides(diagnostics: &mut Vec<ContextDiagnostic>) -> ContextScalarOverrides {
    ContextScalarOverrides {
        allow_autoscan: env_bool_override("LIBNFC_AUTO_SCAN", diagnostics),
        allow_intrusive_scan: env_bool_override("LIBNFC_INTRUSIVE_SCAN", diagnostics),
        log_level: env_var_bytes("LIBNFC_LOG_LEVEL").as_deref().map(atoi_bytes),
    }
}

fn env_bool_override(key: &str, diagnostics: &mut Vec<ContextDiagnostic>) -> Option<bool> {
    let value = env_var_bytes(key)?;
    match parse_boolean_env(&value) {
        Some(value) => Some(value),
        None => {
            diagnostics.push(ContextDiagnostic::general_info(format!(
                "Ignoring invalid {key} environment variable: {}",
                bytes_to_lossy_string(&value)
            )));
            None
        }
    }
}

fn env_var_bytes(key: &str) -> Option<Vec<u8>> {
    Some(
        env::var_os(key)?
            .to_string_lossy()
            .into_owned()
            .into_bytes(),
    )
}

fn load_config_from_root(
    root: &Path,
    diagnostics: &mut Vec<ContextDiagnostic>,
) -> ContextConfigSource {
    let mut source = ParsedConfigSource::default();
    parse_config_file(
        &root.join(LIBNFC_CONFFILE_NAME),
        ConfigFileKind::Context,
        &mut source,
        diagnostics,
    );
    load_device_configs(
        &root.join(LIBNFC_DEVICECONFDIR_NAME),
        &mut source,
        diagnostics,
    );
    source.into_source()
}

fn parse_boolean_env(value: &[u8]) -> Option<bool> {
    match value {
        b"yes" | b"true" | b"1" => Some(true),
        b"no" | b"false" | b"0" => Some(false),
        _ => None,
    }
}

fn parse_config_boolean(value: &str) -> Option<bool> {
    match value {
        "yes" | "true" | "1" => Some(true),
        "no" | "false" | "0" => Some(false),
        _ => None,
    }
}

fn parse_optional_boolean(value: &str) -> Option<bool> {
    match value {
        "true" | "True" | "1" => Some(true),
        "false" | "False" | "0" => Some(false),
        _ => None,
    }
}

fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

fn bytes_to_lossy_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn truncate_bytes(bytes: &[u8], dst_size: usize) -> Vec<u8> {
    let copy_len = bytes.len().min(dst_size.saturating_sub(1));
    bytes[..copy_len].to_vec()
}

fn truncate_string(value: &str, dst_size: usize) -> String {
    bytes_to_lossy_string(&truncate_bytes(value.as_bytes(), dst_size))
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

fn parse_line(line: &[u8]) -> Option<(String, String)> {
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

    let key = bytes_to_lossy_string(&line[key_start..index]);

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

        let value = bytes_to_lossy_string(&line[value_start..index]);
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

    let value = bytes_to_lossy_string(&line[value_start..index]);

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

fn last_device_name_empty(context: &ParsedConfigSource) -> bool {
    context
        .user_defined_devices
        .last()
        .is_none_or(|device| device.name.is_none())
}

fn last_device_connstring_empty(context: &ParsedConfigSource) -> bool {
    context
        .user_defined_devices
        .last()
        .is_none_or(|device| device.connstring.is_none())
}

fn last_device_optional(context: &ParsedConfigSource) -> bool {
    context
        .user_defined_devices
        .last()
        .is_some_and(|device| device.optional)
}

fn push_user_defined_device_slot(context: &mut ParsedConfigSource) -> usize {
    context
        .user_defined_devices
        .push(UserDefinedDeviceDraft::default());
    context.user_defined_devices.len() - 1
}

fn current_device_index(context: &mut ParsedConfigSource, field: UserDeviceField) -> usize {
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
        push_user_defined_device_slot(context)
    } else {
        context.user_defined_devices.len() - 1
    }
}

fn current_device_slot(
    context: &mut ParsedConfigSource,
    field: UserDeviceField,
) -> &mut UserDefinedDeviceDraft {
    let index = current_device_index(context, field);
    &mut context.user_defined_devices[index]
}

fn parse_config_connstring(
    key: &str,
    value: &str,
    diagnostics: &mut Vec<ContextDiagnostic>,
) -> Option<ConnectionString> {
    let value = truncate_string(value, NFC_BUFSIZE_CONNSTRING);
    match ConnectionString::new(value.clone()) {
        Ok(connstring) => Some(connstring),
        Err(_) => {
            diagnostics.push(ContextDiagnostic::config_info(format!(
                "Ignoring invalid connection string in config line: {key} = {value}"
            )));
            None
        }
    }
}

fn apply_config_key_value(
    context: &mut ParsedConfigSource,
    key: &str,
    value: &str,
    diagnostics: &mut Vec<ContextDiagnostic>,
) {
    diagnostics.push(ContextDiagnostic::config_debug(format!(
        "key: [{key}], value: [{value}]"
    )));

    match key {
        "allow_autoscan" => match parse_config_boolean(value) {
            Some(value) => context.allow_autoscan = Some(value),
            None => diagnostics.push(ContextDiagnostic::config_info(format!(
                "Ignoring invalid boolean in config line: {key} = {value}"
            ))),
        },
        "allow_intrusive_scan" => match parse_config_boolean(value) {
            Some(value) => context.allow_intrusive_scan = Some(value),
            None => diagnostics.push(ContextDiagnostic::config_info(format!(
                "Ignoring invalid boolean in config line: {key} = {value}"
            ))),
        },
        "log_level" => {
            context.log_level = Some(atoi_bytes(value.as_bytes()));
        }
        "device.name" => {
            let device = current_device_slot(context, UserDeviceField::Name);
            device.name = Some(truncate_string(value, DEVICE_NAME_LENGTH));
        }
        "device.connstring" => {
            let parsed = parse_config_connstring(key, value, diagnostics);
            let device = current_device_slot(context, UserDeviceField::Connstring);
            device.connstring = parsed;
        }
        "device.optional" => match parse_optional_boolean(value) {
            Some(value) => {
                let device = current_device_slot(context, UserDeviceField::Optional);
                device.optional = value;
            }
            None => diagnostics.push(ContextDiagnostic::config_info(format!(
                "Ignoring invalid boolean in config line: {key} = {value}"
            ))),
        },
        _ => diagnostics.push(ContextDiagnostic::config_info(format!(
            "Unknown key in config line: {key} = {value}"
        ))),
    }
}

fn parse_config_file(
    filename: &Path,
    kind: ConfigFileKind,
    context: &mut ParsedConfigSource,
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
            let normalized_key = kind.normalize_key(&key);
            apply_config_key_value(context, &normalized_key, &value, diagnostics);
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
    context: &mut ParsedConfigSource,
    diagnostics: &mut Vec<ContextDiagnostic>,
) {
    let Ok(entries) = fs::read_dir(dirname) else {
        diagnostics.push(ContextDiagnostic::config_debug(format!(
            "Unable to open directory: {}",
            dirname.display()
        )));
        return;
    };

    let mut files = Vec::new();
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
            files.push(entry.path());
        }
    }

    files.sort();
    for file in files {
        parse_config_file(&file, ConfigFileKind::Device, context, diagnostics);
    }
}
