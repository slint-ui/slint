# Home Automation Demo on ESP32-P4 Function EV Board

This project runs the Slint home-automation demo on the ESP32-P4 Function EV board with C++ and ESP-IDF.

## Prerequisites

- ESP-IDF 5.4.x installed locally
- Rust toolchain for `channel = "esp"` available, because the Slint ESP-IDF component builds helper tools from source when needed
- ESP32-P4 Function EV board connected over USB

## Build

```sh
. ${IDF_PATH}/export.sh
cd demos/home-automation/esp-idf
idf.py set-target esp32p4
idf.py build
```

## Flash and Monitor

```sh
. ${IDF_PATH}/export.sh
cd demos/home-automation/esp-idf
idf.py flash monitor
```

