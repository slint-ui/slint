# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(memory_tutorial_initial main_initial.cpp)
target_link_libraries(memory_tutorial_initial PRIVATE Slint::Slint)
slint_target_sources(memory_tutorial_initial appwindow.slint)

add_executable(memory_tutorial_tiles_from_cpp main_tiles_from_cpp.cpp)
target_link_libraries(memory_tutorial_tiles_from_cpp PRIVATE Slint::Slint)
slint_target_sources(memory_tutorial_tiles_from_cpp memory_tiles_from_cpp.slint)

add_executable(memory_tutorial_game_logic main_game_logic.cpp)
target_link_libraries(memory_tutorial_game_logic PRIVATE Slint::Slint)
slint_target_sources(memory_tutorial_game_logic memory_game_logic.slint)
