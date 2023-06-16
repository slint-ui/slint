# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(slint_cpp_printer_demo LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(printerdemo main.cpp)
target_link_libraries(printerdemo PRIVATE Slint::Slint)
slint_target_sources(printerdemo ../ui/printerdemo.slint)
