# LICENSE BEGIN
# This file is part of the SixtyFPS Project -- https://sixtyfps.io
# Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
# Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>
#
# SPDX-License-Identifier: GPL-3.0-only
# This file is also available under commercial licensing terms.
# Please contact info@sixtyfps.io for more information.
# LICENSE END
cmake_minimum_required(VERSION 3.16)

project(SixtyFPS LANGUAGES CXX)

option(BUILD_TESTING "Build tests" OFF)
include(CTest)

add_subdirectory(api/sixtyfps-cpp/)

option(SIXTYFPS_BUILD_EXAMPLES "Build SixtyFPS Examples" OFF)

if(SIXTYFPS_BUILD_EXAMPLES)
    # Place all compiled examples into the same bin directory
    # on Windows, where we'll also put the dll
    if (WIN32)
        set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)
    endif()
    add_subdirectory(examples/printerdemo/cpp/)
    if (SIXTYFPS_FEATURE_INTERPRETER)
        add_subdirectory(examples/printerdemo/cpp_interpreted/)
    endif()
    add_subdirectory(examples/printerdemo_old/cpp/)
    add_subdirectory(examples/todo/cpp/)
    add_subdirectory(examples/gallery/)
    add_subdirectory(examples/memory/)
    if (SIXTYFPS_FEATURE_INTERPRETER)
        add_subdirectory(examples/qt_viewer/)
    endif()
    # iot-dashboard uses fmtlib via FetchContent and 3.16 has a bug
    # configuring it (issue #232)
    if (SIXTYFPS_FEATURE_INTERPRETER AND CMAKE_VERSION VERSION_GREATER_EQUAL 3.17.0)
        add_subdirectory(examples/iot-dashboard/)
    endif()
    if(BUILD_TESTING)
        add_subdirectory(docs/tutorial/cpp/src/)
    endif()
endif()
