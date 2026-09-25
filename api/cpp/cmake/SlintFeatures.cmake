# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# The SLINT_FEATURE_* options and the cargo features they select. Kept free of any
# other dependency so api/cpp/tests/check_linuxkms_features.sh can configure it on its own.

include(CMakeDependentOption)
include(FeatureSummary)

function(define_cargo_feature cargo_feature description default)
    # turn foo-bar into SLINT_FEATURE_FOO_BAR
    string(TOUPPER "${cargo_feature}" cmake_option)
    string(REPLACE "-" "_" cmake_option "${cmake_option}")
    list(APPEND public_cmake_features ${cmake_option})
    set(cmake_option "SLINT_FEATURE_${cmake_option}")
    option("${cmake_option}" "${description}" ${default})

    if(${cmake_option})
        list(APPEND features ${cargo_feature})
    endif()

    set(features "${features}" PARENT_SCOPE)
    set(public_cmake_features "${public_cmake_features}" PARENT_SCOPE)
    add_feature_info(${cmake_option} ${cmake_option} ${description})
endfunction()

function(define_cargo_dependent_feature cargo_feature description default depends_condition)
    # turn foo-bar into SLINT_FEATURE_FOO_BAR
    string(TOUPPER "${cargo_feature}" cmake_option)
    string(REPLACE "-" "_" cmake_option "${cmake_option}")
    list(APPEND public_cmake_features ${cmake_option})
    set(cmake_option "SLINT_FEATURE_${cmake_option}")
    cmake_dependent_option("${cmake_option}" "${description}" ${default} ${depends_condition} OFF)
    # cmake_dependent_option forces the value off in this scope only; make it the effective one everywhere.
    set(${cmake_option} "${${cmake_option}}" PARENT_SCOPE)

    if(${cmake_option})
        list(APPEND features ${cargo_feature})
    endif()

    set(features "${features}" PARENT_SCOPE)
    set(public_cmake_features "${public_cmake_features}" PARENT_SCOPE)
    add_feature_info(${cmake_option} ${cmake_option} ${description})
endfunction()

# Features that are mapped to features in the Rust crate. These and their
# defaults need to be kept in sync with the Rust bit (cpp/Cargo.toml and cbindgen.rs)

define_cargo_feature(freestanding "Enable use of freestanding environment. This is only for bare-metal systems. Most other features are incompatible with this one" OFF)

# Compat options (must be declared after the STD feature, but before the options they replace)
function(define_compat_option deprecated replacement)
    cmake_dependent_option("SLINT_FEATURE_${deprecated}" "Compat option equivalent to SLINT_FEATURE_${replacement}" OFF "NOT SLINT_FEATURE_FREESTANDING" OFF)
    if(SLINT_FEATURE_${deprecated})
        set("SLINT_FEATURE_${replacement}" ON PARENT_SCOPE)
        message("SLINT_FEATURE_${deprecated} is deprecated, use SLINT_FEATURE_${replacement} instead")
    endif()
endfunction()
define_compat_option(RENDERER_WINIT_FEMTOVG RENDERER_FEMTOVG)
define_compat_option(RENDERER_WINIT_SKIA RENDERER_SKIA)
define_compat_option(RENDERER_WINIT_SKIA_OPENGL RENDERER_SKIA_OPENGL)
define_compat_option(RENDERER_WINIT_SKIA_VULKAN RENDERER_SKIA_VULKAN)
define_compat_option(RENDERER_WINIT_SOFTWARE RENDERER_SOFTWARE)

