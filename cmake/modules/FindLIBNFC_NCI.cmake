find_package(PkgConfig QUIET)

if(PKG_CONFIG_FOUND AND NOT WIN32)
  pkg_check_modules(LIBNFC_NCI QUIET libnfc-nci)
endif()

if(NOT LIBNFC_NCI_FOUND)
  find_path(LIBNFC_NCI_INCLUDE_DIRS
    NAMES linux_nfc_api.h)

  find_library(LIBNFC_NCI_LIBRARIES
    NAMES nfc_nci_linux libnfc_nci_linux)
endif()

if(LIBNFC_NCI_LIBRARIES AND NOT LIBNFC_NCI_LIBRARY_DIRS)
  get_filename_component(LIBNFC_NCI_LIBRARY_DIRS "${LIBNFC_NCI_LIBRARIES}" DIRECTORY)
endif()

include(FindPackageHandleStandardArgs)
find_package_handle_standard_args(LIBNFC_NCI REQUIRED_VARS LIBNFC_NCI_INCLUDE_DIRS LIBNFC_NCI_LIBRARIES)
mark_as_advanced(LIBNFC_NCI_INCLUDE_DIRS LIBNFC_NCI_LIBRARIES LIBNFC_NCI_LIBRARY_DIRS)
