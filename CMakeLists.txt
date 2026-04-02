cmake_minimum_required(VERSION 3.16)

if(NOT DEFINED CMAKE_BUILD_TYPE AND NOT DEFINED CMAKE_CONFIGURATION_TYPES)
  set(CMAKE_BUILD_TYPE Release CACHE STRING "Build type")
endif()

project(libnfc VERSION 1.8.0 LANGUAGES C)

include(CTest)
include(GNUInstallDirs)
include(CMakePackageConfigHelpers)
find_package(Git QUIET)

set(CMAKE_C_STANDARD 99)
set(CMAKE_C_STANDARD_REQUIRED ON)

set(PACKAGE_NAME "${PROJECT_NAME}")
set(PACKAGE_VERSION "${PROJECT_VERSION}")
set(PACKAGE_STRING "${PACKAGE_NAME} ${PACKAGE_VERSION}")
set(PACKAGE_DESCRIPTION "Near Field Communication (NFC) library")
set(VERSION "${PROJECT_VERSION}")
set(VERSION_MAJOR "${PROJECT_VERSION_MAJOR}")
set(VERSION_MINOR "${PROJECT_VERSION_MINOR}")
set(VERSION_PATCH "${PROJECT_VERSION_PATCH}")

if(GIT_FOUND AND EXISTS "${PROJECT_SOURCE_DIR}/.git")
  execute_process(
    COMMAND "${GIT_EXECUTABLE}" describe --always --dirty
    WORKING_DIRECTORY "${PROJECT_SOURCE_DIR}"
    OUTPUT_VARIABLE GIT_REVISION
    OUTPUT_STRIP_TRAILING_WHITESPACE
    ERROR_QUIET)
  if(GIT_REVISION STREQUAL "")
    unset(GIT_REVISION)
  endif()
endif()

option(BUILD_SHARED_LIBS "Build libnfc as a shared library" ON)
option(BUILD_EXAMPLES "Build examples" ON)
option(BUILD_UTILS "Build utilities" ON)
option(PROXIMATE_C_FFI "Build the Rust-backed C ABI layer" ON)
option(PROXIMATE_SECURE "Deprecated no-op compatibility flag retained for older build scripts" ON)
option(PROXIMATE_LIFECYCLE "Deprecated no-op: Rust lifecycle/core helpers are always enabled in this branch" ON)
option(PROXIMATE_ORCHESTRATION "Deprecated no-op: Rust orchestration entrypoints are always enabled in this branch" ON)
option(LIBNFC_LOG "Enable log facility (errors, warning, info and debug messages)" ON)
option(LIBNFC_ENVVARS "Enable envvars facility" ON)
option(LIBNFC_CONFFILES_MODE "Enable configuration files" ON)
option(INSTALL_BUNDLE "Assemble a standalone Windows bundle" OFF)
set(LIBNFC_DEBUG_MODE OFF CACHE BOOL "Debug mode")

if(MSVC)
  option(BUILD_MANUAL "Install man pages" OFF)
else()
  option(BUILD_MANUAL "Install man pages" ON)
endif()

if(NOT PROXIMATE_SECURE OR NOT PROXIMATE_LIFECYCLE OR NOT PROXIMATE_ORCHESTRATION)
  message(DEPRECATION
    "PROXIMATE_SECURE, PROXIMATE_LIFECYCLE, and PROXIMATE_ORCHESTRATION are ignored in this branch; "
    "the Rust core is always enabled.")
endif()

if(NOT PROXIMATE_C_FFI)
  message(FATAL_ERROR
    "PROXIMATE_C_FFI=OFF is no longer supported in this branch. "
    "Only the Rust-backed build is supported.")
endif()

set(RUST_CBINDGEN_FEATURES "" CACHE STRING "Features to pass to cbindgen when generating the Rust FFI header (comma-separated)")
option(RUST_CBINDGEN_DEBUG_HEADER "Generate Rust FFI header including debug helpers (nfc_secure_debug)" OFF)
if(RUST_CBINDGEN_DEBUG_HEADER)
  set(RUST_CBINDGEN_FEATURES "nfc_secure_debug")
