# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(system-tray LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(system-tray main.cpp)
target_link_libraries(system-tray PRIVATE Slint::Slint)
slint_target_sources(system-tray system-tray.slint)
