# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.20.0)

if(NOT BOARD)
    message(FATAL_ERROR "No BOARD option specified")
endif()

# Slint UI entry point to build, relative to ../ui, and its window scale
# factor. Each board branch below sets its own defaults; override with
# -DSLINT_HOME_AUTOMATION_UI=... / -DSLINT_ZEPHYR_SCALE_FACTOR=... to try a
# different UI or scale on a given board.
set(SLINT_HOME_AUTOMATION_UI_DEFAULT "demo-sw-renderer.slint")
set(SLINT_ZEPHYR_SCALE_FACTOR_DEFAULT "")

if(BOARD STREQUAL "native_sim/native/64")
    set(Rust_CARGO_TARGET "x86_64-unknown-linux-gnu")
    set(SLINT_LIBRARY_CARGO_FLAGS "-Zbuild-std=core,alloc")
    set(BOARD_CONF_NAME "native_sim_64")
elseif(BOARD STREQUAL "mimxrt1170_evk@B/mimxrt1176/cm7")
    set(Rust_CARGO_TARGET "thumbv7em-none-eabihf")
    set(BOARD_CONF_NAME "mimxrt1170_evk_mimxrt1176_cm7")
elseif(BOARD STREQUAL "frdm_mcxn947/mcxn947/cpu0")
    set(Rust_CARGO_TARGET "thumbv8m.main-none-eabihf")
    set(SLINT_LIBRARY_CARGO_FLAGS "-Zbuild-std=core,alloc")
    set(BOARD_CONF_NAME "frdm_mcxn947_mcxn947_cpu0")
    # main-480x320.slint lays out a 960x640 logical UI for this board's 480x320
    # physical panel; the 0.5 scale factor is required, not optional.
    set(SLINT_HOME_AUTOMATION_UI_DEFAULT "main-480x320.slint")
    set(SLINT_ZEPHYR_SCALE_FACTOR_DEFAULT "0.5")
else()
    # See rustc --print target-list for available targets. Match that up with you supported board
    # https://docs.zephyrproject.org/latest/boards
    message(FATAL_ERROR "Unsupported BOARD option specified: ${BOARD}")
endif()

set(SLINT_HOME_AUTOMATION_UI "${SLINT_HOME_AUTOMATION_UI_DEFAULT}" CACHE STRING
    "Slint UI entry point to build (relative to ../ui)")

# Use shared Zephyr configuration from zephyr-common
set(ZEPHYR_COMMON_DIR ${CMAKE_CURRENT_SOURCE_DIR}/../../zephyr-common)
set(CONF_FILE ${ZEPHYR_COMMON_DIR}/prj.conf)
if(EXISTS ${ZEPHYR_COMMON_DIR}/boards/${BOARD_CONF_NAME}.overlay)
    set(DTC_OVERLAY_FILE ${ZEPHYR_COMMON_DIR}/boards/${BOARD_CONF_NAME}.overlay)
endif()
if(EXISTS ${ZEPHYR_COMMON_DIR}/boards/${BOARD_CONF_NAME}.conf)
    set(EXTRA_CONF_FILE ${ZEPHYR_COMMON_DIR}/boards/${BOARD_CONF_NAME}.conf)
endif()

find_package(Zephyr)
project(slint_zephyr_home_automation LANGUAGES CXX)

set(SLINT_FEATURE_FREESTANDING ON CACHE BOOL "" FORCE)
set(SLINT_FEATURE_RENDERER_SOFTWARE ON CACHE BOOL "" FORCE)
set(SLINT_FEATURE_SDF_FONTS ON CACHE BOOL "" FORCE)
set(DEFAULT_SLINT_EMBED_RESOURCES "embed-for-software-renderer-with-sdf" CACHE STRING "")
set(CMAKE_BUILD_TYPE Release)
set(BUILD_SHARED_LIBS OFF)

# TODO: FetchContent?
add_subdirectory(../../.. slint_build)

# The app target is defined by the call to find_package(Zephyr)
target_sources(app PRIVATE ../../zephyr-common/slint-zephyr.cpp main.cpp)
target_include_directories(app PRIVATE ../../zephyr-common)
target_link_libraries(app PRIVATE Slint::Slint)

slint_target_sources(app ../ui/${SLINT_HOME_AUTOMATION_UI})

get_filename_component(SLINT_HOME_AUTOMATION_UI_NAME ${SLINT_HOME_AUTOMATION_UI} NAME_WE)
target_compile_definitions(app PRIVATE
    SLINT_HOME_AUTOMATION_UI_HEADER="${SLINT_HOME_AUTOMATION_UI_NAME}.h")

# Optionally render a larger logical UI scaled down to the physical display,
# e.g. -DSLINT_ZEPHYR_SCALE_FACTOR=0.5 shows a 960x640 UI on a 480x320 panel.
# The same factor is passed to the slint compiler so that embedded images are
# downscaled at compile time instead of being embedded at full resolution.
set(SLINT_ZEPHYR_SCALE_FACTOR "${SLINT_ZEPHYR_SCALE_FACTOR_DEFAULT}" CACHE STRING
    "Slint window scale factor override")
if(SLINT_ZEPHYR_SCALE_FACTOR)
    target_compile_definitions(app PRIVATE
        SLINT_ZEPHYR_SCALE_FACTOR=${SLINT_ZEPHYR_SCALE_FACTOR}f)
    set_target_properties(app PROPERTIES SLINT_SCALE_FACTOR ${SLINT_ZEPHYR_SCALE_FACTOR})
endif()

option(SLINT_ZEPHYR_HEAP_PROBE "Log malloc headroom periodically (for measurements)" OFF)
if(SLINT_ZEPHYR_HEAP_PROBE)
    target_compile_definitions(app PRIVATE SLINT_ZEPHYR_HEAP_PROBE)
endif()

# Place the line render buffer in the unused SRAMX region instead of the main
# SRAM, freeing that space for the malloc arena. Opt-in: requires SRAMX to be
# reachable by the display write path on the target.
option(SLINT_ZEPHYR_RENDER_BUFFER_SRAMX "Place the render buffer in SRAMX" OFF)
if(SLINT_ZEPHYR_RENDER_BUFFER_SRAMX)
    target_compile_definitions(app PRIVATE SLINT_ZEPHYR_RENDER_BUFFER_SRAMX)
endif()