endif()

list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_LIST_DIR}/cmake/modules")

if(WIN32)
  if(DEFINED LIBNFC_SYSCONFDIR AND NOT LIBNFC_SYSCONFDIR STREQUAL "")
    set(_LIBNFC_DEFAULT_CONFDIR "${LIBNFC_SYSCONFDIR}")
  else()
    set(_LIBNFC_DEFAULT_CONFDIR "./config")
  endif()
else()
  if(DEFINED SYSCONFDIR AND NOT SYSCONFDIR STREQUAL "")
    set(_LIBNFC_DEFAULT_CONFDIR "${SYSCONFDIR}/nfc")
  else()
    set(_LIBNFC_DEFAULT_CONFDIR "${CMAKE_INSTALL_FULL_SYSCONFDIR}/nfc")
  endif()
endif()
set(LIBNFC_CONFDIR "${_LIBNFC_DEFAULT_CONFDIR}" CACHE STRING "libnfc configuration directory")
unset(_LIBNFC_DEFAULT_CONFDIR)

include(LibnfcDrivers)

set(PROXIMATE_ENABLED_DRIVERS)
if(LIBNFC_DRIVER_PCSC)
  list(APPEND PROXIMATE_ENABLED_DRIVERS pcsc)
endif()
if(LIBNFC_DRIVER_ACR122_PCSC)
  list(APPEND PROXIMATE_ENABLED_DRIVERS acr122_pcsc)
endif()
if(LIBNFC_DRIVER_ACR122_USB)
  list(APPEND PROXIMATE_ENABLED_DRIVERS acr122_usb)
endif()
if(LIBNFC_DRIVER_ACR122S)
  list(APPEND PROXIMATE_ENABLED_DRIVERS acr122s)
endif()
if(LIBNFC_DRIVER_PN53X_USB)
  list(APPEND PROXIMATE_ENABLED_DRIVERS pn53x_usb)
endif()
if(LIBNFC_DRIVER_ARYGON)
  list(APPEND PROXIMATE_ENABLED_DRIVERS arygon)
endif()
if(LIBNFC_DRIVER_PN532_UART)
  list(APPEND PROXIMATE_ENABLED_DRIVERS pn532_uart)
endif()
if(LIBNFC_DRIVER_PN532_SPI)
  list(APPEND PROXIMATE_ENABLED_DRIVERS pn532_spi)
endif()
if(LIBNFC_DRIVER_PN532_I2C)
  list(APPEND PROXIMATE_ENABLED_DRIVERS pn532_i2c)
endif()
if(LIBNFC_DRIVER_PN71XX)
  list(APPEND PROXIMATE_ENABLED_DRIVERS pn71xx)
endif()
string(JOIN "," PROXIMATE_ENABLED_DRIVERS_ENV ${PROXIMATE_ENABLED_DRIVERS})

