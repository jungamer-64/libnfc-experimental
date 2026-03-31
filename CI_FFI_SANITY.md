# Planned CI job: ffi-sanity

目的: Rust と C の FFI 境界を継続的に検証する軽量パイプライン。

現状メモ: `examples/ffi-sanity/` は standalone の確認用サンプルとして存在するが、2026-03-31 時点では GitHub Actions に常設 job としてはまだ接続されていない。

ジョブ概要:

- step-1: Build the Rust crate in release mode using the repo's existing build layout.
  - CMake path: `build/rust-target/`
  - Autotools path: `<builddir>/rust/target/`
- step-2: Run `cbindgen` (via the wrapper script when available) and write the generated header into the build include dir rather than the tracked source-tree header.
- step-3: Build the C library and the standalone `examples/ffi-sanity/` binary against the generated Rust staticlib.
- step-4: Run the small integration test set in `examples/ffi-sanity/` to exercise basic APIs (context create/free, basic read/write, log callback roundtrip).
- step-5: Collect artifacts: generated header, Rust staticlib, `ffi-sanity` logs.

注意点:

- runner には Rust と C ビルドツールが必要（cargo, cbindgen, gcc/clang）。
- Nightly hardening として ASan / TSan / `cargo-fuzz` を追加する余地があるが、現状の既成事実は Nightly ASan job まで。
