INCLUDE_DIRECTORIES(
  ${CMAKE_SOURCE_DIR}
  ${CMAKE_SOURCE_DIR}/include
)

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

add_executable(test_core_registry
  test_rust_core_registry.c
)
target_link_libraries(test_core_registry nfc)
add_test(NAME core_registry COMMAND test_core_registry)

add_executable(test_core_initiator
  test_rust_core_initiator.c
)
target_link_libraries(test_core_initiator nfc)
add_test(NAME core_initiator COMMAND test_core_initiator)

add_executable(test_core_device_io
  test_rust_core_device_io.c
)
target_link_libraries(test_core_device_io nfc)
add_test(NAME core_device_io COMMAND test_core_device_io)

if(PROXIMATE_C_FFI)
  add_executable(test_log_bridge
    test_log_bridge.c
    ${CMAKE_SOURCE_DIR}/libnfc/log.c
  )
  target_compile_definitions(test_log_bridge PRIVATE LOG)
  target_link_libraries(test_log_bridge PRIVATE "${RUST_LIB_PATH}")
  add_test(NAME log_bridge COMMAND test_log_bridge)
endif()

find_package(LIBNFC_NCI QUIET)
set(PN71XX_SMOKE_BUILD_DIR "${CMAKE_CURRENT_BINARY_DIR}/pn71xx-smoke")
set(PN71XX_SMOKE_CONFIGURE_ARGS
  -S "${CMAKE_SOURCE_DIR}"
  -B "${PN71XX_SMOKE_BUILD_DIR}"
  -DBUILD_EXAMPLES=OFF
  -DBUILD_UTILS=OFF
  -DBUILD_TESTING=OFF
  -DLIBNFC_DRIVER_PN71XX=ON
)

if(CMAKE_GENERATOR)
  list(PREPEND PN71XX_SMOKE_CONFIGURE_ARGS -G "${CMAKE_GENERATOR}")
endif()
if(CMAKE_GENERATOR_PLATFORM)
  list(APPEND PN71XX_SMOKE_CONFIGURE_ARGS -A "${CMAKE_GENERATOR_PLATFORM}")
endif()
if(CMAKE_GENERATOR_TOOLSET)
  list(APPEND PN71XX_SMOKE_CONFIGURE_ARGS -T "${CMAKE_GENERATOR_TOOLSET}")
endif()
if(CMAKE_BUILD_TYPE)
  list(APPEND PN71XX_SMOKE_CONFIGURE_ARGS "-DCMAKE_BUILD_TYPE=${CMAKE_BUILD_TYPE}")
endif()
if(CMAKE_C_COMPILER)
  list(APPEND PN71XX_SMOKE_CONFIGURE_ARGS "-DCMAKE_C_COMPILER=${CMAKE_C_COMPILER}")
endif()
if(CMAKE_CXX_COMPILER)
  list(APPEND PN71XX_SMOKE_CONFIGURE_ARGS "-DCMAKE_CXX_COMPILER=${CMAKE_CXX_COMPILER}")
endif()

if(LIBNFC_NCI_FOUND)
  add_test(NAME pn71xx_configure_smoke
    COMMAND ${CMAKE_COMMAND} ${PN71XX_SMOKE_CONFIGURE_ARGS}
  )
  add_test(NAME pn71xx_build_smoke
    COMMAND ${CMAKE_COMMAND} --build "${PN71XX_SMOKE_BUILD_DIR}" --target nfc
  )
  set_tests_properties(pn71xx_build_smoke PROPERTIES DEPENDS pn71xx_configure_smoke)
else()
  add_test(NAME pn71xx_requires_libnfc_nci
    COMMAND ${CMAKE_COMMAND} ${PN71XX_SMOKE_CONFIGURE_ARGS}
  )
  set_tests_properties(pn71xx_requires_libnfc_nci PROPERTIES WILL_FAIL TRUE)
endif()

if(USB_REQUIRED)
  add_executable(test_usbbus_helpers
    test_usbbus_helpers.c
    test_usb_bridge_link_support.c
  )
  target_include_directories(test_usbbus_helpers PRIVATE
    ${CMAKE_SOURCE_DIR}/libnfc
  )
  if(PROXIMATE_C_FFI)
    target_compile_definitions(test_usbbus_helpers PRIVATE PROXIMATE_C_FFI=1)
    target_link_libraries(test_usbbus_helpers PRIVATE "${RUST_LIB_PATH}")
  endif()
  target_link_libraries(test_usbbus_helpers PRIVATE nfc)
  add_test(NAME usbbus_helpers COMMAND test_usbbus_helpers)
endif()
