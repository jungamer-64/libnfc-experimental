# This CMake script looks for a native libusb-1.0 installation.
#
# - POSIX platforms use pkg-config and require the libusb-1.0 module.
# - Windows looks for libusb.h together with an import library named
#   usb-1.0 or libusb-1.0.

if(WIN32)
  find_path(LIBUSB_INCLUDE_DIRS
    NAMES libusb.h
    PATH_SUFFIXES include include/libusb-1.0)

  find_library(LIBUSB_LIBRARIES
    NAMES usb-1.0 libusb-1.0)

  if(LIBUSB_LIBRARIES)
    get_filename_component(LIBUSB_LIBRARY_DIRS "${LIBUSB_LIBRARIES}" DIRECTORY)
  endif()
else()
  find_package(PkgConfig)
  if(PKG_CONFIG_FOUND)
    pkg_check_modules(LIBUSB REQUIRED libusb-1.0)
  else()
    message(FATAL_ERROR "Could not find PkgConfig")
  endif()
endif()

if(LIBUSB_INCLUDE_DIRS AND LIBUSB_LIBRARIES)
  set(LIBUSB_FOUND TRUE)
else()
  set(LIBUSB_FOUND FALSE)
endif()

if(LIBUSB_FOUND)
  if(NOT LIBUSB_FIND_QUIETLY)
    message(STATUS "Found LIBUSB: ${LIBUSB_LIBRARIES} ${LIBUSB_INCLUDE_DIRS}")
  endif()
elseif(LIBUSB_FIND_REQUIRED)
  message(FATAL_ERROR "Could not find LIBUSB (libusb-1.0)")
endif()
