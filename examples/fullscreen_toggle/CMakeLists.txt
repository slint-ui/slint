# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(fullscreen_toggle LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(fullscreen_toggle fullscreen_toggle.cpp)
target_link_libraries(fullscreen_toggle PRIVATE Slint::Slint)
slint_target_sources(fullscreen_toggle fullscreen_toggle.slint)
