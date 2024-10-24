# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(slint_cpp_interpreter_printer_demo LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(printerdemo_interpreted main.cpp)
target_compile_definitions(printerdemo_interpreted PRIVATE
   SOURCE_DIR=\"${CMAKE_CURRENT_SOURCE_DIR}\"
)
target_link_libraries(printerdemo_interpreted PRIVATE Slint::Slint)