if(PROXIMATE_C_FFI)
  find_program(CARGO_EXECUTABLE cargo)
  if(NOT CARGO_EXECUTABLE)
    message(FATAL_ERROR "cargo executable is required to build the Rust core-backed C ABI layer")
  endif()

  find_program(CBINDGEN_EXECUTABLE cbindgen)
  find_program(PYTHON3_EXECUTABLE python3)

  set(RUST_TARGET_DIR "${CMAKE_BINARY_DIR}/rust-target")
  file(GLOB_RECURSE LIBNFC_RUST_SOURCES
    "${CMAKE_SOURCE_DIR}/rust/proximate/src/*.rs"
    "${CMAKE_SOURCE_DIR}/rust/proximate-sys/src/*.rs")
  list(APPEND LIBNFC_RUST_SOURCES
    "${CMAKE_SOURCE_DIR}/rust/Cargo.toml"
    "${CMAKE_SOURCE_DIR}/rust/proximate/Cargo.toml"
    "${CMAKE_SOURCE_DIR}/rust/proximate/build.rs"
    "${CMAKE_SOURCE_DIR}/rust/proximate-sys/Cargo.toml"
    "${CMAKE_SOURCE_DIR}/rust/proximate-sys/build.rs"
    "${CMAKE_SOURCE_DIR}/rust/proximate-sys/cbindgen.toml"
    "${CMAKE_SOURCE_DIR}/rust/proximate-sys/tools/generate_cbindgen_header.py"
    "${CMAKE_SOURCE_DIR}/rust/proximate-sys/tools/update_cbindgen_defines.py")

  set(RUST_CRATE_NAME proximate_sys)
  set(RUST_LIB_FILENAME "${CMAKE_STATIC_LIBRARY_PREFIX}${RUST_CRATE_NAME}${CMAKE_STATIC_LIBRARY_SUFFIX}")
  set(RUST_LIB_PATH "${RUST_TARGET_DIR}/release/${RUST_LIB_FILENAME}" CACHE INTERNAL "Path to Rust support library")

  set(RUST_CARGO_FEATURE_LIST c_ffi secure lifecycle orchestration)
  if(USB_REQUIRED)
    list(APPEND RUST_CARGO_FEATURE_LIST usb_helper)
  endif()
  if(LIBNFC_DRIVER_PCSC OR LIBNFC_DRIVER_ACR122_PCSC)
    list(APPEND RUST_CARGO_FEATURE_LIST pcsc_helper)
  endif()
  if(LIBNFC_DRIVER_PN71XX)
    list(APPEND RUST_CARGO_FEATURE_LIST nci_helper)
  endif()
  list(JOIN RUST_CARGO_FEATURE_LIST "," RUST_CARGO_FEATURES)
  set(PROXIMATE_RUST_ENV
    PROXIMATE_WITH_ENVVARS=${LIBNFC_ENVVARS}
    PROXIMATE_WITH_CONFFILES=${LIBNFC_CONFFILES_MODE}
    PROXIMATE_WITH_LOG=${LIBNFC_LOG}
    PROXIMATE_WITH_DEBUG=${LIBNFC_DEBUG_MODE}
    PROXIMATE_EXTERNAL_BRIDGES=1
    PROXIMATE_CONFDIR=${LIBNFC_CONFDIR}
    PROXIMATE_ENABLED_DRIVERS=${PROXIMATE_ENABLED_DRIVERS_ENV}
    PROXIMATE_PACKAGE_VERSION=${PROJECT_VERSION})
  if(GIT_REVISION)
    list(APPEND PROXIMATE_RUST_ENV PROXIMATE_GIT_REVISION=${GIT_REVISION})
  endif()

  add_custom_command(
    OUTPUT "${RUST_LIB_PATH}"
    COMMAND
      ${CMAKE_COMMAND} -E env
        ${PROXIMATE_RUST_ENV}
        ${CARGO_EXECUTABLE} build --release
        --manifest-path "${CMAKE_SOURCE_DIR}/rust/Cargo.toml"
        --package proximate-sys
        --target-dir "${RUST_TARGET_DIR}"
        --no-default-features
        --features "${RUST_CARGO_FEATURES}"
    DEPENDS ${LIBNFC_RUST_SOURCES}
    WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
    COMMENT "Building Rust core-backed C ABI layer"
    VERBATIM)

  add_custom_target(proximate_sys_build ALL DEPENDS "${RUST_LIB_PATH}")

  set(RUST_GENERATED_INCLUDE_DIR "${RUST_TARGET_DIR}/include")
  set(RUST_HEADER "${RUST_GENERATED_INCLUDE_DIR}/libnfc_rs.h")
  set(RUST_TRACKED_HEADER "${CMAKE_SOURCE_DIR}/rust/proximate-sys/include/libnfc_rs.h")
  if(RUST_CBINDGEN_FEATURES)
    set(RUST_CBINDGEN_FEATURE_FLAGS --features "${RUST_CBINDGEN_FEATURES}")
  else()
    set(RUST_CBINDGEN_FEATURE_FLAGS)
  endif()

  if(CBINDGEN_EXECUTABLE)
    if(PYTHON3_EXECUTABLE AND EXISTS "${CMAKE_SOURCE_DIR}/rust/proximate-sys/tools/generate_cbindgen_header.py")
      add_custom_command(
        OUTPUT "${RUST_HEADER}"
        COMMAND
          ${PYTHON3_EXECUTABLE}
          "${CMAKE_SOURCE_DIR}/rust/proximate-sys/tools/generate_cbindgen_header.py"
          ${RUST_CBINDGEN_FEATURE_FLAGS}
          --output "${RUST_HEADER}"
        DEPENDS ${LIBNFC_RUST_SOURCES}
        WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}/rust/proximate-sys"
        COMMENT "Generating tracked-compatible C header for the Rust core bridge (libnfc_rs.h)"
        VERBATIM)
    else()
      message(FATAL_ERROR
        "python3 is required alongside cbindgen to generate the Rust bridge headers")
    endif()
  elseif(EXISTS "${RUST_TRACKED_HEADER}")
    add_custom_command(
      OUTPUT "${RUST_HEADER}"
      COMMAND
        ${CMAKE_COMMAND} -E copy_if_different
        "${RUST_TRACKED_HEADER}"
        "${RUST_HEADER}"
      DEPENDS "${RUST_TRACKED_HEADER}"
      WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}/rust/proximate-sys"
      COMMENT "Copying tracked C header snapshot for the Rust core bridge (libnfc_rs.h)"
      VERBATIM)
  else()
    message(FATAL_ERROR
      "cbindgen is required (or rust/proximate-sys/include/libnfc_rs.h must exist) "
      "to provide the Rust bridge headers")
  endif()

  add_custom_target(proximate_sys_headers DEPENDS "${RUST_HEADER}")
  add_dependencies(proximate_sys_build proximate_sys_headers)
