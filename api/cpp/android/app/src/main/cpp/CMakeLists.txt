# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

cmake_minimum_required(VERSION 3.21)
project(SlintApp LANGUAGES CXX)

include(FetchContent)
FetchContent_Declare(
    Slint
    GIT_REPOSITORY https://github.com/slint-ui/slint.git
    GIT_TAG v1.17.0
    SOURCE_SUBDIR api/cpp
)
FetchContent_MakeAvailable(Slint)

add_library(app SHARED main.cpp)
target_link_libraries(app PRIVATE Slint::Slint)
slint_target_sources(app main.slint)
