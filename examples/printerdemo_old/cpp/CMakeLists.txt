# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(printerdemo_old main.cpp)
target_link_libraries(printerdemo_old PRIVATE Slint::Slint)
slint_target_sources(printerdemo_old ../ui/printerdemo.slint)