else()
  add_custom_target(proximate_sys_build)
endif()

if(WIN32)
  include_directories("${CMAKE_CURRENT_SOURCE_DIR}/contrib/win32")
  if(NOT MINGW)
    add_compile_definitions(_CRT_SECURE_NO_WARNINGS _CRT_NONSTDC_NO_DEPRECATE)
  endif()
else()
  set(_XOPEN_SOURCE 600)
endif()
configure_file("${CMAKE_CURRENT_SOURCE_DIR}/cmake/config.h.in" "${CMAKE_CURRENT_BINARY_DIR}/config.h")

include_directories("${CMAKE_CURRENT_BINARY_DIR}")
include_directories("${CMAKE_CURRENT_SOURCE_DIR}" "${CMAKE_CURRENT_SOURCE_DIR}/include")
if(PROXIMATE_C_FFI)
  include_directories("${RUST_GENERATED_INCLUDE_DIR}")
endif()
add_compile_definitions(HAVE_CONFIG_H)

if(LIBNFC_LOG)
  add_compile_definitions(LOG)
endif()

if(LIBNFC_ENVVARS)
  add_compile_definitions(ENVVARS)
endif()

if(LIBNFC_DEBUG_MODE)
  add_compile_definitions(DEBUG)
  set(CMAKE_C_FLAGS "-g3 ${CMAKE_C_FLAGS}")
  set(CMAKE_CXX_FLAGS "-g3 ${CMAKE_CXX_FLAGS}")
  set(WIN32_MODE "debug")
  set(CMAKE_RC_FLAGS "-D_DEBUG ${CMAKE_RC_FLAGS}")
else()
  set(WIN32_MODE "release")
endif()

if(LIBNFC_CONFFILES_MODE)
  add_compile_definitions(CONFFILES)
endif()

