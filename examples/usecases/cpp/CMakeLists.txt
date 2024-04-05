# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(slint_cpp_usecases LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

set(SLINT_STYLE "cosmic-light" CACHE STRING "Style for demo" FORCE)

add_executable(usecases main.cpp)
target_link_libraries(usecases PRIVATE Slint::Slint)
slint_target_sources(usecases ../ui/app.slint)
