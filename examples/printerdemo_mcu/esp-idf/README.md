<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# ESP32-S3-Box Printer Demo with ESP-IDF

This project demonstrates how to use a printer with ESP32-S3-Box. It has been tested and proven to work with this specific model.

## Prerequisites

Before you can run this example, make sure you have the following:

- An ESP32-S3-Box
- The Rust xtensa toolchain, which can be obtained from [esp-rs](https://github.com/esp-rs/). Use the installation instructions provided by [espup](https://github.com/esp-rs/espup#installation) to install it.
- The esp-idf SDK. The installation guide can be found at [esp-idf documentation](https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/get-started/index.html#installation).

## Running the Example

Once you have the prerequisites, navigate to this directory and execute the following commands:

    . ${IDF_PATH}/export.sh
    idf.py build
    idf.py flash monitor

This will build the project, flash it to your ESP32-S3-Box, and open a monitor to view the output of the device.
