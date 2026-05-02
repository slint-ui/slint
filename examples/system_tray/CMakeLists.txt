# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.21)
project(system_tray LANGUAGES CXX)

if (NOT TARGET Slint::Slint)
    find_package(Slint REQUIRED)
endif()

# SystemTray is gated behind the experimental feature in slint-cpp.
# Build slint-cpp with `-DSLINT_FEATURE_EXPERIMENTAL=ON` to enable it.
get_target_property(slint_features Slint::Slint SLINT_ENABLED_FEATURES)
if (NOT "EXPERIMENTAL" IN_LIST slint_features)
    message(STATUS "system_tray example: skipped — Slint::Slint was built without SLINT_FEATURE_EXPERIMENTAL")
    return()
endif()

add_executable(system_tray main.cpp)
target_link_libraries(system_tray PRIVATE Slint::Slint)
slint_target_sources(system_tray system_tray.slint)
