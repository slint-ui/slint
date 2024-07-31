# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

set(SLINT_FEATURE_FREESTANDING ON)
set(SLINT_FEATURE_RENDERER_SOFTWARE ON)
# set(SLINT_LIBRARY_CARGO_FLAGS "-Zbuild-std=core,alloc")
set(DEFAULT_SLINT_EMBED_RESOURCES "embed-for-software-renderer" CACHE STRING "")
set(CMAKE_BUILD_TYPE Release)
set(BUILD_SHARED_LIBS OFF)
set(Rust_CARGO_TARGET "thumbv7em-none-eabihf")

add_subdirectory(.. slint_build)

set(header_content
    "#include <${SLINT_STM32_BSP_NAME}.h>
#include <${SLINT_STM32_BSP_NAME}_lcd.h>
#include <${SLINT_STM32_BSP_NAME}_ts.h>
")

add_library(SlintSTM32 INTERFACE)
file(GENERATE OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/slint-stm-config.h" CONTENT "${header_content}")
target_include_directories(SlintSTM32 INTERFACE "${CMAKE_CURRENT_BINARY_DIR}" "${CMAKE_CURRENT_SOURCE_DIR}")
target_sources(SlintSTM32 INTERFACE slint-stm.cpp)
target_link_libraries(SlintSTM32 INTERFACE Slint::Slint)
