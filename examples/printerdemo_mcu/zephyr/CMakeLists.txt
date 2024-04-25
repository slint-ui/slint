cmake_minimum_required(VERSION 3.20.0)

find_package(Zephyr)
project(slint_zephyr_printer_demo_mcu LANGUAGES CXX)

set(SLINT_FEATURE_FREESTANDING ON)
set(SLINT_FEATURE_RENDERER_SOFTWARE ON)
set(SLINT_LIBRARY_CARGO_FLAGS "-Zbuild-std=core,alloc")
set(DEFAULT_SLINT_EMBED_RESOURCES "embed-for-software-renderer" CACHE STRING "")
# set(CMAKE_BUILD_TYPE Release)
set(BUILD_SHARED_LIBS OFF)

# Only correct for -DBOARD=native_sim/native/64
# See rustc --print target-list for available targets
set(Rust_CARGO_TARGET "x86_64-unknown-linux-gnu")

# Work around gcc_s linker issue?
set(Rust_CARGO_TARGET_LINK_NATIVE_LIBS "" CACHE INTERNAL "" FORCE)

# TODO: FetchContent?
add_subdirectory(../../.. slint_build)

# The app target is defined by the call to find_package(Zephyr)
target_sources(app PRIVATE src/slint-zephyr.cpp src/main.cpp)
target_link_libraries(app PRIVATE Slint::Slint)

slint_target_sources(app ../ui/printerdemo.slint)
