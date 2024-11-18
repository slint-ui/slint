# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(slint_cpp_printer_demo LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(printerdemo main.cpp)
target_link_libraries(printerdemo PRIVATE Slint::Slint)
slint_target_sources(printerdemo ../ui/printerdemo.slint)

find_package(Intl)
if(Intl_FOUND AND SLINT_FEATURE_GETTEXT)
    target_compile_definitions(printerdemo PRIVATE HAVE_GETTEXT SRC_DIR="${CMAKE_CURRENT_SOURCE_DIR}")
    target_link_libraries(printerdemo PRIVATE Intl::Intl)
else()
    set_property(TARGET printerdemo PROPERTY SLINT_BUNDLE_TRANSLATIONS "${CMAKE_CURRENT_SOURCE_DIR}/../lang")
endif()
