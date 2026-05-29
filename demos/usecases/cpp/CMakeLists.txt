# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(slint_cpp_usecases LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    # When built standalone (e.g. when the Android Gradle build invokes CMake on this file directly),
    # use Slint from the surrounding source tree if it is available there; otherwise require an install.
    if(EXISTS "${CMAKE_CURRENT_LIST_DIR}/../../../api/cpp/CMakeLists.txt")
        add_subdirectory("${CMAKE_CURRENT_LIST_DIR}/../../../api/cpp" _slint)
    else()
        find_package(Slint REQUIRED)
    endif()
endif()

if(NOT SLINT_STYLE)
    set(SLINT_STYLE "cosmic-light" CACHE STRING "Style for demo" FORCE)
endif()

if(ANDROID)
    add_library(usecases SHARED main.cpp)
else()
    add_executable(usecases main.cpp)
endif()
target_link_libraries(usecases PRIVATE Slint::Slint)
slint_target_sources(usecases ../ui/app.slint)
