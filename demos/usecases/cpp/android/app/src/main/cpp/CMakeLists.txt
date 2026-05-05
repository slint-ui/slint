# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(SlintUsecases LANGUAGES CXX)

# Use the local Slint source tree
add_subdirectory(${SLINT_SOURCE_DIR} slint_build)

# Shared source directory: app/src/main/cpp -> android -> cpp (the desktop example)
cmake_path(SET _usecases_cpp_dir NORMALIZE "${CMAKE_CURRENT_SOURCE_DIR}/../../../..")

add_library(usecases SHARED "${_usecases_cpp_dir}/main.cpp")
target_link_libraries(usecases PRIVATE Slint::Slint)
slint_target_sources(usecases "${_usecases_cpp_dir}/../ui/app.slint")
