use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn tracked_cbindgen_header_is_up_to_date() {
    assert_header_matches(
        "cbindgen.toml",
        "include/libnfc_rs.h",
        "proximate_sys.generated.h",
    );
}

#[test]
fn tracked_private_cbindgen_header_is_up_to_date() {
    assert_header_matches(
        "cbindgen.private.toml",
        "include/libnfc_rs_private.h",
        "proximate_sys.private.generated.h",
    );
}

fn assert_header_matches(config_rel: &str, tracked_rel: &str, generated_name: &str) {
    if Command::new("cbindgen").arg("--version").output().is_err() {
        eprintln!("cbindgen not found in PATH; skipping header diff test");
        return;
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config = manifest_dir.join(config_rel);
    let tracked = manifest_dir.join(tracked_rel);
    let generated = std::env::temp_dir().join(generated_name);

    let status = Command::new("python3")
        .arg("tools/generate_cbindgen_header.py")
        .arg("--config")
        .arg(&config)
        .arg("--output")
        .arg(&generated)
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to execute header generation wrapper");

    assert!(
        status.success(),
        "header generation wrapper failed for {} (status: {:?})",
        config.display(),
        status
    );

    compare_headers(&config, &tracked, &generated);
}

fn compare_headers(config: &Path, tracked: &Path, generated: &Path) {
    let tracked_contents = fs::read_to_string(tracked).expect("failed to read tracked header");
    let generated_contents =
        fs::read_to_string(generated).expect("failed to read generated header");

    if normalize(&tracked_contents) != normalize(&generated_contents) {
        if let Ok(out) = Command::new("diff")
            .arg("-u")
            .arg(tracked)
            .arg(generated)
            .output()
        {
            let diff = String::from_utf8_lossy(&out.stdout);
            eprintln!("Header mismatch detected (diff):\n{}", diff);
        } else {
            eprintln!("Tracked header:\n{}\n", tracked_contents);
            eprintln!("Generated header:\n{}\n", generated_contents);
        }

        panic!(
            "Tracked cbindgen header is out-of-date. Regenerate with:\n  python3 tools/generate_cbindgen_header.py --config {} --output {}",
            config.display(),
            tracked.display()
        );
    }
}

fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n")
        .lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}
