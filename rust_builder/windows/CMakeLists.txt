cmake_minimum_required(VERSION 3.14)
project(shuvpn_core LANGUAGES CXX)

find_program(CARGO_EXECUTABLE cargo REQUIRED)
# Flutter passes the plugin through a junction on Windows.
get_filename_component(RUST_ROOT "${CMAKE_SOURCE_DIR}/../rust" ABSOLUTE)

if(FLUTTER_TARGET_PLATFORM STREQUAL "windows-arm64")
  set(RUST_TARGET aarch64-pc-windows-msvc)
else()
  set(RUST_TARGET x86_64-pc-windows-msvc)
endif()

set(CARGO_TARGET_DIR "${CMAKE_CURRENT_BINARY_DIR}/cargo")
set(CARGO_PROFILE "$<IF:$<CONFIG:Debug>,debug,release>")
set(RUST_LIBRARY "${CARGO_TARGET_DIR}/${RUST_TARGET}/${CARGO_PROFILE}/shuvpn_core.dll")
set(RUST_TUN_HELPER "${CARGO_TARGET_DIR}/${RUST_TARGET}/${CARGO_PROFILE}/shuvpn_tun_helper.exe")

add_custom_target(shuvpn_core_build ALL
  COMMAND "${CARGO_EXECUTABLE}" build
    --manifest-path "${RUST_ROOT}/Cargo.toml"
    --locked
    --lib
    --bin shuvpn_tun_helper
    --target "${RUST_TARGET}"
    --target-dir "${CARGO_TARGET_DIR}"
    $<$<NOT:$<CONFIG:Debug>>:--release>
  BYPRODUCTS "${RUST_LIBRARY}" "${RUST_TUN_HELPER}"
  COMMAND_EXPAND_LISTS
  VERBATIM
)

set(shuvpn_core_bundled_libraries "${RUST_LIBRARY}" "${RUST_TUN_HELPER}" PARENT_SCOPE)
