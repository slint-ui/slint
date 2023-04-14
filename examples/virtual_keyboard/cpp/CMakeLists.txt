# Copyright © SixtyFPS GmbH <info@slint-ui.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

cmake_minimum_required(VERSION 3.21)
project(slint_cpp_virtual_keyboard LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(virtual_keyboard main.cpp)
target_link_libraries(virtual_keyboard PRIVATE Slint::Slint)
slint_target_sources(virtual_keyboard ../ui/main_window.slint)
