# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(dnd-kanban LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(dnd-kanban kanban.cpp)
target_link_libraries(dnd-kanban PRIVATE Slint::Slint)
slint_target_sources(dnd-kanban kanban.slint)
