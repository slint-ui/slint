# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(skia_game_viewport LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

add_executable(skia_game_viewport main.cpp)
target_link_libraries(skia_game_viewport PRIVATE Slint::Slint)
slint_target_sources(skia_game_viewport scene.slint)

# The Skia headers are needed for SkCanvas, SkPaint, etc.
# For standalone builds, set SKIA_INCLUDE_DIR to your Skia installation.
if (DEFINED SKIA_INCLUDE_DIR)
    target_include_directories(skia_game_viewport PRIVATE ${SKIA_INCLUDE_DIR})
endif()

# Link against the Skia static library built by the skia-bindings crate.
# Auto-detect from the cargo build output, or set SKIA_LIBRARY manually.
if (NOT DEFINED SKIA_LIBRARY)
    file(GLOB _skia_lib "${CMAKE_BINARY_DIR}/cargo/*/x86_64-unknown-linux-gnu/release/build/skia-bindings-*/out/skia/libskia.a")
    if (_skia_lib)
        list(GET _skia_lib 0 SKIA_LIBRARY)
        message(STATUS "Auto-detected SKIA_LIBRARY: ${SKIA_LIBRARY}")
    endif()
endif()

if (DEFINED SKIA_LIBRARY)
    # Skia's static library depends on system libraries for font rendering, GL, etc.
    find_package(PkgConfig REQUIRED)
    pkg_check_modules(FREETYPE REQUIRED freetype2)
    pkg_check_modules(FONTCONFIG REQUIRED fontconfig)
    target_link_libraries(skia_game_viewport PRIVATE
        ${SKIA_LIBRARY}
        ${FREETYPE_LIBRARIES}
        ${FONTCONFIG_LIBRARIES}
    )
    target_link_directories(skia_game_viewport PRIVATE
        ${FREETYPE_LIBRARY_DIRS}
        ${FONTCONFIG_LIBRARY_DIRS}
    )
endif()

# Optional gamepad support via SDL2
option(ENABLE_GAMEPAD "Enable gamepad input via SDL2" OFF)
if (ENABLE_GAMEPAD)
    find_package(PkgConfig REQUIRED)
    pkg_check_modules(SDL2 REQUIRED sdl2)
    target_compile_definitions(skia_game_viewport PRIVATE HAS_GAMEPAD)
    target_include_directories(skia_game_viewport PRIVATE ${SDL2_INCLUDE_DIRS})
    target_link_libraries(skia_game_viewport PRIVATE ${SDL2_LIBRARIES})
    target_link_directories(skia_game_viewport PRIVATE ${SDL2_LIBRARY_DIRS})
endif()
