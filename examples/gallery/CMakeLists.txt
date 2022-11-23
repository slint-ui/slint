# Copyright © SixtyFPS GmbH <info@slint-ui.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

cmake_minimum_required(VERSION 3.14)
project(slint_cpp_gallery LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(gallery main.cpp)
if (MSVC)
    target_compile_options(gallery PRIVATE /bigobj)
endif()
target_link_libraries(gallery PRIVATE Slint::Slint)
slint_target_sources(gallery gallery.slint)
