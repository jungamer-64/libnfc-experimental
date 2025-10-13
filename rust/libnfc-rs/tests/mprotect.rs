// tests/mprotect.rs

// Integration tests that simulate memory protection failure scenarios
// without using fork. The parent test spawns the test binary itself
// with a filter so only the helper test is executed in a separate
// process; the helper inspects environment variables to decide which
// mprotect scenario to run.

#[cfg(unix)]
mod tests {
    use libc::c_char;
    use std::env;
    use std::ffi::c_void;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Command;
    use std::ptr;

    // Provide a no-op C symbol for the library's log hook so the
    // integration test binary links cleanly. The production build
    // links to a real logging implementation; integration tests only
    // need a stub.
    #[no_mangle]
    pub extern "C" fn log_put_message(
        _group: u8,
        _category: *const c_char,
        _priority: u8,
        _message: *const c_char,
    ) {
        // intentionally no-op
    }

    // Helper test executed inside a subprocess. The parent harness will
    // spawn the current test executable with the environment variable
    // `LIBNFC_MPROTECT_CHILD=1` set, and a second var selecting the
    // scenario: `LIBNFC_MPROTECT_CASE=readonly` or `=none`.
    #[test]
    fn mprotect_child_helper() {
        // Only run helper logic when explicitly invoked by the parent
        // harness to avoid executing it during normal test runs.
        if env::var("LIBNFC_MPROTECT_CHILD").ok().as_deref() != Some("1") {
            return;
        }

        let case = env::var("LIBNFC_MPROTECT_CASE").unwrap_or_else(|_| "readonly".into());
        let pagesize = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize };
        let len = pagesize;
        let mem = unsafe {
            libc::mmap(
                ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        if mem == libc::MAP_FAILED {
            eprintln!("mmap failed: {}", std::io::Error::last_os_error());
            std::process::exit(10);
        }
        // Initialize to non-zero so we can detect writes
        unsafe { ptr::write_bytes(mem as *mut u8, 0xFFu8, len) };

        let prot = match case.as_str() {
            "readonly" => libc::PROT_READ,
            "none" => libc::PROT_NONE,
            other => {
                eprintln!("unknown case: {}", other);
                std::process::exit(12);
            }
        };

        if unsafe { libc::mprotect(mem, len, prot) } != 0 {
            eprintln!(
                "mprotect to prot {} failed: {}",
                prot,
                std::io::Error::last_os_error()
            );
            std::process::exit(11);
        }

        // Attempt to write using the secure API. The call may either
        // return an error code (non-success), or it may cause the
        // process to be terminated by a signal; treat either case as a
        // valid handling of the protected region. If the API returns
        // success, treat that as unexpected and exit non-zero.
        let rc = unsafe { libnfc_rs::nfc_secure_memset(mem as *mut c_void, 0x5, len) };
        if rc != libnfc_rs::NFC_SECURE_SUCCESS {
            eprintln!("nfc_secure_memset returned error code: {}", rc);
            std::process::exit(0);
        }
        eprintln!("nfc_secure_memset unexpectedly returned success");
        std::process::exit(1);
    }

    #[test]
    fn mprotect_write_protection_detected() {
        // Spawn the same test executable but instruct it to run only
        // the helper test. The child will perform the mprotect test and
        // exit with 0 for handled error or signal for a crash.
        let exe = std::env::current_exe().expect("current_exe");
        let mut cmd = Command::new(exe);
        cmd.arg("mprotect_child_helper");
        cmd.env("LIBNFC_MPROTECT_CHILD", "1");
        cmd.env("LIBNFC_MPROTECT_CASE", "readonly");
        let out = cmd.output().expect("failed to spawn child");
        let status = out.status;
        if status.success() {
            // child indicated handled error
            return;
        }
        if let Some(sig) = status.signal() {
            if sig == libc::SIGSEGV || sig == libc::SIGBUS {
                let so = String::from_utf8_lossy(&out.stdout);
                let se = String::from_utf8_lossy(&out.stderr);
                eprintln!("child signaled {}. stdout='{}' stderr='{}'", sig, so, se);
                return;
            }
            panic!(
                "child died with unexpected signal {}. stdout='{}' stderr='{}'",
                sig,
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        // Non-zero exit code: include child's stdout/stderr to aid debugging
        panic!(
            "child exited with code {:?}. stdout='{}' stderr='{}'",
            status.code(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn mprotect_none_protection_detected() {
        let exe = std::env::current_exe().expect("current_exe");
        let mut cmd = Command::new(exe);
        cmd.arg("mprotect_child_helper");
        cmd.env("LIBNFC_MPROTECT_CHILD", "1");
        cmd.env("LIBNFC_MPROTECT_CASE", "none");
        let out = cmd.output().expect("failed to spawn child");
        let status = out.status;
        if status.success() {
            return;
        }
        if let Some(sig) = status.signal() {
            if sig == libc::SIGSEGV || sig == libc::SIGBUS {
                let so = String::from_utf8_lossy(&out.stdout);
                let se = String::from_utf8_lossy(&out.stderr);
                eprintln!("child signaled {}. stdout='{}' stderr='{}'", sig, so, se);
                return;
            }
            panic!(
                "child died with unexpected signal {}. stdout='{}' stderr='{}'",
                sig,
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        panic!(
            "child exited with code {:?}. stdout='{}' stderr='{}'",
            status.code(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[cfg(windows)]
mod tests_windows {
    use std::env;
    use std::ffi::c_void;
    use std::os::raw::c_char;
    use std::os::windows::process::ExitStatusExt;
    use std::process::Command;
    use std::ptr;

    // Windows API bindings we need for allocation/protection
    #[link(name = "kernel32")]
    extern "system" {
        fn VirtualAlloc(
            lpAddress: *mut c_void,
            dwSize: usize,
            flAllocationType: u32,
            flProtect: u32,
        ) -> *mut c_void;
        fn VirtualProtect(
            lpAddress: *mut c_void,
            dwSize: usize,
            flNewProtect: u32,
            lpflOldProtect: *mut u32,
        ) -> i32;
        fn VirtualFree(lpAddress: *mut c_void, dwSize: usize, dwFreeType: u32) -> i32;
    }

    const MEM_COMMIT: u32 = 0x1000;
    const MEM_RESERVE: u32 = 0x2000;
    const MEM_RELEASE: u32 = 0x8000;
    const PAGE_READWRITE: u32 = 0x04;
    const PAGE_READONLY: u32 = 0x02;
    const PAGE_NOACCESS: u32 = 0x01;

    // Provide a no-op logging symbol for the integration test binary.
    #[no_mangle]
    pub extern "C" fn log_put_message(
        _group: u8,
        _category: *const c_char,
        _priority: u8,
        _message: *const c_char,
    ) {
        // intentionally no-op
    }

    #[test]
    fn mprotect_child_helper_windows() {
        // Helper executed in a spawned child; perform allocation and
        // protection changes based on environment and then invoke the
        // secure memset API.
        if env::var("LIBNFC_MPROTECT_CHILD").ok().as_deref() != Some("1") {
            return;
        }

        let case = env::var("LIBNFC_MPROTECT_CASE").unwrap_or_else(|_| "readonly".into());
        // Use 64KB as an allocation grain to be safe across page sizes.
        let len: usize = 64 * 1024;
        let mem = unsafe {
            VirtualAlloc(
                ptr::null_mut(),
                len,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        };
        if mem.is_null() {
            eprintln!("VirtualAlloc failed: {}", std::io::Error::last_os_error());
            std::process::exit(10);
        }
        // Initialize memory to non-zero
        unsafe { ptr::write_bytes(mem as *mut u8, 0xFFu8, len) };

        let prot = match case.as_str() {
            "readonly" => PAGE_READONLY,
            "none" => PAGE_NOACCESS,
            other => {
                eprintln!("unknown case: {}", other);
                std::process::exit(12);
            }
        };

        let mut old: u32 = 0;
        let ok = unsafe { VirtualProtect(mem, len, prot, &mut old as *mut u32) };
        if ok == 0 {
            eprintln!("VirtualProtect failed: {}", std::io::Error::last_os_error());
            std::process::exit(11);
        }

        // Try to call secure memset on protected memory.
        let rc = unsafe { libnfc_rs::nfc_secure_memset(mem as *mut c_void, 0x5, len) };
        if rc != libnfc_rs::NFC_SECURE_SUCCESS {
            eprintln!("nfc_secure_memset returned error code: {}", rc);
            std::process::exit(0);
        }
        eprintln!("nfc_secure_memset unexpectedly returned success");
        std::process::exit(1);
    }

    #[test]
    fn mprotect_write_protection_detected_windows() {
        let exe = std::env::current_exe().expect("current_exe");
        let mut cmd = Command::new(exe);
        cmd.arg("mprotect_child_helper_windows");
        cmd.env("LIBNFC_MPROTECT_CHILD", "1");
        cmd.env("LIBNFC_MPROTECT_CASE", "readonly");
        let out = cmd.output().expect("failed to spawn child");
        let status = out.status;
        if status.success() {
            // child indicated handled error
            return;
        }
        if let Some(code) = status.code() {
            if code == 1 {
                panic!(
                    "child unexpectedly returned success. stdout='{}' stderr='{}'",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
            } else {
                // Non-zero exit code (likely an OS exception) is accepted; print outputs for debugging
                eprintln!(
                    "child exited with code {}. stdout='{}' stderr='{}'",
                    code,
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                return;
            }
        }
        // If we cannot inspect the exit code, include outputs in a panic
        panic!(
            "child exited with unknown status. stdout='{}' stderr='{}'",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn mprotect_none_protection_detected_windows() {
        let exe = std::env::current_exe().expect("current_exe");
        let mut cmd = Command::new(exe);
        cmd.arg("mprotect_child_helper_windows");
        cmd.env("LIBNFC_MPROTECT_CHILD", "1");
        cmd.env("LIBNFC_MPROTECT_CASE", "none");
        let out = cmd.output().expect("failed to spawn child");
        let status = out.status;
        if status.success() {
            return;
        }
        if let Some(code) = status.code() {
            if code == 1 {
                panic!(
                    "child unexpectedly returned success. stdout='{}' stderr='{}'",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
            } else {
                eprintln!(
                    "child exited with code {}. stdout='{}' stderr='{}'",
                    code,
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                return;
            }
        }
        panic!(
            "child exited with unknown status. stdout='{}' stderr='{}'",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
