cmake_minimum_required(VERSION 3.21)
project(heshun_tsf LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
add_compile_options(/utf-8)

add_library(heshun_tsf SHARED
  src/dllmain.cpp
  src/class_factory.cpp
  src/text_service.cpp
  src/compartment_sink.cpp
  src/candidate_list.cpp
  src/candidate_window.cpp
  src/heshun_icon.rc
  src/heshun_tsf.def
)

target_compile_definitions(heshun_tsf PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_include_directories(heshun_tsf PRIVATE
  ${CMAKE_CURRENT_SOURCE_DIR}/src
  ${CMAKE_CURRENT_SOURCE_DIR}/../heshun/include
)
target_link_libraries(heshun_tsf PRIVATE ole32 advapi32 ws2_32 ntdll userenv)
set_target_properties(heshun_tsf PROPERTIES
  OUTPUT_NAME "heshun_tsf"
  RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/bin"
  LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/bin"
)

# Rust's import library is intentionally linked by path, not checked in.
# Build one TSF DLL per host architecture: 32-bit hosts (WOW64) cannot load
# the x64 component, while 64-bit hosts cannot load the Win32 component.
if(CMAKE_SIZEOF_VOID_P EQUAL 8)
  set(HESHUN_RUST_TARGET "x86_64-pc-windows-msvc")
else()
  set(HESHUN_RUST_TARGET "i686-pc-windows-msvc")
endif()
set(HESHUN_RELEASE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../target/${HESHUN_RUST_TARGET}/release")
if(EXISTS "${HESHUN_RELEASE_DIR}/heshun.lib")
  target_link_libraries(heshun_tsf PRIVATE "${HESHUN_RELEASE_DIR}/heshun.lib")
  add_custom_command(TARGET heshun_tsf POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_if_different
      "${HESHUN_RELEASE_DIR}/heshun.dll" "$<TARGET_FILE_DIR:heshun_tsf>/heshun.dll"
    COMMAND ${CMAKE_COMMAND} -E make_directory "$<TARGET_FILE_DIR:heshun_tsf>/schemas"
    COMMAND ${CMAKE_COMMAND} -E copy_directory
      "${CMAKE_CURRENT_SOURCE_DIR}/../heshun/schemas" "$<TARGET_FILE_DIR:heshun_tsf>/schemas"
    COMMENT "Copying heshun runtime and schemas beside the TSF DLL"
  )
else()
  message(FATAL_ERROR "Missing heshun.lib. Build it first: cargo build -p heshun --release --target ${HESHUN_RUST_TARGET}")
endif()

add_executable(heshun_tsf_profile src/profile_tool.cpp)
add_executable(heshun_tsf_activation_probe tests/com_activation_probe.cpp)
add_executable(heshun_tsf_abi_header_contract tests/abi_header_contract.cpp)
add_executable(heshun_tsf_tsf_interface_contract tests/tsf_interface_contract.cpp)
target_compile_definitions(heshun_tsf_profile PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_compile_definitions(heshun_tsf_activation_probe PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_compile_definitions(heshun_tsf_abi_header_contract PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_compile_definitions(heshun_tsf_tsf_interface_contract PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_include_directories(heshun_tsf_profile PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/src)
target_include_directories(heshun_tsf_activation_probe PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/src)
target_include_directories(heshun_tsf_abi_header_contract PRIVATE
  ${CMAKE_CURRENT_SOURCE_DIR}/src
  ${CMAKE_CURRENT_SOURCE_DIR}/../heshun/include
)
target_include_directories(heshun_tsf_tsf_interface_contract PRIVATE
  ${CMAKE_CURRENT_SOURCE_DIR}/src
  ${CMAKE_CURRENT_SOURCE_DIR}/../heshun/include
)
target_link_libraries(heshun_tsf_profile PRIVATE ole32)
target_link_libraries(heshun_tsf_activation_probe PRIVATE ole32)
set_target_properties(heshun_tsf_activation_probe PROPERTIES
  RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/bin"
)
set_target_properties(heshun_tsf_profile PROPERTIES
  RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/bin"
)
set_target_properties(heshun_tsf_abi_header_contract PROPERTIES
  RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/bin"
)
set_target_properties(heshun_tsf_tsf_interface_contract PROPERTIES
  RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/bin"
)

enable_testing()
add_test(NAME tsf_binary_exports
  COMMAND powershell -NoProfile -ExecutionPolicy Bypass -File
    "${CMAKE_CURRENT_SOURCE_DIR}/tests/verify_exports.ps1" "$<TARGET_FILE:heshun_tsf>"
)
add_test(NAME tsf_abi_header_contract
  COMMAND "$<TARGET_FILE:heshun_tsf_abi_header_contract>"
)
add_test(NAME tsf_tsf_interface_contract
  COMMAND "$<TARGET_FILE:heshun_tsf_tsf_interface_contract>"
)
