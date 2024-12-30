# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

idf_component_register(
    SRCS "src/slint-esp.cpp"
    INCLUDE_DIRS "include"
    REQUIRES "esp_lcd" "esp_lcd_touch")

list(PREPEND CMAKE_MODULE_PATH "${CMAKE_SOURCE_DIR}/cmake")

if (CONFIG_IDF_TARGET_ARCH_XTENSA)
    set(rust_target "xtensa-${IDF_TARGET}-none-elf")
elseif(CONFIG_IDF_TARGET_ARCH_RISCV)
    if (CONFIG_IDF_TARGET_ESP32C6 OR CONFIG_IDF_TARGET_ESP32C5 OR CONFIG_IDF_TARGET_ESP32H2)
        set(rust_target "riscv32imac-esp-espidf")
    elseif (CONFIG_IDF_TARGET_ESP32P4)
        set(rust_target "riscv32imafc-esp-espidf")
    else ()
        set(rust_target "riscv32imc-esp-espidf")
    endif()
else()
    message(FATAL_ERROR "Architecture currently not supported")
endif()

set(SLINT_FEATURE_FREESTANDING ON)
set(SLINT_FEATURE_RENDERER_SOFTWARE ON)
set(SLINT_LIBRARY_CARGO_FLAGS "-Zbuild-std=core,alloc")
set(DEFAULT_SLINT_EMBED_RESOURCES "embed-for-software-renderer" CACHE STRING "")
set(CMAKE_BUILD_TYPE Release)
set(BUILD_SHARED_LIBS OFF)
set(Rust_CARGO_TARGET ${rust_target})

if (SLINT_ESP_LOCAL_EXAMPLE)
    add_subdirectory(../.. slint_build)
else()
    list(PREPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_LIST_DIR}/cmake")
    find_package(Slint)

    if (TARGET Slint::Slint)
        # Imported targets are only visible in directory scope by default, but
        # for use by the application, we need to make the target global.
        set_target_properties(Slint::Slint PROPERTIES IMPORTED_GLOBAL TRUE)
    else()
        include(FetchContent)
        FetchContent_Declare(
            Slint
            GIT_REPOSITORY https://github.com/slint-ui/slint
            GIT_TAG v1.9.2
            SOURCE_SUBDIR api/cpp
        )
        FetchContent_MakeAvailable(Slint)
    endif()
endif()

target_link_libraries(${COMPONENT_LIB} PUBLIC Slint::Slint)
