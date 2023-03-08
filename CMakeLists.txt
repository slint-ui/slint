# Copyright © SixtyFPS GmbH <info@slint-ui.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

cmake_minimum_required(VERSION 3.23)

project(Slint LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

include(FeatureSummary)

option(BUILD_TESTING "Build tests" OFF)
add_feature_info(BUILD_TESTING BUILD_TESTING "configure whether to build the test suite")
include(CTest)

# Place all compiled examples into the same bin directory
# on Windows, where we'll also put the dll
if (WIN32)
    set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)
    set(CMAKE_LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)
    set(CMAKE_LIBRARY_OUTPUT_DIRECTORY_DEBUG ${CMAKE_BINARY_DIR}/bin/debug)
    set(CMAKE_LIBRARY_OUTPUT_DIRECTORY_RELEASE ${CMAKE_BINARY_DIR}/bin/release)
endif()

add_subdirectory(api/cpp/)

option(SLINT_BUILD_EXAMPLES "Build Slint Examples" OFF)
add_feature_info(SLINT_BUILD_EXAMPLES SLINT_BUILD_EXAMPLES "configure whether to build the examples")

if(SLINT_BUILD_EXAMPLES)
    add_subdirectory(examples)
    if(BUILD_TESTING)
        add_subdirectory(docs/tutorial/cpp/src/)
    endif()
endif()

feature_summary(WHAT ENABLED_FEATURES DESCRIPTION "Enabled features:")
feature_summary(WHAT DISABLED_FEATURES DESCRIPTION "Disabled features:")
