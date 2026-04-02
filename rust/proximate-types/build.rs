use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

fn main() {
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
