cmake_minimum_required(VERSION 3.16)

if(NOT DEFINED CMAKE_BUILD_TYPE AND NOT DEFINED CMAKE_CONFIGURATION_TYPES)
  set(CMAKE_BUILD_TYPE Release CACHE STRING "Build type")
endif()

project(libnfc VERSION 1.8.0 LANGUAGES C CXX)

include(CTest)
include(GNUInstallDirs)
include(CMakePackageConfigHelpers)
find_package(Git QUIET)

set(CMAKE_C_STANDARD 99)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

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
option(USE_RUST_NFC_SECURE "Use Rust implementation of nfc-secure" OFF)
option(USE_RUST_NFC_LIFECYCLE "Use Rust implementation of nfc context/device lifecycle alloc/free helpers" OFF)
option(USE_RUST_NFC_CORE "Use Rust implementation of nfc driver registry/open/list/init helpers" OFF)
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

if(USE_RUST_NFC_CORE)
  set(USE_RUST_NFC_LIFECYCLE ON)
endif()

set(RUST_CBINDGEN_FEATURES "" CACHE STRING "Features to pass to cbindgen when generating the Rust FFI header (comma-separated)")
option(RUST_CBINDGEN_DEBUG_HEADER "Generate Rust FFI header including debug helpers (nfc_secure_debug)" OFF)
if(RUST_CBINDGEN_DEBUG_HEADER)
  set(RUST_CBINDGEN_FEATURES "nfc_secure_debug")
endif()

list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_LIST_DIR}/cmake/modules")

find_program(CARGO_EXECUTABLE cargo)
if(NOT CARGO_EXECUTABLE)
  message(FATAL_ERROR "cargo executable is required to build the Rust components")
endif()

find_program(CBINDGEN_EXECUTABLE cbindgen)
find_program(PYTHON3_EXECUTABLE python3)

include(LibnfcDrivers)

set(LIBNFC_RS_ENABLED_DRIVERS)
if(LIBNFC_DRIVER_PCSC)
  list(APPEND LIBNFC_RS_ENABLED_DRIVERS pcsc)
endif()
if(LIBNFC_DRIVER_ACR122_PCSC)
  list(APPEND LIBNFC_RS_ENABLED_DRIVERS acr122_pcsc)
endif()
if(LIBNFC_DRIVER_ACR122_USB)
  list(APPEND LIBNFC_RS_ENABLED_DRIVERS acr122_usb)
endif()
if(LIBNFC_DRIVER_ACR122S)
  list(APPEND LIBNFC_RS_ENABLED_DRIVERS acr122s)
endif()
if(LIBNFC_DRIVER_PN53X_USB)
  list(APPEND LIBNFC_RS_ENABLED_DRIVERS pn53x_usb)
endif()
if(LIBNFC_DRIVER_ARYGON)
  list(APPEND LIBNFC_RS_ENABLED_DRIVERS arygon)
endif()
if(LIBNFC_DRIVER_PN532_UART)
  list(APPEND LIBNFC_RS_ENABLED_DRIVERS pn532_uart)
endif()
if(LIBNFC_DRIVER_PN532_SPI)
  list(APPEND LIBNFC_RS_ENABLED_DRIVERS pn532_spi)
endif()
if(LIBNFC_DRIVER_PN532_I2C)
  list(APPEND LIBNFC_RS_ENABLED_DRIVERS pn532_i2c)
endif()
if(LIBNFC_DRIVER_PN71XX)
  list(APPEND LIBNFC_RS_ENABLED_DRIVERS pn71xx)
endif()
string(JOIN "," LIBNFC_RS_ENABLED_DRIVERS_ENV ${LIBNFC_RS_ENABLED_DRIVERS})

set(RUST_TARGET_DIR "${CMAKE_BINARY_DIR}/rust-target")
file(GLOB_RECURSE LIBNFC_RUST_SOURCES
  "${CMAKE_SOURCE_DIR}/rust/libnfc-rs/src/*.rs")
