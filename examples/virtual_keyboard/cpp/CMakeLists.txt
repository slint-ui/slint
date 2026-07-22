# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(slint_cpp_virtual_keyboard LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(virtual_keyboard main.cpp)
target_link_libraries(virtual_keyboard PRIVATE Slint::Slint)
slint_target_sources(virtual_keyboard ../ui/main_window.slint)
