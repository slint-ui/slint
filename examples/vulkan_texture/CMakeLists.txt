# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(vulkan_cpp_texture LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

# The example calls into Vulkan itself, so unlike Slint it does link the loader.
find_package(Vulkan REQUIRED)

add_executable(vulkan_texture main.cpp)
target_link_libraries(vulkan_texture PRIVATE Slint::Slint Vulkan::Vulkan)
# The SPIR-V is read at run time; see the README for how it is built.
target_compile_definitions(vulkan_texture PRIVATE SHADER_DIR="${CMAKE_CURRENT_SOURCE_DIR}")
slint_target_sources(vulkan_texture scene.slint)
