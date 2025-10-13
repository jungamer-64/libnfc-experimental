// build.rs — detect availability of secure-zero APIs and set cfg flags
use std::env;
use std::process::Command;

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
        "explicit_bzero" => "#define _GNU_SOURCE\n#include <string.h>\nint main(void) { explicit_bzero((void*)0, 0); return 0; }\n",
        "memset_s" => "#define __STDC_WANT_LIB_EXT1__ 1\n#include <string.h>\nint main(void) { size_t s = 0; return memset_s((void*)0, s, 0, s); }\n",
        "memset_explicit" => "#include <string.h>\nint main(void) { memset_explicit((void*)0, 0, 0); return 0; }\n",
        "secure_zero_memory" => "#include <windows.h>\nint main(void) { SecureZeroMemory((void*)0, 0); return 0; }\n",
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
    ] {
        println!("cargo:rustc-check-cfg=cfg({})", cfg_name);
    }
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
