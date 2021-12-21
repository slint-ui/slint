# Copyright © SixtyFPS GmbH <info@sixtyfps.io>
# SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

cmake_minimum_required(VERSION 3.14)

if (NOT TARGET SixtyFPS::SixtyFPS)
    find_package(SixtyFPS REQUIRED)
endif()

add_executable(memory_tutorial_initial main_initial.cpp)
target_link_libraries(memory_tutorial_initial PRIVATE SixtyFPS::SixtyFPS)
sixtyfps_target_60_sources(memory_tutorial_initial memory.60)

add_executable(memory_tutorial_tiles_from_cpp main_tiles_from_cpp.cpp)
target_link_libraries(memory_tutorial_tiles_from_cpp PRIVATE SixtyFPS::SixtyFPS)
sixtyfps_target_60_sources(memory_tutorial_tiles_from_cpp memory_tiles_from_cpp.60)

add_executable(memory_tutorial_game_logic main_game_logic.cpp)
target_link_libraries(memory_tutorial_game_logic PRIVATE SixtyFPS::SixtyFPS)
sixtyfps_target_60_sources(memory_tutorial_game_logic memory_game_logic.60)
