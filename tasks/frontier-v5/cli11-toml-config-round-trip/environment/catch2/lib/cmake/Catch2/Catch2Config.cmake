# Minimal CMake package for the Catch2 v2.13.10 single header CLI11's tests pin
# (sha256 3725c0f0a75f376a5005dde31ead0feb8f7da7507644c201b814443de8355170).
set(Catch2_VERSION 2.13.10)
if(NOT TARGET Catch2::Catch2)
  add_library(Catch2::Catch2 INTERFACE IMPORTED)
  get_filename_component(_vb_catch2_prefix "${CMAKE_CURRENT_LIST_DIR}/../../.." ABSOLUTE)
  set_target_properties(Catch2::Catch2 PROPERTIES INTERFACE_INCLUDE_DIRECTORIES "${_vb_catch2_prefix}/include")
endif()
