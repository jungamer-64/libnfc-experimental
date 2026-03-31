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
