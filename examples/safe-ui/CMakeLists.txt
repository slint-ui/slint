# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(SlintSafeUI LANGUAGES C CXX VERSION 1.0)

include(FetchContent)
FetchContent_Declare(
    Corrosion
    GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
    GIT_TAG v0.6.0
)
FetchContent_MakeAvailable(Corrosion)

corrosion_import_crate(
    MANIFEST_PATH "${CMAKE_CURRENT_SOURCE_DIR}/core/Cargo.toml"
    CRATES slint-safeui-core
    CRATE_TYPES staticlib
)

add_library(SlintSafeUi INTERFACE)
target_link_libraries(SlintSafeUi INTERFACE slint_safeui_core)

target_include_directories(SlintSafeUi INTERFACE
    $<BUILD_INTERFACE:${CMAKE_CURRENT_BINARY_DIR}/core/src>
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/core/src>
)
