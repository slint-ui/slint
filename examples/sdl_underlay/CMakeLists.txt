# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(sdl_underlay LANGUAGES CXX)

find_package(Slint REQUIRED)

# SDL3 is required for this example's game rendering code.
# The Slint SDL backend links SDL3 internally, but the game code also calls
# SDL3 directly, so we need the headers and library.
find_package(PkgConfig REQUIRED)
pkg_check_modules(SDL3 REQUIRED IMPORTED_TARGET sdl3)
pkg_check_modules(SDL3_TTF REQUIRED IMPORTED_TARGET sdl3-ttf)
pkg_check_modules(FONTCONFIG REQUIRED IMPORTED_TARGET fontconfig)

add_executable(sdl_underlay main.cpp)
target_link_libraries(sdl_underlay PRIVATE Slint::Slint PkgConfig::SDL3 PkgConfig::SDL3_TTF PkgConfig::FONTCONFIG)
slint_target_sources(sdl_underlay scene.slint)

add_executable(sdl_underlay_live main_interpreter.cpp)
target_link_libraries(sdl_underlay_live PRIVATE Slint::Slint PkgConfig::SDL3 PkgConfig::SDL3_TTF PkgConfig::FONTCONFIG)
target_compile_features(sdl_underlay_live PRIVATE cxx_std_20)
