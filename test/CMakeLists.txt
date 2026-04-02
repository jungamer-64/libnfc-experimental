INCLUDE_DIRECTORIES(
  ${CMAKE_SOURCE_DIR}
  ${CMAKE_SOURCE_DIR}/include
)
if(PROXIMATE_C_FFI)
  include_directories("${RUST_GENERATED_INCLUDE_DIR}")
endif()

# C-side tests are limited to public installed API smoke coverage.
# Private bridge and repo-local header coverage lives in Rust tests.

add_executable(test_public_compat_smoke
  test_public_compat_smoke.c
)
if(PROXIMATE_C_FFI)
  add_dependencies(test_public_compat_smoke proximate_sys_headers)
endif()
target_link_libraries(test_public_compat_smoke nfc)
add_test(NAME public_compat_smoke COMMAND test_public_compat_smoke)

add_executable(ffi-sanity
  ../examples/ffi-sanity/ffi-sanity.c
)
if(PROXIMATE_C_FFI)
  add_dependencies(ffi-sanity proximate_sys_headers)
endif()
target_link_libraries(ffi-sanity nfc)
add_test(NAME ffi_sanity COMMAND ffi-sanity)

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

add_test(NAME no_retired_private_headers
  COMMAND bash "${CMAKE_SOURCE_DIR}/scripts/check-no-retired-private-header-includes.sh")
