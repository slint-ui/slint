# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(SlintTodo LANGUAGES CXX)

# Use the local Slint source tree
add_subdirectory(${SLINT_SOURCE_DIR} slint_build)

add_library(todo SHARED main.cpp app.cpp)
target_link_libraries(todo PRIVATE Slint::Slint)
# The UI file is shared with the desktop C++ todo example.
# Go up from app/src/main/cpp -> android -> cpp -> todo, then into ui/
cmake_path(SET _todo_ui_dir NORMALIZE "${CMAKE_CURRENT_SOURCE_DIR}/../../../../../../ui")
slint_target_sources(todo "${_todo_ui_dir}/todo.slint" NAMESPACE todo_ui)
