    *-
    * Free/Libre Near Field Communication (NFC) library
    *
    * Libnfc historical contributors:
    * Copyright (C) 2009      Roel Verdult
    * Copyright (C) 2009-2013 Romuald Conty
    * Copyright (C) 2010-2012 Romain Tartière
    * Copyright (C) 2010-2013 Philippe Teuwen
    * Copyright (C) 2012-2013 Ludovic Rousseau
    * Additional contributors of Windows-specific parts:
    * Copyright (C) 2010      Glenn Ergeerts
    * Copyright (C) 2013      Alex Lian
    -*

Requirements
============

- MinGW-w64 compiler toolchain [1]
- CMake 3.16 or newer [2]
- Rust toolchain with `cargo` [3]
- A WinUSB-compatible driver for readers accessed directly over USB

Building
========

This repository is built with CMake only.

A Rust toolchain is required because the public C ABI is backed by the Rust
core in this branch.

To build the distribution the MinGW Makefiles generator of CMake can be used:

    C:\dev\libnfc-experimental> cmake -S . -B build -G "MinGW Makefiles" -DCMAKE_BUILD_TYPE=Release
    C:\dev\libnfc-experimental> cmake --build build --config Release

Useful options:

- `-DINSTALL_BUNDLE=ON` to assemble a redistributable bundle around `nfc-list`
- `-DLIBNFC_DRIVER_PCSC=ON` to enable the PC/SC driver
- `-DLIBNFC_CONFDIR=...` to override the installed configuration directory

Installation
============

    cmake --install build --config Release

The default Windows configuration directory is `./config` relative to the
installed binaries. If you want a different location, set
`-DLIBNFC_CONFDIR=...` when configuring the build.

USB-backed readers use the Rust `nusb` bridge in this branch. Install a
WinUSB-compatible driver for the reader you want to access directly over USB.

References
==========

[1] https://www.mingw-w64.org/

[2] https://cmake.org/

[3] https://www.rust-lang.org/tools/install
