# LICENSE BEGIN
# This file is part of the SixtyFPS Project -- https://sixtyfps.io
# Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
# Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>
#
# SPDX-License-Identifier: GPL-3.0-only
# This file is also available under commercial licensing terms.
# Please contact info@sixtyfps.io for more information.
# LICENSE END
cmake_minimum_required(VERSION 3.16)
project(SixtyFPS LANGUAGES CXX)

include(FetchContent)
FetchContent_Declare(
    Corrosion
    GIT_REPOSITORY https://github.com/AndrewGaspar/corrosion.git
    GIT_TAG 7c6236a60ae97bb3d447c819e4e88a1967439126
)
FetchContent_MakeAvailable(Corrosion)
corrosion_import_crate(MANIFEST_PATH "${CMAKE_CURRENT_SOURCE_DIR}/../../Cargo.toml"
    CRATES sixtyfps-compiler sixtyfps-rendering-backend-default xtask)

add_library(SixtyFPS INTERFACE)
add_library(SixtyFPS::SixtyFPS ALIAS SixtyFPS)
target_link_libraries(SixtyFPS INTERFACE sixtyfps-rendering-backend-default)
target_compile_features(SixtyFPS INTERFACE cxx_std_17)

file(GLOB api_headers RELATIVE "${CMAKE_CURRENT_SOURCE_DIR}/include/"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/*.h")

foreach(header IN LISTS api_headers)
    set_property(TARGET SixtyFPS APPEND PROPERTY PUBLIC_HEADER include/${header})
endforeach()

set(generated_headers
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/sixtyfps_internal.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/sixtyfps_string_internal.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/sixtyfps_sharedarray_internal.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/sixtyfps_properties_internal.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/sixtyfps_resource_internal.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/sixtyfps_color_internal.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/sixtyfps_pathdata_internal.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/sixtyfps_qt_internal.h
    ${CMAKE_CURRENT_BINARY_DIR}/generated_include/sixtyfps_default_backend_internal.h
)

file(GLOB generated_headers_dependencies
    "${CMAKE_CURRENT_SOURCE_DIR}/../../sixtyfps_runtime/corelib/*.rs")

add_custom_target(
    generated_headers_target
    COMMAND
    xtask cbindgen --output-dir "${CMAKE_CURRENT_BINARY_DIR}/generated_include/"
    BYPRODUCTS ${generated_headers}
    DEPENDS ${generated_headers_dependencies}
)

add_dependencies(SixtyFPS generated_headers_target)
foreach(header IN LISTS generated_headers)
    set_property(TARGET SixtyFPS APPEND PROPERTY PUBLIC_HEADER ${header})
endforeach()

target_include_directories(SixtyFPS INTERFACE
    $<BUILD_INTERFACE:${CMAKE_CURRENT_BINARY_DIR}/generated_include>
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
    $<INSTALL_INTERFACE:include/sixtyfps>
)

add_executable(SixtyFPS::sixtyfps_compiler ALIAS sixtyfps_compiler)
include(${CMAKE_CURRENT_LIST_DIR}/cmake/SixtyFPSMacro.cmake)

export(TARGETS SixtyFPS sixtyfps-rendering-backend-default
    NAMESPACE SixtyFPS:: FILE "${CMAKE_BINARY_DIR}/lib/cmake/SixtyFPS/SixtyFPSTargets.cmake")
install(EXPORT SixtyFPSTargets NAMESPACE SixtyFPS:: DESTINATION lib/cmake/SixtyFPS)
install(TARGETS SixtyFPS sixtyfps-rendering-backend-default
    EXPORT SixtyFPSTargets  LIBRARY DESTINATION lib PUBLIC_HEADER DESTINATION include/sixtyfps)

include(CMakePackageConfigHelpers)
include(GNUInstallDirs)

install(FILES $<TARGET_FILE:sixtyfps-rendering-backend-default-shared> TYPE LIB)
if(WIN32)
    install(FILES $<TARGET_LINKER_FILE:sixtyfps-rendering-backend-default-shared> TYPE LIB)
endif()

install(PROGRAMS $<TARGET_FILE:sixtyfps_compiler> TYPE BIN)

set(SIXTYFPS_LIB_PROPERTIES "")
foreach(prop
        IMPORTED_LOCATION IMPORTED_LOCATION_DEBUG IMPORTED_LOCATION_RELEASE
        IMPORTED_IMPLIB IMPORTED_IMPLIB_DEBUG IMPORTED_IMPLIB_RELEASE)
  get_target_property(value sixtyfps-rendering-backend-default-shared ${prop})
  if(value)
    get_filename_component(value ${value} NAME)
    list(APPEND SIXTYFPS_LIB_PROPERTIES ${prop} "\${_IMPORT_PREFIX}/${CMAKE_INSTALL_LIBDIR}/${value}")
  endif()
endforeach()



configure_package_config_file("cmake/SixtyFPSConfig.cmake.in" "${CMAKE_CURRENT_BINARY_DIR}/lib/cmake/SixtyFPS/SixtyFPSConfig.cmake" INSTALL_DESTINATION lib/cmake/SixtyFPS)

write_basic_package_version_file(
  ${CMAKE_CURRENT_BINARY_DIR}/lib/cmake/SixtyFPS/SixtyFPSConfigVersion.cmake
  VERSION 0.0.2
  COMPATIBILITY ExactVersion
)

install(FILES
    "${CMAKE_CURRENT_BINARY_DIR}/lib/cmake/SixtyFPS/SixtyFPSConfig.cmake"
    "${CMAKE_CURRENT_BINARY_DIR}/lib/cmake/SixtyFPS/SixtyFPSConfigVersion.cmake"
    "${CMAKE_CURRENT_LIST_DIR}/cmake/SixtyFPSMacro.cmake"
    DESTINATION lib/cmake/SixtyFPS
)

set(CPACK_PACKAGE_NAME "SixtyFPS")
set(CPACK_PACKAGE_VENDOR "SixtyFPS")
set(CPACK_VERBATIM_VARIABLES true)
set(CPACK_PACKAGE_VERSION_MAJOR 0)
set(CPACK_PACKAGE_VERSION_MINOR 0)
set(CPACK_PACKAGE_VERSION_PATCH 2)
set(CPACK_PACKAGE_HOMEPAGE_URL "https://sixtyfps.io")
set(CPACK_RESOURCE_FILE_LICENSE "${CMAKE_CURRENT_LIST_DIR}/../../LICENSE.md")
set(CPACK_RESOURCE_FILE_README "${CMAKE_CURRENT_LIST_DIR}/README.md")
set(CPACK_STRIP_FILES ON)
include(CPack)

