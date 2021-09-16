# LICENSE BEGIN
# This file is part of the SixtyFPS Project -- https://sixtyfps.io
# Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
# Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>
#
# SPDX-License-Identifier: GPL-3.0-only
# This file is also available under commercial licensing terms.
# Please contact info@sixtyfps.io for more information.
# LICENSE END
cmake_minimum_required(VERSION 3.19)

project(SixtyFPS LANGUAGES CXX)

include(FeatureSummary)

option(BUILD_TESTING "Build tests" OFF)
add_feature_info(BUILD_TESTING BUILD_TESTING "configure whether to build the test suite")
include(CTest)

add_subdirectory(api/sixtyfps-cpp/)

option(SIXTYFPS_BUILD_EXAMPLES "Build SixtyFPS Examples" OFF)
add_feature_info(SIXTYFPS_BUILD_EXAMPLES SIXTYFPS_BUILD_EXAMPLES "configure whether to build the examples")

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
    if (SIXTYFPS_FEATURE_INTERPRETER)
        add_subdirectory(examples/iot-dashboard/)
    endif()
    if(BUILD_TESTING)
        add_subdirectory(docs/tutorial/cpp/src/)
    endif()
endif()

feature_summary(WHAT ENABLED_FEATURES DESCRIPTION "Enabled features:")
feature_summary(WHAT DISABLED_FEATURES DESCRIPTION "Disabled features:")