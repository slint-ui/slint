# Copyright © SixtyFPS GmbH <info@slint-ui.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

cmake_minimum_required(VERSION 3.23)
project(slint_cpp_printer_demo LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(printerdemo main.cpp)
if (MSVC)
    target_compile_options(printerdemo PRIVATE /bigobj)
endif()
target_link_libraries(printerdemo PRIVATE Slint::Slint)
slint_target_sources(printerdemo ../ui/printerdemo.slint)
