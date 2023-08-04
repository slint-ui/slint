# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

idf_component_register(
  SRCS "src/esp_slint.cpp"
  INCLUDE_DIRS "include"
  REQUIRES "esp_lcd" "esp_lcd_touch"
  PRIV_REQUIRES "esp_timer")

set(Rust_CARGO_TARGET "xtensa-${IDF_TARGET}-none-elf")
set(CORROSION_NO_WARN_PARSE_TARGET_TRIPLE_FAILED ON)
set(SLINT_FEATURE_STD OFF)
set(SLINT_FEATURE_RENDERER_SOFTWARE ON)
set(SLINT_TARGET_CARGO_FLAGS "-Zbuild-std=core,alloc")
set(CMAKE_BUILD_TYPE Release)
set(BUILD_SHARED_LIBS OFF)

# FIXME: this never uses the current git repo
include(FetchContent)
FetchContent_Declare(
  Slint
  GIT_REPOSITORY https://github.com/slint-ui/slint
  GIT_TAG master #FIXME: this needs to be hardcoded to a version number
  SOURCE_SUBDIR api/cpp
)
FetchContent_MakeAvailable(Slint)

target_link_libraries(${COMPONENT_LIB} PUBLIC Slint::Slint)
