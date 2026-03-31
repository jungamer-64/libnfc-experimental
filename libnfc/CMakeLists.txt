# Windows MinGW workarounds
if(WIN32)
  set(WINDOWS_SOURCES ../contrib/win32/stdlib.c)
  include_directories("${CMAKE_CURRENT_SOURCE_DIR}/../contrib/win32")

  set(RC_COMMENT "${PACKAGE_NAME} library")
  set(RC_INTERNAL_NAME "libnfc")
  set(RC_ORIGINAL_NAME "libnfc.dll")
  set(RC_FILE_TYPE VFT_DLL)
  set(RC_FILE_FLAGS 0x0)
  if(LIBNFC_DEBUG_MODE)
    set(RC_FILE_FLAGS VS_FF_DEBUG)
  endif()
  configure_file("${CMAKE_CURRENT_SOURCE_DIR}/../contrib/win32/version.rc.in" "${CMAKE_CURRENT_BINARY_DIR}/libnfc.rc" @ONLY)
  list(APPEND WINDOWS_SOURCES "${CMAKE_CURRENT_BINARY_DIR}/libnfc.rc")

  if(BUILD_SHARED_LIBS)
    configure_file("${CMAKE_CURRENT_SOURCE_DIR}/../contrib/win32/libnfc.def.in" "${CMAKE_CURRENT_BINARY_DIR}/libnfc.def" @ONLY)
    list(APPEND WINDOWS_SOURCES "${CMAKE_CURRENT_BINARY_DIR}/libnfc.def")
  endif()
endif()

set(CHIPS_SOURCES chips/pn53x.c)
include_directories("${CMAKE_CURRENT_SOURCE_DIR}/chips")

if(USB_REQUIRED)
  list(APPEND BUSES_SOURCES buses/usbbus.c)
endif()

if(UART_REQUIRED)
  if(WIN32)
    list(APPEND BUSES_SOURCES ../contrib/win32/libnfc/buses/uart.c)
  else()
    list(APPEND BUSES_SOURCES buses/uart.c)
  endif()
endif()

if(I2C_REQUIRED)
  if(CMAKE_SYSTEM_NAME MATCHES "Linux")
    list(APPEND BUSES_SOURCES buses/i2c.c)
  else()
    message(FATAL_ERROR "I2C is only (yet) supported in Linux!")
  endif()
endif()

if(SPI_REQUIRED)
  if(CMAKE_SYSTEM_NAME MATCHES "Linux")
    list(APPEND BUSES_SOURCES buses/spi.c)
  else()
    message(FATAL_ERROR "SPI is only (yet) supported in Linux!")
  endif()
endif()

include_directories("${CMAKE_CURRENT_SOURCE_DIR}/buses")
include_directories("${CMAKE_CURRENT_SOURCE_DIR}")

set(LIBRARY_SOURCES
  nfc.c
  nfc-device.c
  nfc-emulation.c
  nfc-internal.c
  nfc-secure.c
  nfc-common.cpp
  conf.c
  iso14443-subr.c
  mirror-subr.c
  target-subr.c
  target-subr-helpers.c
  target-subr-helpers2.c
  ${DRIVERS_SOURCES}
  ${BUSES_SOURCES}
  ${CHIPS_SOURCES}
  ${WINDOWS_SOURCES})

set(LIBNFC_EXPORTS_FILE "${CMAKE_SOURCE_DIR}/libnfc/libnfc.exports")
set(LIBNFC_VERSION_SCRIPT "${CMAKE_CURRENT_BINARY_DIR}/libnfc.map")
if(EXISTS "${LIBNFC_EXPORTS_FILE}")
  file(STRINGS "${LIBNFC_EXPORTS_FILE}" LIBNFC_EXPORT_SYMBOLS)
endif()

if(USE_RUST_NFC_SECURE)
  list(REMOVE_ITEM LIBRARY_SOURCES nfc-secure.c)
endif()
if(USE_RUST_NFC_LIFECYCLE)
  list(REMOVE_ITEM LIBRARY_SOURCES nfc-device.c)
endif()

if(LIBNFC_LOG)
  if(WIN32)
    list(APPEND LIBRARY_SOURCES log.cpp ../contrib/win32/libnfc/log-internal.c)
  else()
    list(APPEND LIBRARY_SOURCES log.cpp log-internal.c)
  endif()
endif()

add_library(nfc ${LIBRARY_SOURCES})
add_dependencies(nfc libnfc_rs_build)

target_include_directories(nfc
  PUBLIC
    $<BUILD_INTERFACE:${CMAKE_SOURCE_DIR}/include>
    $<INSTALL_INTERFACE:${CMAKE_INSTALL_INCLUDEDIR}>
  PRIVATE
    "${CMAKE_SOURCE_DIR}"
    "${CMAKE_BINARY_DIR}"
    "${CMAKE_CURRENT_SOURCE_DIR}"
    "${CMAKE_CURRENT_SOURCE_DIR}/chips"
    "${CMAKE_CURRENT_SOURCE_DIR}/buses"
    ${LIBUSB_INCLUDE_DIRS}
    ${PCSC_INCLUDE_DIRS})

