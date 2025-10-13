// tests/cbindgen_diff.rs

use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn tracked_cbindgen_header_is_up_to_date() {
    // Skip test if cbindgen is not available in PATH — CI should install it where needed.
    if Command::new("cbindgen").arg("--version").output().is_err() {
        eprintln!("cbindgen not found in PATH; skipping header diff test");
        return;
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config = manifest_dir.join("cbindgen.toml");
    let tracked = manifest_dir.join("include/libnfc_rs.h");
    let generated = std::env::temp_dir().join("libnfc_rs.generated.h");

    // Use the stable wrapper script to generate the header. The wrapper
    // centralizes cbindgen invocation, suppresses a known class of
    // benign warnings and optionally can auto-update the cbindgen
    // defines mapping when requested.
    let status = Command::new("python3")
        .arg("tools/generate_cbindgen_header.py")
        .arg("--output")
        .arg(&generated)
        .status()
        .expect("failed to execute header generation wrapper");

    if !status.success() {
        panic!("header generation wrapper failed (status: {:?})", status);
    }

    let a = fs::read_to_string(&tracked).expect("failed to read tracked header");
    let b = fs::read_to_string(&generated).expect("failed to read generated header");

    if normalize(&a) != normalize(&b) {
        // Try to show a unified diff if `diff` is available
        if let Ok(out) = Command::new("diff")
            .arg("-u")
            .arg(&tracked)
            .arg(&generated)
            .output()
        {
            let s = String::from_utf8_lossy(&out.stdout);
            eprintln!("Header mismatch detected (diff):\n{}", s);
        } else {
            eprintln!("Tracked header:\n{}\n", a);
            eprintln!("Generated header:\n{}\n", b);
        }
        panic!(
            "Tracked cbindgen header is out-of-date. Regenerate with:\n  cbindgen --config {} --crate libnfc-rs --output {}",
            config.display(), tracked.display()
        );
    }
}

fn normalize(s: &str) -> String {
    // Normalize line endings and trim trailing whitespace for robust comparison
    s.replace("\r\n", "\n")
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}
