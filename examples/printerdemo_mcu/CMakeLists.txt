# Copyright © SixtyFPS GmbH <info@slint-ui.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

cmake_minimum_required(VERSION 3.14)

project(slint_cpp_printer_demo_mcu LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(printerdemo_mcu main.cpp)
target_link_libraries(printerdemo_mcu PRIVATE Slint::Slint)
slint_target_sources(printerdemo_mcu ./ui/printerdemo.slint)