define_cargo_dependent_feature(interpreter "Enable support for the Slint interpreter to load .slint files at run-time" ON "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_dependent_feature(live-preview "Enable support for the Slint live-preview to re-load changed .slint files at run-time" OFF "SLINT_FEATURE_INTERPRETER")

define_cargo_dependent_feature(backend-winit "Enable support for the winit crate to interaction with all windowing systems." ON "NOT SLINT_FEATURE_FREESTANDING AND NOT ANDROID")
define_cargo_dependent_feature(backend-winit-x11 "Enable support for the winit create to interact only with the X11 windowing system on Unix. Enable this option and turn off SLINT_FEATURE_BACKEND_WINIT for a smaller build with just X11 support on Unix." OFF "NOT SLINT_FEATURE_FREESTANDING AND NOT ANDROID")
define_cargo_dependent_feature(backend-winit-wayland "Enable support for the winit create to interact only with the wayland windowing system on Unix. Enable this option and turn off SLINT_FEATURE_BACKEND_WINIT for a smaller build with just wayland support." OFF "NOT SLINT_FEATURE_FREESTANDING AND NOT ANDROID")

define_cargo_dependent_feature(renderer-femtovg "Enable support for the OpenGL ES 2.0 based FemtoVG rendering engine." ON "NOT SLINT_FEATURE_FREESTANDING AND NOT ANDROID")
define_cargo_dependent_feature(renderer-femtovg-wgpu "Enable support for the WGPU based FemtoVG rendering engine." OFF "NOT SLINT_FEATURE_FREESTANDING AND NOT ANDROID")
if(ANDROID)
    set(_slint_default_renderer_skia ON)
else()
    set(_slint_default_renderer_skia OFF)
endif()
define_cargo_dependent_feature(renderer-skia "Enable support for the Skia based rendering engine." ${_slint_default_renderer_skia} "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_dependent_feature(renderer-skia-opengl "Enable support for the Skia based rendering engine with its OpenGL backend." OFF "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_dependent_feature(renderer-skia-vulkan "Enable support for the Skia based rendering engine with its Vulkan backend." OFF "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_dependent_feature(renderer-vello "Enable support for the WGPU based vello rendering engine. Experimental: it is never selected automatically, set SLINT_BACKEND=winit-vello or SLINT_BACKEND=linuxkms-vello to use it." OFF "NOT SLINT_FEATURE_FREESTANDING AND NOT ANDROID")
define_cargo_feature(renderer-software "Enable support for the software renderer" ON)
define_cargo_feature(software-renderer-path "Enable support for Path element rendering with the software renderer. This is implicitly enabled when SLINT_FEATURE_FREESTANDING is OFF. Enable this in bare-metal environments if you need support for Path elements" OFF)

define_cargo_dependent_feature(backend-qt "Enable Qt based rendering backend" OFF "NOT SLINT_FEATURE_FREESTANDING AND NOT ANDROID")

# Declared before the capabilities: it provides their default.
define_cargo_dependent_feature(backend-linuxkms "Enable support for the backend that renders a single window fullscreen on Linux. Turns SLINT_FEATURE_BACKEND_LINUXKMS_LIBSEAT and SLINT_FEATURE_BACKEND_LINUXKMS_LIBINPUT on by default; turn either off to build without that library" OFF "NOT SLINT_FEATURE_FREESTANDING")
if(SLINT_FEATURE_BACKEND_LINUXKMS)
    set(_slint_default_backend_linuxkms_capability ON)
else()
    set(_slint_default_backend_linuxkms_capability OFF)
endif()
define_cargo_dependent_feature(backend-linuxkms-libseat "Add libseat to the LinuxKMS backend, for GPU and input device access without root privileges. Enables the backend on its own and combines with SLINT_FEATURE_BACKEND_LINUXKMS_LIBINPUT" ${_slint_default_backend_linuxkms_capability} "NOT SLINT_FEATURE_FREESTANDING")
define_compat_option(BACKEND_LINUXKMS_NOSEAT BACKEND_LINUXKMS_LIBINPUT)
define_cargo_dependent_feature(backend-linuxkms-libinput "Add libinput to the LinuxKMS backend, to react to mouse, touch and keyboard input. Enables the backend on its own and combines with SLINT_FEATURE_BACKEND_LINUXKMS_LIBSEAT" ${_slint_default_backend_linuxkms_capability} "NOT SLINT_FEATURE_FREESTANDING")

if(SLINT_FEATURE_BACKEND_LINUXKMS OR SLINT_FEATURE_BACKEND_LINUXKMS_LIBSEAT OR SLINT_FEATURE_BACKEND_LINUXKMS_LIBINPUT)
    message(STATUS "LinuxKMS backend: libseat=${SLINT_FEATURE_BACKEND_LINUXKMS_LIBSEAT}, libinput=${SLINT_FEATURE_BACKEND_LINUXKMS_LIBINPUT}")
    if(NOT SLINT_FEATURE_BACKEND_LINUXKMS_LIBSEAT)
        message(STATUS "  Without libseat the application must run privileged, typically as root; turn SLINT_FEATURE_BACKEND_LINUXKMS_LIBSEAT on for unprivileged access.")
    endif()
    if(NOT SLINT_FEATURE_BACKEND_LINUXKMS_LIBINPUT)
        message(STATUS "  Without libinput the application receives no input events; turn SLINT_FEATURE_BACKEND_LINUXKMS_LIBINPUT on for mouse, touch and keyboard input.")
    endif()
endif()

define_cargo_dependent_feature(gettext "Enable support of translations using gettext" OFF "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_dependent_feature(accessibility "Enable integration with operating system provided accessibility APIs" ON "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_dependent_feature(system-tray "Enable support for the SystemTrayIcon element" ON "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_dependent_feature(testing "Enable support for testing API (experimental)" ON "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_feature(experimental "Enable experimental features. (No backward compatibility guarantees)" OFF)
define_cargo_dependent_feature(system-testing "Enable support for controlling the application from a system testing tool" OFF "NOT SLINT_FEATURE_FREESTANDING")
define_cargo_dependent_feature(mcp "Enable the embedded MCP server for AI-assisted debugging" OFF "NOT SLINT_FEATURE_FREESTANDING")
