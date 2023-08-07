# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.14)

include($ENV{IDF_PATH}/tools/cmake/project.cmake)

set(SLINT_ESP_LOCAL_EXAMPLE ON)
set(EXTRA_COMPONENT_DIRS ../../../api/cpp/esp-idf/)

add_compile_options(-fdiagnostics-color=always)

project(slint_esp_printer_demo_mcu LANGUAGES CXX)
