# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(sdl_underlay LANGUAGES CXX)

find_package(Slint REQUIRED)

# SDL3 is required for this example's game rendering code.
# The Slint SDL backend links SDL3 internally, but the game code also calls
# SDL3 directly, so we need the headers and library.
find_package(SDL3 QUIET)
if(NOT SDL3_FOUND)
    find_package(PkgConfig REQUIRED)
    pkg_check_modules(SDL3 REQUIRED IMPORTED_TARGET sdl3)
    add_library(SDL3::SDL3 ALIAS PkgConfig::SDL3)
endif()

add_executable(sdl_underlay main.cpp)
target_link_libraries(sdl_underlay PRIVATE Slint::Slint SDL3::SDL3)
slint_target_sources(sdl_underlay scene.slint)
