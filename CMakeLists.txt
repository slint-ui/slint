# Copyright © SixtyFPS GmbH <info@slint-ui.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

cmake_minimum_required(VERSION 3.21)

project(Slint LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

include(FeatureSummary)

option(SLINT_BUILD_TESTING "Build tests" OFF)
add_feature_info(SLINT_BUILD_TESTING SLINT_BUILD_TESTING "configure whether to build the test suite")
include(CTest)

set(SLINT_IS_TOPLEVEL_BUILD TRUE)

# Place all compiled examples into the same bin directory
# on Windows, where we'll also put the dll
if (WIN32)
    set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)
    set(CMAKE_LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)
    set(CMAKE_LIBRARY_OUTPUT_DIRECTORY_DEBUG ${CMAKE_BINARY_DIR}/bin/debug)
    set(CMAKE_LIBRARY_OUTPUT_DIRECTORY_RELEASE ${CMAKE_BINARY_DIR}/bin/release)
elseif(APPLE)
    # On macOS, the slint_cpp.dylib's install_name uses @rpath. CMake doesn't set BUILD_RPATH for
    # imported targets though, so include the directory here by hand in the rpath used to build binaries
    # in the build tree (such as our examples or tests).
    set(CMAKE_BUILD_RPATH ${CMAKE_BINARY_DIR}/api/cpp)
endif()

add_subdirectory(api/cpp/)

option(SLINT_BUILD_EXAMPLES "Build Slint Examples" OFF)
add_feature_info(SLINT_BUILD_EXAMPLES SLINT_BUILD_EXAMPLES "configure whether to build the examples")

if(SLINT_BUILD_EXAMPLES)
    add_subdirectory(examples)
endif()
if(SLINT_BUILD_TESTING)
    add_subdirectory(docs/tutorial/cpp/src/)
endif()

feature_summary(WHAT ENABLED_FEATURES DESCRIPTION "Enabled features:")
feature_summary(WHAT DISABLED_FEATURES DESCRIPTION "Disabled features:")
