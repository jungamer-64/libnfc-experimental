Hello hackers!

General remarks about contributing
----------------------------------

Contributions to the libnfc are welcome!
Here are some directions to get you started:

  1. Follow style conventions
     The source code of the library tends to follow some conventions so that it
     is consistent in style and thus easier to read.
     Look around and respect the same style.
     Don't use tabs. Increment unit is two spaces.
     Don't leave trailing spaces or tabs at EOL.

  2. Chase warnings: no warning should be introduced by your changes
     Depending on what you touch, you can check with:

    2.1 When configuring and building with CMake

            cmake -S . -B build -DBUILD_EXAMPLES=ON -DBUILD_UTILS=ON -DBUILD_TESTING=ON
            cmake --build build -j"$(nproc)"
            ctest --test-dir build --output-on-failure

    2.2 When validating a static build

            cmake -S . -B build-static -DBUILD_SHARED_LIBS=OFF -DBUILD_EXAMPLES=OFF -DBUILD_UTILS=OFF -DBUILD_TESTING=ON
            cmake --build build-static -j"$(nproc)"
            ctest --test-dir build-static --output-on-failure

    2.3 When touching the Rust bridge

            cargo fmt --manifest-path rust/Cargo.toml --all -- --check
            cargo check --manifest-path rust/Cargo.toml --workspace
            cargo clippy --manifest-path rust/Cargo.toml --workspace -- -D warnings
            cargo doc --manifest-path rust/Cargo.toml --workspace --no-deps
            cargo test --manifest-path rust/Cargo.toml --workspace --lib
            cargo check --manifest-path rust/Cargo.toml --workspace --all-targets
            cargo check --manifest-path rust/Cargo.toml --workspace --no-default-features
            bash scripts/check_callerfree_usage.sh
            cmake -S . -B build-rust-core -DBUILD_EXAMPLES=OFF -DBUILD_UTILS=OFF -DBUILD_TESTING=ON
            cmake --build build-rust-core -j"$(nproc)"
            ctest --test-dir build-rust-core --output-on-failure

     Run `cargo check --manifest-path rust/Cargo.toml --workspace --all-features`
     on Linux, where all platform-constrained drivers are supported.

     Windows x64 changes must also keep the product boundaries green. MSVC is
     the primary product toolchain; both generic and ACR122 PC/SC drivers are
     enabled by default for these CMake builds:

            cmake -S . -B build-windows-shared -A x64 -DBUILD_EXAMPLES=ON -DBUILD_UTILS=ON -DBUILD_TESTING=ON
            cmake --build build-windows-shared --parallel --config Release
            ctest --test-dir build-windows-shared --output-on-failure -C Release
            cmake --install build-windows-shared --prefix install-windows-shared --config Release
            cpack --config build-windows-shared/CPackConfig.cmake -C Release -G ZIP

            cmake -S . -B build-windows-static -A x64 -DBUILD_SHARED_LIBS=OFF -DBUILD_EXAMPLES=OFF -DBUILD_UTILS=OFF -DBUILD_TESTING=ON
            cmake --build build-windows-static --parallel --config Release
            ctest --test-dir build-windows-static --output-on-failure -C Release
            cmake --install build-windows-static --prefix install-windows-static --config Release

     Configure and link a separate CMake consumer against each installed
     package. The static consumer is the authority for transitive `winscard`
     propagation. For the GNU ABI, also run:

            rustup target add x86_64-pc-windows-gnu
            cargo check --manifest-path rust/Cargo.toml -p proximate-native --target x86_64-pc-windows-gnu --features driver-pcsc,driver-acr122-pcsc
            cmake -S . -B build-windows-mingw -G Ninja -DCMAKE_BUILD_TYPE=Release -DBUILD_EXAMPLES=ON -DBUILD_UTILS=ON -DBUILD_TESTING=ON
            cmake --build build-windows-mingw --parallel
            ctest --test-dir build-windows-mingw --output-on-failure

     Hardware communication through PC/SC, WinUSB, or UART is a separate
     validation boundary from enumeration, compilation, C ABI linkage, tests,
     installation, and packaging.

  3. Preserve cross-platform compatibility

     The source code should remain compilable across various platforms,
     including some you probably cannot test alone, so keep it in mind.
     Supported platforms:

     - Linux
     - FreeBSD
     - macOS
     - Windows x64 with MSVC or MinGW-w64
