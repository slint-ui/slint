# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.20.0)

if(NOT BOARD)
    message(FATAL_ERROR "No BOARD option specified")
endif()

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
else()
    # See rustc --print target-list for available targets. Match that up with you supported board
    # https://docs.zephyrproject.org/latest/boards
    message(FATAL_ERROR "Unsupported BOARD option specified: ${BOARD}")
endif()

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
project(slint_zephyr_printer_demo_mcu LANGUAGES CXX)

set(SLINT_FEATURE_FREESTANDING ON)
set(SLINT_FEATURE_RENDERER_SOFTWARE ON)
set(DEFAULT_SLINT_EMBED_RESOURCES "embed-for-software-renderer" CACHE STRING "")
set(CMAKE_BUILD_TYPE Release)
set(BUILD_SHARED_LIBS OFF)

# Work around gcc_s linker issue?
set(Rust_CARGO_TARGET_LINK_NATIVE_LIBS "" CACHE INTERNAL "" FORCE)

# TODO: FetchContent?
add_subdirectory(../../.. slint_build)

# The app target is defined by the call to find_package(Zephyr)
target_sources(app PRIVATE ../../zephyr-common/slint-zephyr.cpp main.cpp)
target_include_directories(app PRIVATE ../../zephyr-common)
target_link_libraries(app PRIVATE Slint::Slint)

# Workaround for Slint render_by_line type mismatch on 32-bit ARM
target_compile_options(app PRIVATE -fpermissive)

slint_target_sources(app ../ui/printerdemo.slint)
