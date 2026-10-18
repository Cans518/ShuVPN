# Project-level configuration.
cmake_minimum_required(VERSION 3.13)
project(runner LANGUAGES CXX)

# The name of the executable created for the application. Change this to change
# the on-disk name of your application.
set(BINARY_NAME "shuvpn")
# The unique GTK application identifier for this application. See:
# https://wiki.gnome.org/HowDoI/ChooseApplicationID
set(APPLICATION_ID "work.shuvpn.app")

# Explicitly opt in to modern CMake behaviors to avoid warnings with recent
# versions of CMake.
cmake_policy(SET CMP0063 NEW)

# Load bundled libraries from the lib/ directory relative to the binary.
set(CMAKE_INSTALL_RPATH "$ORIGIN/lib")

# Root filesystem for cross-building.
if(FLUTTER_TARGET_PLATFORM_SYSROOT)
  set(CMAKE_SYSROOT ${FLUTTER_TARGET_PLATFORM_SYSROOT})
  set(CMAKE_FIND_ROOT_PATH ${CMAKE_SYSROOT})
  set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
  set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)
  set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
  set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
endif()

# Define build configuration options.
if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
  set(CMAKE_BUILD_TYPE "Debug" CACHE
    STRING "Flutter build mode" FORCE)
  set_property(CACHE CMAKE_BUILD_TYPE PROPERTY STRINGS
    "Debug" "Profile" "Release")
endif()

# Compilation settings that should be applied to most targets.
#
# Be cautious about adding new options here, as plugins use this function by
# default. In most cases, you should add new options to specific targets instead
# of modifying this function.
function(APPLY_STANDARD_SETTINGS TARGET)
  target_compile_features(${TARGET} PUBLIC cxx_std_14)
  target_compile_options(${TARGET} PRIVATE -Wall -Werror)
  target_compile_options(${TARGET} PRIVATE "$<$<NOT:$<CONFIG:Debug>>:-O3>")
  target_compile_definitions(${TARGET} PRIVATE "$<$<NOT:$<CONFIG:Debug>>:NDEBUG>")
endfunction()

# Flutter library and tool build rules.
set(FLUTTER_MANAGED_DIR "${CMAKE_CURRENT_SOURCE_DIR}/flutter")
add_subdirectory(${FLUTTER_MANAGED_DIR})

# System-level dependencies.
find_package(PkgConfig REQUIRED)
pkg_check_modules(GTK REQUIRED IMPORTED_TARGET gtk+-3.0)

# Application build; see runner/CMakeLists.txt.
add_subdirectory("runner")

# Run the Flutter tool portions of the build. This must not be removed.
add_dependencies(${BINARY_NAME} flutter_assemble)

# Only the install-generated bundle's copy of the executable will launch
# correctly, since the resources must in the right relative locations. To avoid
# people trying to run the unbundled copy, put it in a subdirectory instead of
# the default top-level location.
set_target_properties(${BINARY_NAME}
  PROPERTIES
  RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/intermediates_do_not_run"
)


# Generated plugin build rules, which manage building the plugins and adding
# them to the application.
include(flutter/generated_plugins.cmake)

# The pinned plugin's Linux menu update methods are stubs. Replace that one
# translation unit without changing the shared package cache or other platforms.
get_target_property(TRAY_SOURCE_DIR system_tray_plugin SOURCE_DIR)
file(SHA256 "${TRAY_SOURCE_DIR}/menu.cc" TRAY_UPSTREAM_SHA)
if(NOT TRAY_UPSTREAM_SHA STREQUAL "8f682c66fdaf2de8726d1b72a6c3252c78ec53dc5fb8bd60749ab404c34b6337")
  message(FATAL_ERROR "Review the Linux tray menu adaptation for this system_tray source")
