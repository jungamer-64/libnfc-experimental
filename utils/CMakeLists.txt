set(UTILITY_SOURCES
  nfc-barcode
  nfc-emulate-forum-tag4
  nfc-jewel
  nfc-list
  nfc-mfclassic
  nfc-mfultralight
  nfc-read-forum-tag3
  nfc-relay-picc
  nfc-scan-device
)

add_library(nfcutils STATIC
  nfc-utils.c
)
target_link_libraries(nfcutils PRIVATE nfc)

foreach(source ${UTILITY_SOURCES})
  set(TARGETS "${source}.c")

  if(WIN32)
    set(RC_COMMENT "${PACKAGE_NAME} utility")
    set(RC_INTERNAL_NAME "${source}")
    set(RC_ORIGINAL_NAME "${source}.exe")
    set(RC_FILE_TYPE 0x00000001L)
    configure_file("${CMAKE_CURRENT_SOURCE_DIR}/../contrib/win32/version.rc.in" "${CMAKE_CURRENT_BINARY_DIR}/../windows/${source}.rc" @ONLY)
    list(APPEND TARGETS "${CMAKE_CURRENT_BINARY_DIR}/../windows/${source}.rc")
  endif()

  if(source MATCHES "nfc-jewel")
    list(APPEND TARGETS jewel.c)
  endif()

  if(source MATCHES "nfc-mfultralight" OR source MATCHES "nfc-mfclassic")
    list(APPEND TARGETS mifare.c)
  endif()

  if(WIN32)
    if(source MATCHES "nfc-scan-device")
      list(APPEND TARGETS ../contrib/win32/stdlib.c)
      include_directories("${CMAKE_CURRENT_SOURCE_DIR}/../contrib/win32")
    endif()
    if(source MATCHES "nfc-read-forum-tag3")
      list(APPEND TARGETS "${CMAKE_CURRENT_SOURCE_DIR}/../contrib/win32/getopt.c")
    endif()
  endif()

  add_executable("${source}" ${TARGETS})
  target_link_libraries("${source}" PRIVATE nfc nfcutils)
  install(TARGETS "${source}" RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}" COMPONENT utils)
endforeach()

if(BUILD_MANUAL)
  file(GLOB manuals "${CMAKE_CURRENT_SOURCE_DIR}/*.1")
  install(FILES ${manuals} DESTINATION "${CMAKE_INSTALL_MANDIR}/man1" COMPONENT manuals)
endif()
