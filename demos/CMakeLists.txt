# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT
cmake_minimum_required(VERSION 3.21)
project(SlintDemos LANGUAGES CXX)

list(PREPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/../cmake")

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
    include(FetchContent)
endif()

if (TARGET Slint::slint-compiler)
    add_subdirectory(printerdemo/cpp/)
endif()
if (SLINT_FEATURE_INTERPRETER)
    add_subdirectory(printerdemo/cpp_interpreted/)
endif()

