Hello hackers!

General remarks about contributing
----------------------------------

Contributions to libnfc are welcome.

1. Follow the existing code style.
   - Use spaces, not tabs.
   - Keep indentation to two spaces.
   - Avoid trailing whitespace.
2. Do not introduce new warnings.
3. Keep Linux, FreeBSD, macOS, and Windows compatibility in mind.

Build and test workflow
-----------------------

The supported build system is CMake.

```sh
cmake -S . -B build -DBUILD_EXAMPLES=ON -DBUILD_UTILS=ON -DBUILD_TESTING=ON
cmake --build build -j"$(nproc)"
ctest --test-dir build --output-on-failure
```

Useful verification passes:

```sh
cmake -S . -B build-static -DBUILD_SHARED_LIBS=OFF -DBUILD_EXAMPLES=OFF -DBUILD_UTILS=OFF -DBUILD_TESTING=ON
cmake --build build-static -j"$(nproc)"
ctest --test-dir build-static --output-on-failure
```

If you touch the Rust bridge, also run:

```sh
bash scripts/check-cbindgen.sh
bash scripts/check_callerfree_usage.sh
cargo test --manifest-path rust/libnfc-rs/Cargo.toml --features nfc_secure -- --nocapture
cargo test --manifest-path rust/libnfc-rs/Cargo.toml --features "nfc_core nfc_lifecycle" -- --nocapture
cmake -S . -B build-rust-core -DBUILD_EXAMPLES=OFF -DBUILD_UTILS=OFF -DBUILD_TESTING=ON -DUSE_RUST_NFC_SECURE=ON -DUSE_RUST_NFC_LIFECYCLE=ON -DUSE_RUST_NFC_CORE=ON
cmake --build build-rust-core -j"$(nproc)"
ctest --test-dir build-rust-core --output-on-failure
```
