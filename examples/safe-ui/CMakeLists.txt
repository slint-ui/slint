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

set(Rust_CARGO_TARGET_LINK_NATIVE_LIBS "" CACHE INTERNAL "")

corrosion_import_crate(MANIFEST_PATH "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml"
    CRATES slint-safeui CRATE_TYPES staticlib)

set_property(
    TARGET slint_safeui_lib
    PROPERTY CORROSION_NO_DEFAULT_FEATURES
    ON
)

add_library(SlintSafeUi INTERFACE)
target_link_libraries(SlintSafeUi INTERFACE slint_safeui_lib)

target_include_directories(SlintSafeUi INTERFACE
    $<BUILD_INTERFACE:${CMAKE_CURRENT_BINARY_DIR}/src>
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/src>
)
