
# Carousel Demo with ESP-IDF

This project demonstrates how to show the carousel demo on an ESP32 S3 Box, ESP32 S3 USB OTG stick, or an ESP32-S2 Kaluga Kit. It has been tested and proven to work with these specific models.

## Prerequisites

Before you can run this example, make sure you have the following:

- An ESP32 S3 Box, ESP32-USB-OTG stick, or an ESP32 S2 Kaluga Kit
- The Rust xtensa toolchain, which can be obtained from [esp-rs](https://github.com/esp-rs/). Use the installation instructions provided by [espup](https://github.com/esp-rs/espup#installation) to install it.
- The esp-idf SDK. The installation guide can be found at [esp-idf documentation](https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/get-started/index.html#installation).

## Running the Example

Once you have the prerequisites, navigate to either the `s3-box`, `s3-usb-otg`, or `s2-kaluga-kit` directory and execute the following command:

    . ${IDF_PATH}/export.sh
    idf.py build
    idf.py flash monitor

This will build the project, flash it to your ESP32 device, and open a monitor to view the output of the device.
