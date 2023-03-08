# Copyright © SixtyFPS GmbH <info@slint-ui.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

cmake_minimum_required(VERSION 3.23)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(memory_tutorial_initial main_initial.cpp)
target_link_libraries(memory_tutorial_initial PRIVATE Slint::Slint)
slint_target_sources(memory_tutorial_initial memory.slint)

add_executable(memory_tutorial_tiles_from_cpp main_tiles_from_cpp.cpp)
target_link_libraries(memory_tutorial_tiles_from_cpp PRIVATE Slint::Slint)
slint_target_sources(memory_tutorial_tiles_from_cpp memory_tiles_from_cpp.slint)

add_executable(memory_tutorial_game_logic main_game_logic.cpp)
target_link_libraries(memory_tutorial_game_logic PRIVATE Slint::Slint)
slint_target_sources(memory_tutorial_game_logic memory_game_logic.slint)
