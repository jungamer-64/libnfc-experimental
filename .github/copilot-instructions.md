# Copilot Instructions for libnfc

- **Core Layout**: `libnfc/` holds the C entry points (`nfc.c`, `nfc-common.cpp`, `log.cpp`) and driver glue under `libnfc/drivers/`; shared headers live in `include/nfc/`, while CLI tools sit in `utils/` and sample apps in `examples/`.
- **Rust Bridge**: `rust/libnfc-rs/src/lib.rs` exposes connstring helpers and thread-local error buffers; the repo builds the Rust staticlib as part of the normal CMake/Autotools flow (`libnfc_rs_build` in CMake, `all-local` in `rust/Makefile.am`). Respect the contract described in `FFI_POLICY.md` whenever editing FFI.
- **FFI Safety Rules**: Exported Rust entry points must apply the Rust 2024 `#[unsafe(no_mangle)] extern "C"` ABI (or the equivalent for the active edition), wrap logic with `ffi_catch_unwind`, return NULL (or sentinel errno) on panic/error for pointer/handle APIs, and avoid handing back borrowed buffers. Regenerate the tracked ABI header snapshot with the project wrapper, for example:

	```sh
	python3 rust/libnfc-rs/tools/generate_cbindgen_header.py --output rust/libnfc-rs/include/libnfc_rs.h
	```

	CI includes a header-check script at `scripts/check-cbindgen.sh` which verifies the generated header matches the tracked `rust/libnfc-rs/include/libnfc_rs.h`. That header is a repository-tracked ABI snapshot, not an installed public header. Mirror any ABI notes into `FFI_POLICY.md`.
- **Thread-Local Errors**: `nfc_get_last_error`/`nfc_clear_last_error` surface Rust thread-local buffers; always call `nfc_set_last_error` on failure paths and clear it before returning success. When adding error codes, register them through the mapping rules in `FFI_POLICY.md` and avoid sharing buffers across threads.
- **Lifecycle Slice**: The current Phase 4 slice moves `nfc_context_alloc_defaults`, `nfc_device_new`, and `nfc_device_free` into Rust behind `nfc_lifecycle` / `USE_RUST_NFC_LIFECYCLE` / `--enable-rust-lifecycle`. Keep `nfc_context_new()` as the C wrapper that still owns env/config/log setup, and keep `nfc_context_free()` in C while it owns `log_exit()`.
- **Safe Memory Utilities**: Default to `NFC_SAFE_MEMCPY` / `NFC_SECURE_MEMSET` from `libnfc/nfc-secure.h` when editing in-tree code. This header is repository-internal and is not part of the installed orig-compatible public headers. Only fall back to raw `memcpy` in clearly justified hot paths, backed by a benchmark note explaining the regression risk.
- **Driver Discovery**: `nfc_list_devices` merges config files (`/etc/nfc/devices.d`) with runtime env vars (`LIBNFC_*`); keep that priority order intact. "Optional devices must stay silent" means: do not surface probe failures to callers (no ERROR-level logging, no addition to the device list) unless a probe succeeds; emit debug-level diagnostics only behind `LIBNFC_LOG`.

	Feature flag policy for drivers (examples):

	- CMake option: `LIBNFC_ENABLE_<DRIVERNAME>` (e.g. `LIBNFC_ENABLE_PN53X`)
	- Cargo feature: `driver-<drivername>` (e.g. `driver-pn53x`)

	When adding a driver, update both CMake `option()` and `Cargo.toml` `features` and add a CI job matrix entry to exercise the new flag combination.
- **Build Workflow**: Standard loop is `cmake -S . -B build -DCMAKE_BUILD_TYPE=RelWithDebInfo`, `cmake --build build`, and `cmake --build build --target libnfc_rs_build` when touching Rust. CMake writes Rust artifacts under `build/rust-target/`; Autotools writes them under `<builddir>/rust/target/`. Never commit generated outputs except the single tracked cbindgen header snapshot `rust/libnfc-rs/include/libnfc_rs.h`.
- **Testing**: Run `ctest --test-dir build --output-on-failure` for the C suite, `cargo test --manifest-path rust/libnfc-rs/Cargo.toml` for Rust, and run the test-only `ffi-sanity` check locally when touching FFI (for example `ctest --test-dir build -R 'ffi_sanity|public_compat_smoke' --output-on-failure`). Extend that check whenever new externs or drivers land; a dedicated CI job is still planned rather than current.
- **CI Expectations**: Current GitHub Actions baselines are `build-and-test`, `rust-sanity`, and `asan`. Planned additions such as `ci/ffi-sanity` / `ci/full` should keep generated artifacts in build directories, never in the source tree except for the tracked cbindgen header.

Document precedence when multiple documents conflict: follow this ordered priority:

1. `FFI_POLICY.md`
2. `SECURITY.md`
3. `Rust.md`
4. `README.md`
5. Other repository documentation

If a conflict requires deviation from higher-priority guidance, document it in the PR/RFC. If the repo later introduces an FFI Maintainer / CODEOWNERS flow, obtain that explicit approval there.

Short definitions (to remove ambiguous phrasing used elsewhere):

- documented hot paths: code paths explicitly justified in code comments or PR notes where higher-performance—but audited—memory primitives may be used; any deviation requires a benchmark note and PR justification.
- silent: optional drivers or probes must not emit ERROR-level logs or appear in `nfc_list_devices` results on probe failure; they may emit DEBUG logs behind `LIBNFC_LOG`.
- noise-free: CI jobs should avoid DEBUG-level output by default; debug logs are acceptable only when `LIBNFC_LOG` or a debug CI job is explicitly enabled.

Recommended minimum tool versions (baseline):

- CMake >= 3.20
- Rust stable toolchain (tracked via `rust-toolchain.toml` in the repo and kept aligned with CI)
- cbindgen >= 0.24

Escalation path: if implementers cannot follow any rule for a valid technical reason, open an issue and RFC and document the exception in the PR. If the repo later adopts dedicated FFI Maintainer / Security approval gates, route the exception through that flow.
- **Logging**: Prefer the structured `log_put`/`log_put_message` helpers (`libnfc/log.cpp`), guard every call against null devices, and register new categories via `LOG_CATEGORY` before emitting. Keep debug-only logs behind `LIBNFC_LOG` so CI log tests stay noise-free.
- **Docs to Consult**: `Rust.md` outlines the staged migration roadmap; scan it before large refactors to stay aligned with current phase. Security-sensitive changes should reference `SECURITY.md` and the memory guidelines above.
- **Absolute Prohibitions** *(break any and the migration backslides immediately)*:
	1. **Never let a panic cross the FFI boundary** — all exported Rust entry points must wrap work in `ffi_catch_unwind` (or equivalent) and normalize errors to NULL/sentinel codes. No `unwrap()` or panic-prone operations may run outside that guard.
	2. **Never free CallerFree pointers with raw `free()`** — when Rust hands ownership to C (via `CString::into_raw`, etc.), the paired `*_free` wrapper is the *only* release mechanism; bypassing it risks allocator mismatch and UAF.
	3. **Never change ABI without the regenerated header** — any FFI-visible change requires rerunning `cbindgen` and committing the updated `rust/libnfc-rs/include/libnfc_rs.h`, along with the checklist evidence from `FFI_POLICY.md` §7.
