# Copyright © SixtyFPS GmbH <info@sixtyfps.io>
# SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

cmake_minimum_required(VERSION 3.19)

project(SixtyFPS LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

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
    add_subdirectory(examples)
    if(BUILD_TESTING)
        add_subdirectory(docs/tutorial/cpp/src/)
    endif()
endif()

feature_summary(WHAT ENABLED_FEATURES DESCRIPTION "Enabled features:")
feature_summary(WHAT DISABLED_FEATURES DESCRIPTION "Disabled features:")
