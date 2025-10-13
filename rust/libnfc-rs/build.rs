// build.rs — detect availability of secure-zero APIs and set cfg flags
use std::env;
use std::process::Command;

fn have_function_in_libc(func: &str) -> bool {
    // Try to compile a small C program that references the function.
    // Use cc crate when available, but keep this minimal to avoid adding dependencies.
    let out_dir = env::var("OUT_DIR").unwrap_or_else(|_| ".".into());
    let test_c = format!(
        "#include <string.h>\nint main() {{ void *p = (void*)(&{func}); (void)p; return 0; }}\n",
        func = func
    );
    let test_file = std::path::Path::new(&out_dir).join(format!("check_{}.c", func));
    if std::fs::write(&test_file, test_c).is_err() {
        return false;
    }
    let cc = env::var("CC").unwrap_or_else(|_| "cc".into());
    let exe = std::path::Path::new(&out_dir).join(format!("check_{}", func));
    let status = Command::new(cc).arg(test_file).arg("-o").arg(&exe).status();
    match status {
        Ok(s) => s.success(),
        Err(_) => false,
    }
}

fn main() {
    // Tell rustc to not warn about our custom cfg names during check-cfg
    println!("cargo:rustc-check-cfg=cfg(have_secure_zero_memory)");
    println!("cargo:rustc-check-cfg=cfg(have_explicit_bzero)");
    println!("cargo:rustc-check-cfg=cfg(have_memset_s)");
    // Conservative default: do not assume availability.
    // Check for explicit_bzero, memset_s and SecureZeroMemory by trying to compile.
    // Note: cross-compilation may not allow executing the compiled binary, but the
    // compiler will still accept references to external symbols, so the compile
    // should succeed if symbols are available in the headers/libraries.

    // Only try to probe on non-windows for explicit_bzero and memset_s
    if cfg!(not(target_os = "windows")) {
        // probe explicit_bzero
        let _ = std::fs::create_dir_all(env::var("OUT_DIR").unwrap());
        if have_function_in_libc("explicit_bzero") {
            println!("cargo:rustc-cfg=have_explicit_bzero");
            println!("cargo:rustc-check-cfg=cfg(have_explicit_bzero)");
        }
        if have_function_in_libc("memset_s") {
            println!("cargo:rustc-cfg=have_memset_s");
            println!("cargo:rustc-check-cfg=cfg(have_memset_s)");
        }
    } else {
        // Windows: assume SecureZeroMemory is present in kernel32/lib
        println!("cargo:rustc-cfg=have_secure_zero_memory");
        println!("cargo:rustc-check-cfg=cfg(have_secure_zero_memory)");
    }
}