endif()
get_target_property(TRAY_SOURCES system_tray_plugin SOURCES)
list(REMOVE_ITEM TRAY_SOURCES "menu.cc")
set_property(TARGET system_tray_plugin PROPERTY SOURCES "${TRAY_SOURCES}")
target_sources(system_tray_plugin PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/third_party/system_tray/menu.cc")
target_include_directories(system_tray_plugin PRIVATE "${TRAY_SOURCE_DIR}")

add_executable(shuvpn_tray_menu_test EXCLUDE_FROM_ALL
  "${CMAKE_CURRENT_SOURCE_DIR}/../test/native/linux_tray_menu_test.cc"
  "${CMAKE_CURRENT_SOURCE_DIR}/third_party/system_tray/menu.cc"
  "${TRAY_SOURCE_DIR}/errors.cc")
apply_standard_settings(shuvpn_tray_menu_test)
target_include_directories(shuvpn_tray_menu_test PRIVATE "${TRAY_SOURCE_DIR}")
target_link_libraries(shuvpn_tray_menu_test PRIVATE flutter PkgConfig::GTK)
add_dependencies(shuvpn_tray_menu_test flutter_assemble)

add_executable(shuvpn_view_accessibility_test EXCLUDE_FROM_ALL
  "${CMAKE_CURRENT_SOURCE_DIR}/../test/native/linux_view_accessibility_test.cc")
apply_standard_settings(shuvpn_view_accessibility_test)
target_include_directories(shuvpn_view_accessibility_test PRIVATE
  "${CMAKE_CURRENT_SOURCE_DIR}/runner")
target_link_libraries(shuvpn_view_accessibility_test PRIVATE flutter PkgConfig::GTK)
add_dependencies(shuvpn_view_accessibility_test flutter_assemble)

# === Installation ===
# By default, "installing" just makes a relocatable bundle in the build
# directory.
set(BUILD_BUNDLE_DIR "${PROJECT_BINARY_DIR}/bundle")
if(CMAKE_INSTALL_PREFIX_INITIALIZED_TO_DEFAULT)
  set(CMAKE_INSTALL_PREFIX "${BUILD_BUNDLE_DIR}" CACHE PATH "..." FORCE)
endif()

# Start with a clean build bundle directory every time.
install(CODE "
  file(REMOVE_RECURSE \"${BUILD_BUNDLE_DIR}/\")
  " COMPONENT Runtime)

set(INSTALL_BUNDLE_DATA_DIR "${CMAKE_INSTALL_PREFIX}/data")
set(INSTALL_BUNDLE_LIB_DIR "${CMAKE_INSTALL_PREFIX}/lib")

install(TARGETS ${BINARY_NAME} RUNTIME DESTINATION "${CMAKE_INSTALL_PREFIX}"
  COMPONENT Runtime)
install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/install.py"
  "${CMAKE_CURRENT_SOURCE_DIR}/shuvpn.desktop"
  DESTINATION "${CMAKE_INSTALL_PREFIX}" COMPONENT Runtime)
install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/third_party/system_tray/LICENSE"
  DESTINATION "${CMAKE_INSTALL_PREFIX}/data/licenses/system_tray" COMPONENT Runtime)

install(FILES "${FLUTTER_ICU_DATA_FILE}" DESTINATION "${INSTALL_BUNDLE_DATA_DIR}"
  COMPONENT Runtime)

install(FILES "${FLUTTER_LIBRARY}" DESTINATION "${INSTALL_BUNDLE_LIB_DIR}"
  COMPONENT Runtime)

foreach(bundled_library ${PLUGIN_BUNDLED_LIBRARIES})
  get_filename_component(bundled_name "${bundled_library}" NAME)
  if(bundled_name STREQUAL "shuvpn_tun_helper")
    install(PROGRAMS "${bundled_library}"
      DESTINATION "${CMAKE_INSTALL_PREFIX}" COMPONENT Runtime)
  else()
    install(FILES "${bundled_library}"
      DESTINATION "${INSTALL_BUNDLE_LIB_DIR}"
      COMPONENT Runtime)
  endif()
endforeach(bundled_library)

# Copy the native assets provided by the build.dart from all packages.
set(NATIVE_ASSETS_DIR "${PROJECT_BUILD_DIR}native_assets/linux/")
install(DIRECTORY "${NATIVE_ASSETS_DIR}"
   DESTINATION "${INSTALL_BUNDLE_LIB_DIR}"
   COMPONENT Runtime)

# Fully re-copy the assets directory on each build to avoid having stale files
# from a previous install.
set(FLUTTER_ASSET_DIR_NAME "flutter_assets")
install(CODE "
  file(REMOVE_RECURSE \"${INSTALL_BUNDLE_DATA_DIR}/${FLUTTER_ASSET_DIR_NAME}\")
  " COMPONENT Runtime)
install(DIRECTORY "${PROJECT_BUILD_DIR}/${FLUTTER_ASSET_DIR_NAME}"
  DESTINATION "${INSTALL_BUNDLE_DATA_DIR}" COMPONENT Runtime)

# Install the AOT library on non-Debug builds only.
if(NOT CMAKE_BUILD_TYPE MATCHES "Debug")
  install(FILES "${AOT_LIBRARY}" DESTINATION "${INSTALL_BUNDLE_LIB_DIR}"
    COMPONENT Runtime)
endif()
