# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(slint_cpp_home_automation LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(home-automation main.cpp)
target_link_libraries(home-automation PRIVATE Slint::Slint)
slint_target_sources(home-automation ../ui/demo.slint COMPILATION_UNITS 20)
set_target_properties(home-automation PROPERTIES SLINT_EMBED_RESOURCES "embed-for-software-renderer-with-sdf")
target_compile_options(home-automation PRIVATE -fno-exceptions)
target_compile_features(home-automation PRIVATE cxx_std_23)