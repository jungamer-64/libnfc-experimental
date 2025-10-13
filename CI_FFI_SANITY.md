# CI plan: ffi-sanity

目的: Rust と C の FFI 境界を継続的に検証する軽量パイプライン。

ジョブ概要:

- step-1: Build Rust crate (release) with `--target-dir build/rust`.
- step-2: Run `cbindgen` to generate header to `build/rust/include/libnfc_rs.h`.
- step-3: Build C library linking against generated Rust staticlib.
- step-4: Run small set of integration tests (examples/ffi-sanity/) that exercise basic APIs (context create/free, basic read/write, log callback roundtrip).
- step-5: Collect artifacts: generated header, rust staticlib, test logs.

注意点:

- runner には Rust と C ビルドツールが必要（cargo, cbindgen, gcc/clang）。
- Nightly パイプラインでは ASan/TSan + cargo-fuzz を追加する。
