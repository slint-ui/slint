# Copyright © SixtyFPS GmbH <info@slint-ui.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

cmake_minimum_required(VERSION 3.23)
project(slint_cpp_todo LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(todo main.cpp)
target_link_libraries(todo PRIVATE Slint::Slint)
slint_target_sources(todo ../ui/todo.slint)
