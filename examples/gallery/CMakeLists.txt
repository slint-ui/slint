# Copyright © SixtyFPS GmbH <info@slint-ui.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

cmake_minimum_required(VERSION 3.21)
project(slint_cpp_gallery LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()


add_executable(gallery main.cpp)
target_link_libraries(gallery PRIVATE Slint::Slint)
slint_target_sources(gallery gallery.slint)

find_package(Intl)
if(Intl_FOUND)
    target_compile_definitions(gallery PRIVATE HAVE_GETTEXT SRC_DIR="${CMAKE_CURRENT_SOURCE_DIR}")
    target_link_libraries(gallery PRIVATE Intl::Intl)
endif()

