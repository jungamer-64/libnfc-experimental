INCLUDE_DIRECTORIES(
  ${CMAKE_SOURCE_DIR}
  ${CMAKE_SOURCE_DIR}/include
  ${CMAKE_SOURCE_DIR}/libnfc
)

ADD_EXECUTABLE(test_acr122_core
  test_acr122_core.c
  ../libnfc/drivers/acr122-core.c
)

add_test(NAME acr122_core COMMAND test_acr122_core)

if(LIBUSB_FOUND)
  add_executable(test_usbbus_helpers
    test_usbbus_helpers.c
    ../libnfc/buses/usbbus.c
    ../libnfc/log.cpp
    ../libnfc/log-internal.c
  )
  target_include_directories(test_usbbus_helpers PRIVATE
    ${LIBUSB_INCLUDE_DIRS}
  )
  target_link_libraries(test_usbbus_helpers ${LIBUSB_LIBRARIES})
  add_test(NAME usbbus_helpers COMMAND test_usbbus_helpers)
endif()
