use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn env_flag_enabled(name: &str, default_enabled: bool) -> bool {
    println!("cargo:rerun-if-env-changed={name}");

    env::var(name).map_or(default_enabled, |value| {
        match value.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => default_enabled,
        }
    })
}

fn emit_enabled_driver_cfgs() {
    println!("cargo:rerun-if-env-changed=PROXIMATE_ENABLED_DRIVERS");

    let Ok(value) = env::var("PROXIMATE_ENABLED_DRIVERS") else {
        return;
    };

    for raw_driver in value.split(',') {
        let driver = raw_driver.trim();
        if driver.is_empty() {
            continue;
        }

        let cfg_name = match driver {
            "pcsc" => "libnfc_driver_pcsc",
            "acr122_pcsc" => "libnfc_driver_acr122_pcsc",
            "acr122_usb" => "libnfc_driver_acr122_usb",
            "acr122s" => "libnfc_driver_acr122s",
            "pn53x_usb" => "libnfc_driver_pn53x_usb",
            "arygon" => "libnfc_driver_arygon",
            "pn532_uart" => "libnfc_driver_pn532_uart",
            "pn532_spi" => "libnfc_driver_pn532_spi",
            "pn532_i2c" => "libnfc_driver_pn532_i2c",
            "pn71xx" => "libnfc_driver_pn71xx",
            _ => continue,
        };

        println!("cargo:rustc-cfg={cfg_name}");
    }
}

fn workspace_root() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").ok()?);
    manifest_dir.parent()?.parent().map(Path::to_path_buf)
}

fn detect_package_version_from_cmake(root: &Path) -> Option<String> {
    let cmake_lists = root.join("CMakeLists.txt");
    println!("cargo:rerun-if-changed={}", cmake_lists.display());

    let contents = fs::read_to_string(cmake_lists).ok()?;
    let project_line = contents
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("project(") && line.contains(" VERSION "))?;
    let version_tail = project_line.split(" VERSION ").nth(1)?;
    let version = version_tail
        .split(|ch: char| ch.is_whitespace() || ch == ')')
        .find(|token| !token.is_empty())?;
    Some(version.to_string())
}

fn detect_git_revision(root: &Path) -> Option<String> {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("describe")
        .arg("--always")
        .arg("--dirty")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            let value = String::from_utf8(output.stdout).ok()?;
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
}

fn emit_version_envs() {
    println!("cargo:rerun-if-env-changed=PROXIMATE_PACKAGE_VERSION");
    println!("cargo:rerun-if-env-changed=PROXIMATE_GIT_REVISION");

    let root = workspace_root();
    let package_version = env::var("PROXIMATE_PACKAGE_VERSION")
        .ok()
        .or_else(|| root.as_deref().and_then(detect_package_version_from_cmake))
        .unwrap_or_else(|| env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "unknown".into()));
    println!("cargo:rustc-env=PROXIMATE_PACKAGE_VERSION={package_version}");

    let git_revision = env::var("PROXIMATE_GIT_REVISION")
        .ok()
        .or_else(|| root.as_deref().and_then(detect_git_revision));
    if let Some(git_revision) = git_revision.filter(|value| !value.is_empty()) {
        println!("cargo:rustc-env=PROXIMATE_GIT_REVISION={git_revision}");
    }
}

fn main() {
    for cfg_name in &[
        "cbindgen",
        "libnfc_conffiles",
        "libnfc_debug",
        "libnfc_envvars",
        "libnfc_external_bridges",
        "libnfc_log",
        "libnfc_driver_pcsc",
        "libnfc_driver_acr122_pcsc",
        "libnfc_driver_acr122_usb",
        "libnfc_driver_acr122s",
        "libnfc_driver_pn53x_usb",
        "libnfc_driver_arygon",
        "libnfc_driver_pn532_uart",
        "libnfc_driver_pn532_spi",
        "libnfc_driver_pn532_i2c",
        "libnfc_driver_pn71xx",
    ] {
        println!("cargo:rustc-check-cfg=cfg({cfg_name})");
    }

    if env_flag_enabled("PROXIMATE_WITH_ENVVARS", true) {
        println!("cargo:rustc-cfg=libnfc_envvars");
    }
    if env_flag_enabled("PROXIMATE_WITH_CONFFILES", true) {
        println!("cargo:rustc-cfg=libnfc_conffiles");
    }
    if env_flag_enabled("PROXIMATE_WITH_LOG", true) {
        println!("cargo:rustc-cfg=libnfc_log");
    }
    if env_flag_enabled("PROXIMATE_WITH_DEBUG", false) {
        println!("cargo:rustc-cfg=libnfc_debug");
    }
    if env_flag_enabled("PROXIMATE_EXTERNAL_BRIDGES", false) {
        println!("cargo:rustc-cfg=libnfc_external_bridges");
    }

    emit_enabled_driver_cfgs();
    emit_version_envs();
}
