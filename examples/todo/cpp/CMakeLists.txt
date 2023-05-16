# Copyright © SixtyFPS GmbH <info@slint-ui.com>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(slint_cpp_todo LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(todo main.cpp)
target_link_libraries(todo PRIVATE Slint::Slint)
slint_target_sources(todo ../ui/todo.slint)
