# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(system_tray LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(system_tray main.cpp)
target_link_libraries(system_tray PRIVATE Slint::Slint)
slint_target_sources(system_tray system_tray.slint)
