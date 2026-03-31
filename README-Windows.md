    *-
    * Free/Libre Near Field Communication (NFC) library
    *
    * Windows-specific notes for the CMake build
    -*

Requirements
============

- MinGW-w64 or MSVC
- CMake 3.16 or newer
- libusb-1.0 development files

Building
========

This repository is built with CMake only.

Example with MinGW Makefiles:

```bat
cmake -S . -B build -G "MinGW Makefiles" -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release
```

Example with Ninja:

```bat
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release
```

Useful options:

- `-DBUILD_SHARED_LIBS=OFF` for a static build
- `-DINSTALL_BUNDLE=ON` to assemble a redistributable bundle around `nfc-list`
- `-DLIBNFC_DRIVER_PCSC=ON` to enable the PC/SC driver

Installation
============

```bat
cmake --install build --config Release
```

The default Windows configuration directory is `./config` relative to the
installed binaries. If you want a different location, set
`-DLIBNFC_CONFDIR=...` when configuring the build.

For compatibility with older build scripts, `-DLIBNFC_SYSCONFDIR=...` is still
accepted and mapped to `LIBNFC_CONFDIR`.

References
==========

- MinGW-w64: <https://www.mingw-w64.org/>
- libusb-1.0: <https://libusb.info/>
- CMake: <https://cmake.org/>
