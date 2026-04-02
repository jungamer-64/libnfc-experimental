proximate-sys: tracked Rust/C ABI surface for libnfc

This crate exposes the Rust-backed libnfc entrypoints that are used by this
repository's C build. The tracked headers under
`rust/proximate-sys/include/` are ABI snapshots for repository checks and are
not installed as part of the orig-compatible public header surface.

The temporary `nfc_safe_*` / `nfc_secure_*` helper ABI that existed only in
this experimental tree has been retired. In-tree examples and utilities now
use local C helpers where they need bounded copies or string-length checks.
