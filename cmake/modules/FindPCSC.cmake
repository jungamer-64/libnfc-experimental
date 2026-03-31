find_package(PkgConfig QUIET)
if(PKG_CONFIG_FOUND AND NOT WIN32)
  pkg_check_modules(PCSC QUIET libpcsclite)
endif()

if(NOT PCSC_FOUND)
  find_path(PCSC_INCLUDE_DIRS
    NAMES WinSCard.h winscard.h)

  if(APPLE)
    find_library(PCSC_LIBRARIES NAMES PCSC)
  else()
    find_library(PCSC_LIBRARIES NAMES pcsclite PCSC libwinscard winscard WinSCard)
  endif()
endif()

if(PCSC_LIBRARIES AND NOT PCSC_LIBRARY_DIRS)
  get_filename_component(PCSC_LIBRARY_DIRS "${PCSC_LIBRARIES}" DIRECTORY)
endif()

include(FindPackageHandleStandardArgs)
find_package_handle_standard_args(PCSC REQUIRED_VARS PCSC_LIBRARIES PCSC_INCLUDE_DIRS)
mark_as_advanced(PCSC_INCLUDE_DIRS PCSC_LIBRARIES PCSC_LIBRARY_DIRS)
