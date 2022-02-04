# Copyright © SixtyFPS GmbH <info@sixtyfps.io>
# SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

cmake_minimum_required(VERSION 3.14)
project(opengl_cpp_underlay LANGUAGES CXX)

if (NOT TARGET SixtyFPS::SixtyFPS)
    find_package(SixtyFPS REQUIRED)
endif()

add_executable(opengl_underlay main.cpp)
target_link_libraries(opengl_underlay PRIVATE SixtyFPS::SixtyFPS OpenGLES2::OpenGLES2)
sixtyfps_target_60_sources(opengl_underlay scene.60)