set(builddir "${CMAKE_BINARY_DIR}")
set(top_srcdir "${CMAKE_SOURCE_DIR}")
include(UseDoxygen)

if(CMAKE_C_COMPILER_ID STREQUAL "GNU")
  set(CMAKE_C_FLAGS "-Wall -pedantic ${CMAKE_C_FLAGS}")
  set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -fstack-protector-strong")
  set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -D_FORTIFY_SOURCE=2")
  set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -Wformat -Wformat-security")
  if(NOT WIN32)
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -fPIE")
    set(CMAKE_EXE_LINKER_FLAGS "${CMAKE_EXE_LINKER_FLAGS} -pie")
  endif()
endif()

if(CMAKE_CXX_COMPILER_ID STREQUAL "GNU")
  set(CMAKE_CXX_FLAGS "-Wall -Wextra -pedantic ${CMAKE_CXX_FLAGS}")
  set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -fstack-protector-strong")
  set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -D_FORTIFY_SOURCE=2")
  set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -Wformat -Wformat-security")
  if(NOT WIN32)
    set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -fPIE")
  endif()
endif()

if(MINGW)
  if(CMAKE_HOST_SYSTEM_NAME STREQUAL "Windows")
    if(CMAKE_SYSTEM_PROCESSOR STREQUAL "x86")
      message(STATUS "Building 32-bit Windows DLL")
      set(CMAKE_RC_FLAGS "--target=pe-i386 --output-format=coff ${CMAKE_RC_FLAGS}")
    elseif(CMAKE_SYSTEM_PROCESSOR STREQUAL "x86_64" OR CMAKE_SYSTEM_PROCESSOR STREQUAL "AMD64")
      message(STATUS "Building 64-bit Windows DLL")
      set(CMAKE_RC_FLAGS "--target=pe-x86-64 --output-format=coff ${CMAKE_RC_FLAGS}")
    else()
      message(FATAL_ERROR "Unknown Processor: ${CMAKE_SYSTEM_PROCESSOR}")
    endif()
  endif()

  find_program(DLLTOOL dlltool CMAKE_FIND_ROOT_PATH_BOTH)
  if(NOT DLLTOOL)
    message(FATAL_ERROR "Could not find dlltool command")
  endif()
endif()

set(LIBNFC_PKGCONFIG_REQUIRES_PRIVATE)
set(LIBNFC_PKGCONFIG_LIBS_PRIVATE)

if(LIBNFC_DRIVER_PCSC OR LIBNFC_DRIVER_ACR122_PCSC)
  list(APPEND LIBNFC_PKGCONFIG_REQUIRES_PRIVATE libpcsclite)
endif()
if(LIBNFC_DRIVER_PN71XX)
  list(APPEND LIBNFC_PKGCONFIG_REQUIRES_PRIVATE libnfc-nci)
endif()

if(CMAKE_SYSTEM_NAME MATCHES "Linux" AND I2C_REQUIRED)
  include(CheckFunctionExists)
  include(CheckLibraryExists)
  check_function_exists(clock_gettime HAVE_CLOCK_GETTIME)
  if(NOT HAVE_CLOCK_GETTIME)
    check_library_exists(rt clock_gettime "" HAVE_CLOCK_GETTIME_IN_RT)
    if(HAVE_CLOCK_GETTIME_IN_RT)
      set(LIBRT_FOUND TRUE)
      set(LIBRT_LIBRARIES rt)
      list(APPEND LIBNFC_PKGCONFIG_LIBS_PRIVATE -lrt)
    endif()
  endif()
endif()

if(PCSC_INCLUDE_DIRS)
  include_directories(${PCSC_INCLUDE_DIRS})
endif()
if(PCSC_LIBRARY_DIRS)
  link_directories(${PCSC_LIBRARY_DIRS})
endif()

if(LIBNFC_NCI_FOUND)
  include_directories(${LIBNFC_NCI_INCLUDE_DIRS})
