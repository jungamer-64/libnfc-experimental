find_package(PkgConfig QUIET)

if(PKG_CONFIG_FOUND AND NOT WIN32)
  pkg_check_modules(LIBUSB QUIET libusb-1.0)
endif()

if(NOT LIBUSB_FOUND)
  if(WIN32)
    find_path(LIBUSB_INCLUDE_DIRS
      NAMES libusb.h
      PATH_SUFFIXES include include/libusb-1.0)

    find_library(LIBUSB_LIBRARIES
      NAMES usb-1.0 libusb-1.0)
  else()
    find_path(LIBUSB_INCLUDE_DIRS
      NAMES libusb.h
      PATH_SUFFIXES include include/libusb-1.0)

    find_library(LIBUSB_LIBRARIES
      NAMES usb-1.0 libusb-1.0 usb)
  endif()
endif()

if(LIBUSB_LIBRARIES AND NOT LIBUSB_LIBRARY_DIRS)
  get_filename_component(LIBUSB_LIBRARY_DIRS "${LIBUSB_LIBRARIES}" DIRECTORY)
endif()

include(FindPackageHandleStandardArgs)
find_package_handle_standard_args(LIBUSB REQUIRED_VARS LIBUSB_INCLUDE_DIRS LIBUSB_LIBRARIES)
mark_as_advanced(LIBUSB_INCLUDE_DIRS LIBUSB_LIBRARIES LIBUSB_LIBRARY_DIRS)
