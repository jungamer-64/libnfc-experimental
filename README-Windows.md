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

- Visual Studio with the x64 C++ build tools, or a MinGW-w64 x64 toolchain [1]
- CMake 3.16 or newer [2]
- Rust with the matching `x86_64-pc-windows-msvc` or
  `x86_64-pc-windows-gnu` target [3]
- A WinUSB binding only for readers accessed through a direct-USB driver

Building
========

The supported product build is driven by CMake. MSVC x64 is the primary
Windows toolchain:

    C:\dev\libnfc-experimental> cmake -S . -B build-msvc -A x64 -DBUILD_TESTING=ON
    C:\dev\libnfc-experimental> cmake --build build-msvc --config Release --parallel
    C:\dev\libnfc-experimental> ctest --test-dir build-msvc -C Release --output-on-failure

For MinGW-w64 x64, use a shell in which the MinGW compiler and the Rust GNU
target are both available:

    rustup target add x86_64-pc-windows-gnu
    cmake -S . -B build-mingw -G Ninja -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=ON
    cmake --build build-mingw --parallel
    ctest --test-dir build-mingw --output-on-failure

CMake passes an explicit Rust target matching the selected C ABI. A Windows
product build does not use the host Rust target implicitly.

Useful options:

- `-DINSTALL_BUNDLE=ON` to assemble a redistributable bundle around `nfc-list`
- `-DLIBNFC_DRIVER_PCSC=OFF` to disable the generic PC/SC driver
- `-DLIBNFC_DRIVER_ACR122_PCSC=OFF` to disable the ACR122 PC/SC driver
- `-DLIBNFC_CONFDIR=...` to override the installed configuration directory

Both PC/SC drivers are enabled by default in Windows CMake builds. They link
the Windows SDK `winscard` import library; no separately installed PC/SC SDK
headers are required by the CMake discovery step. Cargo-only builds retain
explicit PC/SC features rather than target-specific default features.

Installation
============

    cmake --install build-msvc --config Release

The default Windows configuration directory is `./config` relative to the
installed binaries. If you want a different location, set
`-DLIBNFC_CONFDIR=...` when configuring the build.

Reader bindings and discovery
=============================

PC/SC and direct USB are separate ownership choices. Use the Windows standard
CCID binding for a reader accessed through PC/SC. Install a WinUSB binding only
when using `pn53x_usb` or `acr122_usb`; rebinding a CCID reader to WinUSB makes
it unavailable to the Windows PC/SC stack.

Serial discovery enumerates present `GUID_DEVINTERFACE_COMPORT` devices through
SetupAPI and reads each device's `PortName`. Valid names are deduplicated,
sorted numerically, and opened as `\\.\COMn`. An explicit serial connection
string can still open a valid port that is not returned by enumeration.

Direct-USB scan results use a stable Windows device instance selector:

    pn53x_usb:instance:<fixed-width UTF-16 hexadecimal units>
    acr122_usb:instance:<fixed-width UTF-16 hexadecimal units>

The payload contains four hexadecimal digits per UTF-16 code unit. ASCII case
in instance IDs is normalized, so a scan result can be passed back to open the
same device even when transient USB addresses change. Existing numeric
`<driver>:BBB:DDD` selectors and the `usb` selector remain accepted inputs.
A numeric selector must identify exactly one matching Windows device; otherwise
open reports an ambiguous selection instead of choosing arbitrarily.

If PC/SC has no attached readers, scanning completes with an empty PC/SC
result. If the PC/SC service is stopped or unavailable, the C API logs that
driver condition and continues scanning serial and direct-USB drivers.

References
==========

[1] https://www.mingw-w64.org/

[2] https://cmake.org/

[3] https://www.rust-lang.org/tools/install
