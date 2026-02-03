# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(SlintSafeUI LANGUAGES C CXX VERSION 1.0)

if(NOT DEFINED SLINT_SAFEUI_WIDTH)
    set(SLINT_SAFEUI_WIDTH "" CACHE STRING "SafeUI window width (optional)")
endif()

if(NOT DEFINED SLINT_SAFEUI_HEIGHT)
    set(SLINT_SAFEUI_HEIGHT "" CACHE STRING "SafeUI window height (optional)")
endif()

# Use pixel-bgra8888 as default pixel format
set(SLINT_SAFEUI_PIXEL_FORMAT "pixel-bgra8888" CACHE STRING "Pixel format for Slint SafeUI")
set_property(CACHE SLINT_SAFEUI_PIXEL_FORMAT PROPERTY STRINGS
    "pixel-bgra8888"
    "pixel-rgb565"
    "pixel-rgb888"
)

# Enable default SafeUI panic handler by default
option(SLINT_SAFEUI_PANIC_HANDLER "Enable default SafeUI Panic Handler" ON)

include(FetchContent)
FetchContent_Declare(
    Corrosion
    GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
    GIT_TAG v0.6.0
)
FetchContent_MakeAvailable(Corrosion)

set(SLINT_SAFEUI_FEATURES "${SLINT_SAFEUI_PIXEL_FORMAT}")
if(SLINT_SAFEUI_PANIC_HANDLER)
    list(APPEND SLINT_SAFEUI_FEATURES "panic-handler")
endif()

message(STATUS "Active Slint SafeUI Features: ${SLINT_SAFEUI_FEATURES}")

corrosion_import_crate(
    MANIFEST_PATH "${CMAKE_CURRENT_SOURCE_DIR}/core/Cargo.toml"
    CRATES slint-safeui-core
    CRATE_TYPES staticlib
    FEATURES "${SLINT_SAFEUI_FEATURES}"
)

if(SLINT_SAFEUI_WIDTH AND SLINT_SAFEUI_HEIGHT)
    message(STATUS "Custom Slint SafeUI Window Size: ${SLINT_SAFEUI_WIDTH}x${SLINT_SAFEUI_HEIGHT}")
    corrosion_set_env_vars(
        slint_safeui_core
        SAFE_UI_WIDTH=${SLINT_SAFEUI_WIDTH}
        SAFE_UI_HEIGHT=${SLINT_SAFEUI_HEIGHT}
    )
endif()

add_library(SlintSafeUi INTERFACE)
target_link_libraries(SlintSafeUi INTERFACE slint_safeui_core)

target_include_directories(SlintSafeUi INTERFACE
    $<BUILD_INTERFACE:${CMAKE_CURRENT_BINARY_DIR}/core/src>
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/core/src>
)
