fn main() {
    println!("cargo::rustc-check-cfg=cfg(cbindgen)");

    println!("cargo::rerun-if-env-changed=PROXIMATE_CONFDIR");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "linux" && std::env::var_os("CARGO_FEATURE_DRIVER_PN71XX").is_some() {
        println!("cargo::rustc-link-lib=nfc_nci");
    }
}
