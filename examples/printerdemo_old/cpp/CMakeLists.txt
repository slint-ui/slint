# Copyright © SixtyFPS GmbH <info@sixtyfps.io>
# SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

cmake_minimum_required(VERSION 3.14)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(printerdemo_old main.cpp)
target_link_libraries(printerdemo_old PRIVATE Slint::Slint)
slint_target_sources(printerdemo_old ../ui/printerdemo.slint)
