---
title: "MCU: Introduction"
description: Overview of using Slint on microcontrollers — ESP-IDF, STM32 and generic MCU environments.
---

Microcontrollers (MCUs) are highly customizable and each vendor typically provides their own development
environment and toolchain. Slint aims to support any MCU provided the SDK supports a C++ 20 cross-compiler
as well as CMake as build system.

This documentation is divided into three sub-sections:

  - [ESP-IDF section](../esp-idf/), when targeting MCUs with Espressif's IoT Development Framework
  - [STM32 section](../stm32/), when targeting MCUs in STMicroelectronics' STM32Cube Ecosystem.
  - [Generic section](../generic/), providing an overview how to use Slint with other MCUs.
