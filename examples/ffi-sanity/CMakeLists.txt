add_executable(ffi-sanity ffi-sanity.c)
target_include_directories(ffi-sanity BEFORE PRIVATE
  ${CMAKE_SOURCE_DIR}/libnfc_orig/libnfc-master/include
)
target_link_libraries(ffi-sanity PRIVATE nfc)
install(TARGETS ffi-sanity RUNTIME DESTINATION bin COMPONENT examples)
