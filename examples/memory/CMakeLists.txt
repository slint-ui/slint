# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(memory LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

# Note, we can't name it simply "memory" because there is a C++ standard header with the same name
# so `#include<memory>` includes the generated binary when it exists
add_executable(memory_game memory.cpp)
target_link_libraries(memory_game PRIVATE Slint::Slint)
slint_target_sources(memory_game memory.slint)