endif()
if(LIBNFC_NCI_FOUND AND LIBNFC_NCI_LIBRARY_DIRS)
  link_directories(${LIBNFC_NCI_LIBRARY_DIRS})
endif()

if(NOT WIN32)
  if(LIBNFC_PKGCONFIG_REQUIRES_PRIVATE)
    list(SORT LIBNFC_PKGCONFIG_REQUIRES_PRIVATE)
    list(REMOVE_DUPLICATES LIBNFC_PKGCONFIG_REQUIRES_PRIVATE)
    string(REPLACE ";" "," PKG_CONFIG_REQUIRES_PRIVATE "${LIBNFC_PKGCONFIG_REQUIRES_PRIVATE}")
  else()
    set(PKG_CONFIG_REQUIRES_PRIVATE "")
  endif()

  if(LIBNFC_PKGCONFIG_LIBS_PRIVATE)
    list(REMOVE_DUPLICATES LIBNFC_PKGCONFIG_LIBS_PRIVATE)
    string(REPLACE ";" " " PKG_CONFIG_LIBS_PRIVATE "${LIBNFC_PKGCONFIG_LIBS_PRIVATE}")
  else()
    set(PKG_CONFIG_LIBS_PRIVATE "")
  endif()

  if(NOT BUILD_SHARED_LIBS AND PROXIMATE_C_FFI)
    if(PKG_CONFIG_LIBS_PRIVATE)
      set(PKG_CONFIG_LIBS_PRIVATE "-lproximate_sys -lnfc ${PKG_CONFIG_LIBS_PRIVATE}")
    else()
      set(PKG_CONFIG_LIBS_PRIVATE "-lproximate_sys -lnfc")
    endif()
  endif()

  set(prefix "${CMAKE_INSTALL_PREFIX}")
  set(exec_prefix "\${prefix}")
  set(libdir "\${exec_prefix}/${CMAKE_INSTALL_LIBDIR}")
  set(includedir "\${prefix}/${CMAKE_INSTALL_INCLUDEDIR}")

  configure_file("${CMAKE_CURRENT_SOURCE_DIR}/libnfc.pc.in" "${CMAKE_CURRENT_BINARY_DIR}/libnfc.pc" @ONLY)
  install(FILES "${CMAKE_CURRENT_BINARY_DIR}/libnfc.pc" DESTINATION "${CMAKE_INSTALL_LIBDIR}/pkgconfig")
endif()

if(WIN32)
  string(TIMESTAMP CURRENT_YEAR %Y)
endif()

set(LIBNFC_CMAKE_INSTALL_DIR "${CMAKE_INSTALL_LIBDIR}/cmake/LibNFC")
set(LIBNFC_STATIC_PACKAGE_LIBRARIES)
if(NOT BUILD_SHARED_LIBS)
  if(PCSC_FOUND)
    list(APPEND LIBNFC_STATIC_PACKAGE_LIBRARIES ${PCSC_LIBRARIES})
  endif()
  if(LIBNFC_NCI_FOUND)
    list(APPEND LIBNFC_STATIC_PACKAGE_LIBRARIES ${LIBNFC_NCI_LIBRARIES})
  endif()
  if(LIBRT_FOUND)
    list(APPEND LIBNFC_STATIC_PACKAGE_LIBRARIES ${LIBRT_LIBRARIES})
  endif()
  if(WIN32)
    list(APPEND LIBNFC_STATIC_PACKAGE_LIBRARIES wsock32)
  endif()
  list(REMOVE_DUPLICATES LIBNFC_STATIC_PACKAGE_LIBRARIES)
endif()

add_subdirectory(libnfc)
add_library(LibNFC::nfc ALIAS nfc)
add_subdirectory(include)

if(BUILD_UTILS)
  add_subdirectory(utils)
endif()

if(BUILD_EXAMPLES)
  add_subdirectory(examples)
endif()

if(BUILD_TESTING)
  add_subdirectory(test)
endif()

