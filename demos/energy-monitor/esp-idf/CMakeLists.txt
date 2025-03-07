# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.14)

include($ENV{IDF_PATH}/tools/cmake/project.cmake)

set(SLINT_ESP_LOCAL_EXAMPLE ON)
set(SLINT_FEATURE_EXPERIMENTAL ON)
set(EXTRA_COMPONENT_DIRS ../../../api/cpp/esp-idf/)
set(SLINT_STYLE "cosmic-light" CACHE STRING "Style for demo" FORCE)

add_compile_options(-fdiagnostics-color=always)

project(slint_esp_usecases_mcu LANGUAGES CXX)
