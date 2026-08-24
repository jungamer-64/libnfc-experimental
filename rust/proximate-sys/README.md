proximate-sys: internal Rust implementation of libnfc's public C ABI

This crate exposes the Rust-backed libnfc entrypoints that are used by this
repository's C build. The supported libnfc 1.8.0 C ABI is defined by the
installed headers under `include/nfc/`.
