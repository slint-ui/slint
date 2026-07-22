# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(slint_cpp_carousel LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(carousel main.cpp)
target_link_libraries(carousel PRIVATE Slint::Slint)
slint_target_sources(carousel ../ui/carousel_demo.slint)
