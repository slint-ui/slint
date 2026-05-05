# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(SlintTodo LANGUAGES CXX)

# Use the local Slint source tree
add_subdirectory(${SLINT_SOURCE_DIR} slint_build)

# Shared source directory: app/src/main/cpp -> android -> cpp (the desktop example)
cmake_path(SET _todo_cpp_dir NORMALIZE "${CMAKE_CURRENT_SOURCE_DIR}/../../../..")

add_library(todo_lib STATIC "${_todo_cpp_dir}/app.cpp")
slint_target_sources(todo_lib "${_todo_cpp_dir}/../ui/todo.slint" NAMESPACE todo_ui)
target_link_libraries(todo_lib PUBLIC Slint::Slint)

add_library(todo SHARED "${_todo_cpp_dir}/main.cpp")
target_link_libraries(todo PRIVATE todo_lib)
