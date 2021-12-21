# Copyright © SixtyFPS GmbH <info@sixtyfps.io>
# SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

cmake_minimum_required(VERSION 3.14)

if (NOT TARGET SixtyFPS::SixtyFPS)
    find_package(SixtyFPS REQUIRED)
endif()

add_executable(printerdemo_old main.cpp)
target_link_libraries(printerdemo_old PRIVATE SixtyFPS::SixtyFPS)
sixtyfps_target_60_sources(printerdemo_old ../ui/printerdemo.60)
