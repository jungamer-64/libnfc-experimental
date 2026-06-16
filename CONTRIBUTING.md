# Contributing to libnfc

Thank you for your interest in contributing to libnfc.

## Build and test expectations

All development in this repository is expected to use CMake.

```bash
cmake -S . -B build -DBUILD_EXAMPLES=ON -DBUILD_UTILS=ON -DBUILD_TESTING=ON
cmake --build build -j"$(nproc)"
ctest --test-dir build --output-on-failure
```

When you touch packaging or exported targets, also verify a static build:

```bash
cmake -S . -B build-static -DBUILD_SHARED_LIBS=OFF -DBUILD_EXAMPLES=OFF -DBUILD_UTILS=OFF -DBUILD_TESTING=ON
cmake --build build-static -j"$(nproc)"
ctest --test-dir build-static --output-on-failure
```

## Code standards

### Memory safety

When you touch in-tree buffer handling, prefer the local bounded-copy helpers
already in use in `utils/nfc-utils.h` or an equivalent explicit size check
instead of introducing unchecked `memcpy` / `memset` calls.

### Error handling

Prefer the existing logging/error infrastructure in `libnfc/log.h` and the
current public libnfc error paths instead of introducing ad-hoc `perror()` or
integer-only error handling.

### Cross-platform behavior

Keep Linux, FreeBSD, macOS, and Windows in mind. If you cannot test a target
platform directly, avoid changes that hard-code Linux-only assumptions into
shared code paths.

## Local verification

Recommended local checks:

```bash
cargo test --manifest-path rust/Cargo.toml -p proximate-sys --no-default-features --features c_ffi
bash scripts/check_callerfree_usage.sh
cargo test --manifest-path rust/Cargo.toml -p proximate -- --nocapture
```

If you touch the Rust lifecycle/core bridge, also verify the Rust-backed core
slice:

```bash
cargo test --manifest-path rust/Cargo.toml -p proximate-sys --no-default-features --features "c_ffi,secure,lifecycle,orchestration" -- --nocapture
cmake -S . -B build-rust-core -DBUILD_EXAMPLES=OFF -DBUILD_UTILS=OFF -DBUILD_TESTING=ON
cmake --build build-rust-core -j"$(nproc)"
ctest --test-dir build-rust-core --output-on-failure
```

In this experimental branch, Rust is the only supported core implementation.
The `PROXIMATE_SECURE`, `PROXIMATE_LIFECYCLE`, and
`PROXIMATE_ORCHESTRATION` CMake options are deprecated no-ops retained for
older build scripts.

The Rust workspace lives under `rust/`, and `proximate-sys` remains the
internal implementation used by the public C ABI exposed through `include/nfc/`.

If you change exported CMake/package behavior, verify all of the following:

1. Shared build configure/build/install succeeds.
2. Static build configure/build/install succeeds.
3. An external CMake consumer can link `LibNFC::nfc`.
4. `pkg-config --cflags --libs libnfc` works against the install tree.

## Pull requests

Keep pull requests small and intentional.

- `feat`: new feature
- `fix`: bug fix
- `docs`: documentation only
- `test`: tests only
- `refactor`: internal restructuring without behavior change
- `perf`: performance work
- `chore`: build, CI, or maintenance work

If you change the public FFI boundary, update any affected headers under
`include/nfc/` and describe buffer ownership clearly in the PR description.
