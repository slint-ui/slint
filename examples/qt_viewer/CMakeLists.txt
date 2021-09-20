# LICENSE BEGIN
# This file is part of the SixtyFPS Project -- https://sixtyfps.io
# Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
# Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>
#
# SPDX-License-Identifier: GPL-3.0-only
# This file is also available under commercial licensing terms.
# Please contact info@sixtyfps.io for more information.
# LICENSE END
cmake_minimum_required(VERSION 3.14)
project(qt_viewer LANGUAGES CXX)

if (NOT TARGET SixtyFPS::SixtyFPS)
    find_package(SixtyFPS REQUIRED)
endif()

find_package(Qt6 6.0 COMPONENTS Core Widgets)
if (NOT TARGET Qt::Widgets)
    find_package(Qt5 5.15 COMPONENTS Core Widgets)
endif()

if (TARGET Qt::Widgets)
    set(CMAKE_AUTOUIC ON)
    add_executable(qt_viewer qt_viewer.cpp)
    target_link_libraries(qt_viewer PRIVATE SixtyFPS::SixtyFPS Qt::Core Qt::Widgets)
endif()
