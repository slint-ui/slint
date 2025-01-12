<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Introduction

Microcontrollers (MCUs) are highly customizable and each vendor typically provides their own development
environment and toolchain. Slint aims to support any MCU provided the SDK supports a C++ 20 cross-compiler
as well as CMake as build system.

This documentation is divided into three sub-sections:

  - [ESP-IDF section](esp_idf.md), when targeting MCUs with Espressif's IoT Development Framework
  - [STM32 section](stm32.md), when targeting MCUs in STMicroelectronics' STM32Cube Ecosystem.
  - [Generic section](generic.md), providing an overview how to use Slint with other MCUs.
