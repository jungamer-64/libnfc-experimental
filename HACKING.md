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

            cargo test --manifest-path rust/Cargo.toml -p proximate-sys --no-default-features --features c_ffi
            bash scripts/check_callerfree_usage.sh
            cargo test --manifest-path rust/Cargo.toml -p proximate -- --nocapture
            cargo test --manifest-path rust/Cargo.toml -p proximate-sys --no-default-features --features "c_ffi,secure,lifecycle,orchestration" -- --nocapture
            cmake -S . -B build-rust-core -DBUILD_EXAMPLES=OFF -DBUILD_UTILS=OFF -DBUILD_TESTING=ON
            cmake --build build-rust-core -j"$(nproc)"
            ctest --test-dir build-rust-core --output-on-failure

     `PROXIMATE_SECURE`, `PROXIMATE_LIFECYCLE`, and
     `PROXIMATE_ORCHESTRATION` remain accepted only as deprecated no-op
     compatibility flags retained for older build scripts.

  3. Preserve cross-platform compatibility

     The source code should remain compilable across various platforms,
     including some you probably cannot test alone, so keep it in mind.
     Supported platforms:

     - Linux
     - FreeBSD
     - macOS
     - Windows with MinGW
