# LICENSE BEGIN
#
# This file is part of the Sixty FPS Project
#
# Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
# Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>
#
# SPDX-License-Identifier: GPL-3.0-only
#
# LICENSE END
cmake_minimum_required(VERSION 3.16)

project(SixtyFPS LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 17)

set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

add_library(SixtyFPS INTERFACE)

foreach(lib_path IN LISTS SIXTYFPS_INTERNAL_LIBS)
    get_filename_component(lib_name "${lib_path}" NAME_WE)

    add_library(${lib_name} SHARED IMPORTED)
    set_property(TARGET ${lib_name} PROPERTY IMPORTED_LOCATION "${lib_path}")

    target_link_libraries(SixtyFPS INTERFACE ${lib_name})
    list(APPEND internal_libs ${lib_name})

    install(FILES ${lib_path} TYPE LIB)
endforeach()


if (DEFINED SIXTYFPS_EXTERNAL_LIBS)
    target_link_libraries(SixtyFPS INTERFACE ${SIXTYFPS_EXTERNAL_LIBS})
endif()

set(api_headers
    sixtyfps.h
    sixtyfps_properties.h
    sixtyfps_signals.h
    sixtyfps_string.h
    vtable.h
)
foreach(header IN LISTS api_headers)
    set_property(TARGET SixtyFPS APPEND PROPERTY PUBLIC_HEADER ../include/${header})
endforeach()

file(GLOB_RECURSE generated_headers RELATIVE_PATH "${CMAKE_BINARY_DIR}" "${CMAKE_BINARY_DIR}/include/*.h")

foreach(header IN LISTS generated_headers)
    set_property(TARGET SixtyFPS APPEND PROPERTY PUBLIC_HEADER ${header})
endforeach()

target_include_directories(SixtyFPS INTERFACE
    $<BUILD_INTERFACE:${CMAKE_BINARY_DIR}/include>
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/../include>
    $<INSTALL_INTERFACE:include/sixtyfps>
)

export(TARGETS SixtyFPS NAMESPACE SixtyFPS:: FILE "${CMAKE_BINARY_DIR}/lib/cmake/SixtyFPS/SixtyFPSTargets.cmake")

install(EXPORT SixtyFPSTargets NAMESPACE SixtyFPS:: DESTINATION lib/cmake/SixtyFPS)

install(TARGETS SixtyFPS
    EXPORT SixtyFPSTargets
    LIBRARY DESTINATION lib
    PUBLIC_HEADER DESTINATION include/sixtyfps
)

include(CMakePackageConfigHelpers)

configure_package_config_file("SixtyFPSConfig.cmake.in" "${CMAKE_CURRENT_BINARY_DIR}/lib/cmake/SixtyFPS/SixtyFPSConfig.cmake" INSTALL_DESTINATION lib/cmake/SixtyFPS)

write_basic_package_version_file(
  ${CMAKE_CURRENT_BINARY_DIR}/lib/cmake/SixtyFPS/SixtyFPSConfigVersion.cmake
  VERSION 1.0.0
  COMPATIBILITY SameMajorVersion
)

install(FILES
    "${CMAKE_CURRENT_BINARY_DIR}/lib/cmake/SixtyFPS/SixtyFPSConfig.cmake"
    "${CMAKE_CURRENT_BINARY_DIR}/lib/cmake/SixtyFPS/SixtyFPSConfigVersion.cmake"
    DESTINATION lib/cmake/SixtyFPS
)

if(APPLE)
    execute_process(COMMAND brew --prefix llvm RESULT_VARIABLE prefix_status OUTPUT_VARIABLE llvm_prefix ERROR_QUIET OUTPUT_STRIP_TRAILING_WHITESPACE)
    if(prefix_status EQUAL "0")
        set(clang_doc_hint "HINTS;${llvm_prefix}/bin")
    endif()
endif()

find_program(clang_doc NAMES clang-doc clang-doc-10 ${clang_doc_hint})
if(clang_doc)
    message(STATUS "Clang doc found at ${clang_doc}. Will build API documentation")
    add_library(api_docs lib.cpp)
    target_link_libraries(api_docs SixtyFPS)
    add_custom_target(docs ALL COMMAND ${clang_doc} -p ${CMAKE_CURRENT_BINARY_DIR} --format=html --public --project-name "SixtyFPS C++" ${CMAKE_CURRENT_SOURCE_DIR}/lib.cpp)
else()
    message(STATUS "Clang doc NOT found. Will NOT build API documentation")
    add_custom_target(docs COMMAND ${CMAKE} -E echo "Not building documentation because clang-doc wasn't found in PATH")
endif()