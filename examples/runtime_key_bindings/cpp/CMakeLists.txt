# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(slint_cpp_runtime_key_bindings LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(runtime_key_bindings main.cpp)
target_link_libraries(runtime_key_bindings PRIVATE Slint::Slint)
target_compile_definitions(runtime_key_bindings
    PRIVATE USER_SHORTCUT_CONF_PATH="${CMAKE_CURRENT_SOURCE_DIR}/../user_shortcut.conf")
slint_target_sources(runtime_key_bindings ../ui/main_window.slint)