list(APPEND LIBNFC_RUST_SOURCES "${CMAKE_SOURCE_DIR}/rust/libnfc-rs/build.rs")

set(RUST_CRATE_NAME libnfc_rs)
set(RUST_LIB_FILENAME "${CMAKE_STATIC_LIBRARY_PREFIX}${RUST_CRATE_NAME}${CMAKE_STATIC_LIBRARY_SUFFIX}")
set(RUST_LIB_PATH "${RUST_TARGET_DIR}/release/${RUST_LIB_FILENAME}" CACHE INTERNAL "Path to Rust support library")

set(RUST_CARGO_FEATURES)
if(USE_RUST_NFC_SECURE)
  list(APPEND RUST_CARGO_FEATURES --features nfc_secure)
endif()
if(USE_RUST_NFC_LIFECYCLE)
  list(APPEND RUST_CARGO_FEATURES --features nfc_lifecycle)
endif()
if(USE_RUST_NFC_CORE)
  list(APPEND RUST_CARGO_FEATURES --features nfc_core)
endif()

if(USE_RUST_NFC_LIFECYCLE)
  add_compile_definitions(USE_RUST_NFC_LIFECYCLE)
endif()
if(USE_RUST_NFC_CORE)
  add_compile_definitions(USE_RUST_NFC_CORE)
endif()

add_custom_command(
  OUTPUT "${RUST_LIB_PATH}"
  COMMAND
    ${CMAKE_COMMAND} -E env
      LIBNFC_RS_WITH_ENVVARS=${LIBNFC_ENVVARS}
      LIBNFC_RS_WITH_CONFFILES=${LIBNFC_CONFFILES_MODE}
      LIBNFC_RS_WITH_LOG=${LIBNFC_LOG}
      LIBNFC_RS_WITH_DEBUG=${LIBNFC_DEBUG_MODE}
      LIBNFC_RS_EXTERNAL_BRIDGES=1
      LIBNFC_RS_CONFDIR=${LIBNFC_CONFDIR}
      LIBNFC_RS_ENABLED_DRIVERS=${LIBNFC_RS_ENABLED_DRIVERS_ENV}
      ${CARGO_EXECUTABLE} build --release
      --manifest-path "${CMAKE_SOURCE_DIR}/rust/libnfc-rs/Cargo.toml"
      --target-dir "${RUST_TARGET_DIR}"
      ${RUST_CARGO_FEATURES}
  DEPENDS "${CMAKE_SOURCE_DIR}/rust/libnfc-rs/Cargo.toml" ${LIBNFC_RUST_SOURCES}
  WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
  COMMENT "Building Rust support library"
  VERBATIM)

add_custom_target(libnfc_rs_build ALL DEPENDS "${RUST_LIB_PATH}")

if(CBINDGEN_EXECUTABLE)
  set(RUST_HEADER "${RUST_TARGET_DIR}/include/libnfc_rs.h")
  if(RUST_CBINDGEN_FEATURES)
    set(RUST_CBINDGEN_FEATURE_FLAGS --features "${RUST_CBINDGEN_FEATURES}")
  else()
    set(RUST_CBINDGEN_FEATURE_FLAGS)
  endif()

  if(PYTHON3_EXECUTABLE AND EXISTS "${CMAKE_SOURCE_DIR}/rust/libnfc-rs/tools/generate_cbindgen_header.py")
    add_custom_command(
      OUTPUT "${RUST_HEADER}"
      COMMAND
        ${PYTHON3_EXECUTABLE}
        "${CMAKE_SOURCE_DIR}/rust/libnfc-rs/tools/generate_cbindgen_header.py"
        ${RUST_CBINDGEN_FEATURE_FLAGS}
        --output "${RUST_HEADER}"
      DEPENDS "${CMAKE_SOURCE_DIR}/rust/libnfc-rs/Cargo.toml" ${LIBNFC_RUST_SOURCES}
      WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}/rust/libnfc-rs"
      COMMENT "Generating C header for Rust FFI (libnfc_rs.h) via wrapper"
      VERBATIM)
  else()
    add_custom_command(
      OUTPUT "${RUST_HEADER}"
      COMMAND
        ${CBINDGEN_EXECUTABLE}
        --config "${CMAKE_SOURCE_DIR}/rust/libnfc-rs/cbindgen.toml"
        --crate libnfc-rs
        ${RUST_CBINDGEN_FEATURE_FLAGS}
        --output "${RUST_HEADER}"
      DEPENDS "${CMAKE_SOURCE_DIR}/rust/libnfc-rs/Cargo.toml" ${LIBNFC_RUST_SOURCES}
      WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}/rust/libnfc-rs"
      COMMENT "Generating C header for Rust FFI (libnfc_rs.h)"
      VERBATIM)
  endif()

  add_custom_target(libnfc_rs_headers DEPENDS "${RUST_HEADER}")
  add_dependencies(libnfc_rs_build libnfc_rs_headers)
