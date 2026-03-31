// build.rs — detect availability of secure-zero APIs and set cfg flags
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn env_flag_enabled(name: &str, default_enabled: bool) -> bool {
    println!("cargo:rerun-if-env-changed={}", name);

    match env::var(name) {
        Ok(value) => match value.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => default_enabled,
        },
        Err(_) => default_enabled,
    }
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

        println!("cargo:rustc-cfg={}", cfg_name);
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

fn pick_compiler_for_target() -> String {
    // Prefer an explicit CC if provided; otherwise pick a reasonable
    // default based on the TARGET triple so cross-compilation attempts
    // are more likely to use an appropriate toolchain when available.
    if let Ok(cc) = env::var("CC") {
        return cc;
    }
    match env::var("TARGET").unwrap_or_default().as_str() {
        t if t.contains("windows") => "x86_64-w64-mingw32-gcc".into(),
        t if t.contains("musl") => "musl-gcc".into(),
        _ => "cc".into(),
    }
}

fn have_function_in_libc(func: &str) -> bool {
    // Generate a minimal C program that references the requested symbol and
    // attempt to compile *and link* it. Linking ensures we do not report
    // false positives when the prototype exists but the implementation is
    // missing from libc for the current target (common when cross-compiling).
    let out_dir = env::var("OUT_DIR").unwrap_or_else(|_| ".".into());

    let snippet = match func {
        "explicit_bzero" => {
            "#define _GNU_SOURCE\n#include <string.h>\nint main(void) { explicit_bzero((void*)0, 0); return 0; }\n"
        }
        "memset_s" => {
            "#define __STDC_WANT_LIB_EXT1__ 1\n#include <string.h>\nint main(void) { size_t s = 0; return memset_s((void*)0, s, 0, s); }\n"
        }
        "memset_explicit" => {
            "#include <string.h>\nint main(void) { memset_explicit((void*)0, 0, 0); return 0; }\n"
        }
        "secure_zero_memory" => {
            "#include <windows.h>\nint main(void) { SecureZeroMemory((void*)0, 0); return 0; }\n"
        }
        _ => return false,
    };

    let test_file = std::path::Path::new(&out_dir).join(format!("check_{}.c", func));
    if std::fs::write(&test_file, snippet).is_err() {
        return false;
    }

    let cc = pick_compiler_for_target();
    let target = env::var("TARGET").unwrap_or_default();
    let exe_ext = if target.contains("windows") {
        ".exe"
    } else {
        ""
    };
    let exe_path = std::path::Path::new(&out_dir).join(format!("check_{}{}", func, exe_ext));

    let status = Command::new(&cc)
        .arg(&test_file)
        .arg("-o")
        .arg(&exe_path)
        .status();

    let success = matches!(status, Ok(s) if s.success());

    // Best-effort cleanup of temporary files; ignore errors so detection
    // results are driven solely by the compilation/link step above.
    let _ = std::fs::remove_file(&test_file);
    let _ = std::fs::remove_file(&exe_path);

    success
}

fn main() {
    // Always announce the cfg names so `rustc --check-cfg` does not warn
    // regardless of detection results. This keeps tools like cbindgen from
    // emitting spurious "missing defines" warnings.
    for cfg_name in &[
        "have_memset_explicit",
        "have_memset_s",
        "have_explicit_bzero",
        "have_secure_zero_memory",
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
        println!("cargo:rustc-check-cfg=cfg({})", cfg_name);
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

    println!("cargo:rerun-if-env-changed=PROXIMATE_CONFDIR");
    if let Ok(confdir) = env::var("PROXIMATE_CONFDIR") {
        println!("cargo:rustc-env=PROXIMATE_CONFDIR={}", confdir);
    }

    emit_enabled_driver_cfgs();
    emit_version_envs();

    // Conservative default: do not assume availability.
    // Check for explicit_bzero, memset_s and SecureZeroMemory by trying to compile.
    // Note: cross-compilation may not allow executing the compiled binary, but the
    // compiler will still accept references to external symbols, so the compile
    // should succeed if symbols are available in the headers/libraries.

    // Use the TARGET triple to decide detection strategy rather than the
    // build host. This makes build.rs robust when cross-compiling.
    let target = env::var("TARGET").unwrap_or_default();
    let _ = std::fs::create_dir_all(env::var("OUT_DIR").unwrap());

    if target.contains("windows") {
        // Windows targets expose SecureZeroMemory in system libraries.
        println!("cargo:rustc-cfg=have_secure_zero_memory");
    } else {
        // For Unix-like targets attempt to detect C23/C11/BSD primitives.
        if have_function_in_libc("memset_explicit") {
            println!("cargo:rustc-cfg=have_memset_explicit");
        }
        if have_function_in_libc("memset_s") {
            println!("cargo:rustc-cfg=have_memset_s");
        }
        if have_function_in_libc("explicit_bzero") {
            println!("cargo:rustc-cfg=have_explicit_bzero");
        }
    }
}
