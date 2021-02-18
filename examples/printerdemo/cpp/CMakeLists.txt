# LICENSE BEGIN
# This file is part of the SixtyFPS Project -- https://sixtyfps.io
# Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
# Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>
#
# SPDX-License-Identifier: GPL-3.0-only
# This file is also available under commercial licensing terms.
# Please contact info@sixtyfps.io for more information.
# LICENSE END
cmake_minimum_required(VERSION 3.14)
project(sixtyfps_cpptest LANGUAGES CXX)

if (NOT TARGET SixtyFPS::SixtyFPS)
    find_package(SixtyFPS REQUIRED)
endif()

add_executable(printerdemo main.cpp)
target_compile_definitions(printerdemo PRIVATE
   FONTS_DIR=\"${CMAKE_CURRENT_SOURCE_DIR}/../ui/fonts\"
)
target_link_libraries(printerdemo PRIVATE SixtyFPS::SixtyFPS)
sixtyfps_target_60_sources(printerdemo ../ui/printerdemo.60)