else()
  message(STATUS "cbindgen not found: rust FFI header will not be auto-generated")
endif()

if(WIN32)
  if(DEFINED LIBNFC_SYSCONFDIR AND NOT LIBNFC_SYSCONFDIR STREQUAL "")
    set(_LIBNFC_DEFAULT_CONFDIR "${LIBNFC_SYSCONFDIR}")
  else()
    set(_LIBNFC_DEFAULT_CONFDIR "./config")
  endif()
  include_directories("${CMAKE_CURRENT_SOURCE_DIR}/contrib/win32")
  if(NOT MINGW)
    add_compile_definitions(_CRT_SECURE_NO_WARNINGS _CRT_NONSTDC_NO_DEPRECATE)
  endif()
else()
  set(_XOPEN_SOURCE 600)
  if(DEFINED SYSCONFDIR AND NOT SYSCONFDIR STREQUAL "")
    set(_LIBNFC_DEFAULT_CONFDIR "${SYSCONFDIR}/nfc")
  else()
    set(_LIBNFC_DEFAULT_CONFDIR "${CMAKE_INSTALL_FULL_SYSCONFDIR}/nfc")
  endif()
endif()
set(LIBNFC_CONFDIR "${_LIBNFC_DEFAULT_CONFDIR}" CACHE STRING "libnfc configuration directory")
unset(_LIBNFC_DEFAULT_CONFDIR)
configure_file("${CMAKE_CURRENT_SOURCE_DIR}/cmake/config.h.in" "${CMAKE_CURRENT_BINARY_DIR}/config.h")

include_directories("${CMAKE_CURRENT_BINARY_DIR}")
include_directories("${CMAKE_CURRENT_SOURCE_DIR}" "${CMAKE_CURRENT_SOURCE_DIR}/include")
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

if(LIBNFC_DRIVER_PN53X_USB OR LIBNFC_DRIVER_ACR122_USB)
  list(APPEND LIBNFC_PKGCONFIG_REQUIRES_PRIVATE libusb-1.0)
endif()
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

if(LIBUSB_INCLUDE_DIRS)
  include_directories(${LIBUSB_INCLUDE_DIRS})
endif()
if(LIBUSB_LIBRARY_DIRS)
  link_directories(${LIBUSB_LIBRARY_DIRS})
endif()

if(PCSC_INCLUDE_DIRS)
  include_directories(${PCSC_INCLUDE_DIRS})
endif()
if(PCSC_LIBRARY_DIRS)
  link_directories(${PCSC_LIBRARY_DIRS})
endif()

if(LIBNFC_NCI_INCLUDE_DIRS)
  include_directories(${LIBNFC_NCI_INCLUDE_DIRS})
endif()
if(LIBNFC_NCI_LIBRARY_DIRS)
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
  if(LIBUSB_FOUND)
    list(APPEND LIBNFC_STATIC_PACKAGE_LIBRARIES ${LIBUSB_LIBRARIES})
  endif()
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
