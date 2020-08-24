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

find_package(Doxygen)
find_program(PipEnv pipenv)
if (DOXYGEN_FOUND)
    if (PipEnv)
       configure_file(../docs/conf.py.in docs/conf.py)
       file(CREATE_LINK ${CMAKE_CURRENT_SOURCE_DIR}/../docs/index.rst ${CMAKE_CURRENT_BINARY_DIR}/docs/index.rst SYMBOLIC)
       file(CREATE_LINK ${CMAKE_CURRENT_SOURCE_DIR}/../README.md ${CMAKE_CURRENT_BINARY_DIR}/docs/README.md SYMBOLIC)
       get_filename_component(markdown_docs_source ../../../docs ABSOLUTE)
       file(CREATE_LINK ${markdown_docs_source} ${CMAKE_CURRENT_BINARY_DIR}/docs/markdown SYMBOLIC)
       execute_process(COMMAND ${CMAKE_COMMAND} -E env PIPENV_PIPFILE=${CMAKE_CURRENT_SOURCE_DIR}/../docs/Pipfile ${PipEnv} install)
       add_custom_target(docs ALL COMMAND ${CMAKE_COMMAND} -E env PIPENV_PIPFILE=${CMAKE_CURRENT_SOURCE_DIR}/../docs/Pipfile ${PipEnv} run sphinx-build ./docs ./docs/html)
    else()
        message("Pipenv not found, not building documentation")
    endif()
else()
    message("Doxygen not found, not building documentation")
endif()
