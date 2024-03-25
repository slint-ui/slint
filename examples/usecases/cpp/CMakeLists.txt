# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(slint_cpp_uscases LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(usecases main.cpp)
target_link_libraries(todo PRIVATE Slint::Slint)
slint_target_sources(todo ../ui/app.slint NAMESPACE usecases_ui)
