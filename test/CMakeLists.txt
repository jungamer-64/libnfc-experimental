INCLUDE_DIRECTORIES(
  ${CMAKE_SOURCE_DIR}
  ${CMAKE_SOURCE_DIR}/include
)

ADD_EXECUTABLE(test_acr122_core
  test_acr122_core.c
  ../libnfc/drivers/acr122-core.c
)
target_include_directories(test_acr122_core PRIVATE
  ${CMAKE_SOURCE_DIR}/libnfc
)

add_test(NAME acr122_core COMMAND test_acr122_core)

add_executable(test_public_compat_smoke
  test_public_compat_smoke.c
)
target_link_libraries(test_public_compat_smoke nfc)
add_test(NAME public_compat_smoke COMMAND test_public_compat_smoke)

add_executable(ffi-sanity
  ../examples/ffi-sanity/ffi-sanity.c
)
target_link_libraries(ffi-sanity nfc)
add_test(NAME ffi_sanity COMMAND ffi-sanity)

if(LIBUSB_FOUND)
  add_executable(test_usbbus_helpers
    test_usbbus_helpers.c
    ../libnfc/buses/usbbus.c
    ../libnfc/log.cpp
    ../libnfc/log-internal.c
  )
  target_include_directories(test_usbbus_helpers PRIVATE
    ${CMAKE_SOURCE_DIR}/libnfc
    ${LIBUSB_INCLUDE_DIRS}
  )
  target_link_libraries(test_usbbus_helpers ${LIBUSB_LIBRARIES})
  add_test(NAME usbbus_helpers COMMAND test_usbbus_helpers)
endif()
