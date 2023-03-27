# Copyright © SixtyFPS GmbH <info@slint-ui.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

cmake_minimum_required(VERSION 3.21)
project(slint_cpp_interpreter_printer_demo LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(printerdemo_interpreted main.cpp)
target_compile_definitions(printerdemo_interpreted PRIVATE
   SOURCE_DIR=\"${CMAKE_CURRENT_SOURCE_DIR}\"
)
if (MSVC)
    target_compile_options(printerdemo_interpreted PRIVATE /bigobj)
endif()
target_link_libraries(printerdemo_interpreted PRIVATE Slint::Slint)
