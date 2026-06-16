proximate-sys: internal Rust implementation of libnfc's public C ABI

This crate exposes the Rust-backed libnfc entrypoints that are used by this
repository's C build. The supported C ABI is defined by the orig-compatible
installed headers under `include/nfc/`.

The temporary `nfc_safe_*` / `nfc_secure_*` helper ABI that existed only in
this experimental tree has been retired. In-tree examples and utilities now
use local C helpers where they need bounded copies or argument checks, while
branch-only PN53x entrypoints have been removed from the maintained surface.
