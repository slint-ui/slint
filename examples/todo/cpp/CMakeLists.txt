# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(slint_cpp_todo LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_library(todo_lib STATIC app.cpp)
slint_target_sources(todo_lib ../ui/todo.slint NAMESPACE todo_ui)
target_link_libraries(todo_lib PUBLIC Slint::Slint)

add_executable(todo main.cpp)
target_link_libraries(todo PRIVATE todo_lib)

if(SLINT_BUILD_TESTING AND SLINT_FEATURE_TESTING AND SLINT_FEATURE_EXPERIMENTAL)
    add_executable(test_todo_basic tests/test_todo_basic.cpp)
    target_link_libraries(test_todo_basic PRIVATE Catch2::Catch2WithMain todo_lib)
    add_test(NAME test_todo_basic COMMAND test_todo_basic)
endif()
