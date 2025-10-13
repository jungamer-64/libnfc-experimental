add_executable(ffi-sanity ffi-sanity.c)
target_include_directories(ffi-sanity PRIVATE ${CMAKE_SOURCE_DIR}/rust/libnfc-rs/include ${CMAKE_CURRENT_SOURCE_DIR}/..)
target_link_libraries(ffi-sanity PRIVATE nfc)
install(TARGETS ffi-sanity RUNTIME DESTINATION bin COMPONENT examples)