configure_package_config_file(
  "${CMAKE_CURRENT_SOURCE_DIR}/cmake/LibNFCConfig.cmake.in"
  "${CMAKE_CURRENT_BINARY_DIR}/LibNFCConfig.cmake"
  INSTALL_DESTINATION "${LIBNFC_CMAKE_INSTALL_DIR}")

write_basic_package_version_file(
  "${CMAKE_CURRENT_BINARY_DIR}/LibNFCConfigVersion.cmake"
  VERSION "${PROJECT_VERSION}"
  COMPATIBILITY SameMajorVersion)

install(FILES
  "${CMAKE_CURRENT_BINARY_DIR}/LibNFCConfig.cmake"
  "${CMAKE_CURRENT_BINARY_DIR}/LibNFCConfigVersion.cmake"
  DESTINATION "${LIBNFC_CMAKE_INSTALL_DIR}")

install(EXPORT LibNFCTargets
  FILE LibNFCTargets.cmake
  NAMESPACE LibNFC::
  DESTINATION "${LIBNFC_CMAKE_INSTALL_DIR}")

if(WIN32 AND INSTALL_BUNDLE AND BUILD_UTILS)
  include(InstallRequiredSystemLibraries)
  configure_file("${CMAKE_SOURCE_DIR}/cmake/FixBundle.cmake.in" "${CMAKE_CURRENT_BINARY_DIR}/FixBundle.cmake" @ONLY)
  install(SCRIPT "${CMAKE_CURRENT_BINARY_DIR}/FixBundle.cmake")
endif()

set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "${PACKAGE_DESCRIPTION}")
set(CPACK_PACKAGE_VENDOR "Roel Verdult")
set(CPACK_PACKAGE_CONTACT "Roel Verdult <roel@libnfc.org>")

if(WIN32)
  set(CPACK_PACKAGE_DESCRIPTION_FILE "${CMAKE_CURRENT_SOURCE_DIR}/README-Windows.md")
  set(CPACK_GENERATOR "ZIP")
  set(CPACK_SOURCE_GENERATOR "ZIP")
else()
  set(CPACK_PACKAGE_DESCRIPTION_FILE "${CMAKE_CURRENT_SOURCE_DIR}/README.md")
  set(CPACK_GENERATOR "TGZ")
  set(CPACK_SOURCE_GENERATOR "TGZ")
endif()

set(CPACK_RESOURCE_FILE_LICENSE "${CMAKE_CURRENT_SOURCE_DIR}/COPYING")
set(CPACK_PACKAGE_INSTALL_DIRECTORY "libnfc")
set(CPACK_PACKAGE_VERSION_MAJOR "${PROJECT_VERSION_MAJOR}")
set(CPACK_PACKAGE_VERSION_MINOR "${PROJECT_VERSION_MINOR}")
set(CPACK_PACKAGE_VERSION_PATCH "${PROJECT_VERSION_PATCH}")
set(CPACK_COMPONENT_LIBRARIES_DISPLAY_NAME "NFC Library")
set(CPACK_COMPONENT_EXAMPLES_DISPLAY_NAME "Example Applications")
set(CPACK_COMPONENT_HEADERS_DISPLAY_NAME "Development Headers")
if(NOT WIN32)
  set(CPACK_COMPONENT_MANUALS_DISPLAY_NAME "Example Applications Manuals")
  set(CPACK_COMPONENT_MANUALS_DISABLED TRUE)
  set(CPACK_COMPONENT_MANUALS_DEPENDS examples)
endif()
set(CPACK_COMPONENT_HEADERS_DISABLED TRUE)
set(CPACK_COMPONENT_HEADERS_DEPENDS libraries)
set(CPACK_COMPONENT_EXAMPLES_DEPENDS libraries)
set(CPACK_SOURCE_PACKAGE_FILE_NAME "libnfc-${PROJECT_VERSION}")
set(CPACK_SOURCE_IGNORE_FILES "~$" "/\\\\.git/" "bin/")
include(CPack)
