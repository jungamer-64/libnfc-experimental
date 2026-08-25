proximate-sys: internal Rust implementation of libnfc's public C ABI

This crate exposes the Rust-backed libnfc entrypoints that are used by this
repository's C build. The supported libnfc 1.8.0 C ABI is defined by the
installed headers under `include/nfc/`.

The C ABI is an outer boundary rather than the crate's internal object model.
`nfc_context` and `nfc_device` are opaque handles to Rust-owned allocations;
raw pointers, C enum carriers, legacy external-driver views, and C allocation
ownership are validated or projected in the boundary modules. Built-in and
registered C drivers both enter the same `proximate_driver::Device` operation
path after open. Ordinary operations are serialized per device, while the
command-abort capability remains independently callable during blocking I/O.
