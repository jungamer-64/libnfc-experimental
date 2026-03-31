set(EXAMPLE_SOURCES
  nfc-anticol
  nfc-dep-initiator
  nfc-dep-target
  nfc-emulate-forum-tag2
  nfc-emulate-tag
  nfc-emulate-uid
  nfc-mfsetuid
  nfc-poll
  nfc-relay
  nfc-st25tb
  pn53x-diagnose
  pn53x-sam
  pn53x-tamashell
)

include_directories("${CMAKE_CURRENT_SOURCE_DIR}/../libnfc")

foreach(source ${EXAMPLE_SOURCES})
  set(TARGETS "${source}.c")

  if(WIN32)
    set(RC_COMMENT "${PACKAGE_NAME} example")
    set(RC_INTERNAL_NAME "${source}")
    set(RC_ORIGINAL_NAME "${source}.exe")
    set(RC_FILE_TYPE 0x00000001L)
    configure_file("${CMAKE_CURRENT_SOURCE_DIR}/../contrib/win32/version.rc.in" "${CMAKE_CURRENT_BINARY_DIR}/../windows/${source}.rc" @ONLY)
    list(APPEND TARGETS "${CMAKE_CURRENT_BINARY_DIR}/../windows/${source}.rc")

    if(source MATCHES "nfc-st25tb")
      list(APPEND TARGETS "${CMAKE_CURRENT_SOURCE_DIR}/../contrib/win32/getopt.c")
    endif()
  endif()

  add_executable("${source}" ${TARGETS})
  target_link_libraries("${source}" PRIVATE nfc nfcutils)
  install(TARGETS "${source}" RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}" COMPONENT examples)
endforeach()

if(BUILD_MANUAL)
  file(GLOB manuals "${CMAKE_CURRENT_SOURCE_DIR}/*.1")
  install(FILES ${manuals} DESTINATION "${CMAKE_INSTALL_MANDIR}/man1" COMPONENT manuals)
endif()
