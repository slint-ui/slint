# Copyright © SixtyFPS GmbH <info@slint-ui.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

cmake_minimum_required(VERSION 3.21)
project(opengl_cpp_texture LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(opengl_texture main.cpp)
target_link_libraries(opengl_texture PRIVATE Slint::Slint OpenGLES2::OpenGLES2)
slint_target_sources(opengl_texture scene.slint)