set(LIBNFC_SHARED_LINK_LIBRARIES "${RUST_LIB_PATH}")
set(LIBNFC_BUILD_INTERFACE_STATIC_LIBRARIES "${RUST_LIB_PATH}")

if(LIBUSB_FOUND)
  list(APPEND LIBNFC_SHARED_LINK_LIBRARIES ${LIBUSB_LIBRARIES})
  list(APPEND LIBNFC_BUILD_INTERFACE_STATIC_LIBRARIES ${LIBUSB_LIBRARIES})
endif()

if(PCSC_FOUND)
  list(APPEND LIBNFC_SHARED_LINK_LIBRARIES ${PCSC_LIBRARIES})
  list(APPEND LIBNFC_BUILD_INTERFACE_STATIC_LIBRARIES ${PCSC_LIBRARIES})
endif()

if(LIBRT_FOUND)
  list(APPEND LIBNFC_SHARED_LINK_LIBRARIES ${LIBRT_LIBRARIES})
  list(APPEND LIBNFC_BUILD_INTERFACE_STATIC_LIBRARIES ${LIBRT_LIBRARIES})
endif()

if(WIN32)
  list(APPEND LIBNFC_SHARED_LINK_LIBRARIES wsock32)
  list(APPEND LIBNFC_BUILD_INTERFACE_STATIC_LIBRARIES wsock32)
endif()

if(BUILD_SHARED_LIBS)
  target_link_libraries(nfc PRIVATE ${LIBNFC_SHARED_LINK_LIBRARIES})
else()
  target_link_libraries(nfc INTERFACE $<BUILD_INTERFACE:${LIBNFC_BUILD_INTERFACE_STATIC_LIBRARIES}>)
endif()

if(BUILD_SHARED_LIBS AND LIBUSB_LIBRARY_DIRS)
  target_link_directories(nfc PRIVATE ${LIBUSB_LIBRARY_DIRS})
endif()
if(BUILD_SHARED_LIBS AND PCSC_LIBRARY_DIRS)
  target_link_directories(nfc PRIVATE ${PCSC_LIBRARY_DIRS})
endif()

if(UNIX AND NOT APPLE AND BUILD_SHARED_LIBS)
  file(WRITE "${LIBNFC_VERSION_SCRIPT}" "LIBNFC_6 {\n  global:\n")
  foreach(symbol ${LIBNFC_EXPORT_SYMBOLS})
    file(APPEND "${LIBNFC_VERSION_SCRIPT}" "    ${symbol};\n")
  endforeach()
  file(APPEND "${LIBNFC_VERSION_SCRIPT}" "  local:\n    *;\n};\n")
  target_link_options(nfc PRIVATE "-Wl,--version-script=${LIBNFC_VERSION_SCRIPT}")
endif()

set_target_properties(nfc PROPERTIES
  OUTPUT_NAME nfc
  VERSION 6.0.0
  SOVERSION 6)

if(WIN32)
  set_target_properties(nfc PROPERTIES PREFIX lib)
endif()

if(WIN32 AND BUILD_SHARED_LIBS)
  if(MINGW)
    add_custom_command(
      OUTPUT libnfc.lib
      COMMAND ${DLLTOOL} -d "${CMAKE_CURRENT_BINARY_DIR}/libnfc.def" -l "${CMAKE_CURRENT_BINARY_DIR}/libnfc.lib" "${CMAKE_CURRENT_BINARY_DIR}/libnfc.dll"
      DEPENDS nfc "${CMAKE_CURRENT_BINARY_DIR}/libnfc.def")
    add_custom_target(win32lib ALL DEPENDS libnfc.lib)
  endif()
endif()

install(TARGETS nfc
  EXPORT LibNFCTargets
  RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}" COMPONENT libraries
  LIBRARY DESTINATION "${CMAKE_INSTALL_LIBDIR}" COMPONENT libraries
  ARCHIVE DESTINATION "${CMAKE_INSTALL_LIBDIR}" COMPONENT libraries)

if(WIN32 AND BUILD_SHARED_LIBS AND MINGW)
  install(FILES "${CMAKE_CURRENT_BINARY_DIR}/libnfc.lib" DESTINATION "${CMAKE_INSTALL_LIBDIR}" COMPONENT libraries)
endif()

if(NOT BUILD_SHARED_LIBS)
  install(FILES "${RUST_LIB_PATH}" DESTINATION "${CMAKE_INSTALL_LIBDIR}" COMPONENT libraries)
endif()
