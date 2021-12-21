# Copyright © SixtyFPS GmbH <info@sixtyfps.io>
# SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

cmake_minimum_required(VERSION 3.19)
project(memory LANGUAGES CXX)

if (NOT TARGET SixtyFPS::SixtyFPS)
    find_package(SixtyFPS REQUIRED)
endif()

# Note, we can't name it simply "memory" because there is a C++ standard header with the same name
# so `#include<memory>` includes the generated binary when it exists
add_executable(memory_game memory.cpp)
target_link_libraries(memory_game PRIVATE SixtyFPS::SixtyFPS)
sixtyfps_target_60_sources(memory_game memory.60)
