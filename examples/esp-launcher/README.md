Hack launcher for ESP32P4


## Build the home automation demo

```bash
cd demos/home-automation/esp-idf
idf.py build
```

## Build the use cases demo

```bash
cd demos/usecases/esp-idf
idf.py build
```

## Build the launcher

```bash
cd examples/esp-launcher
idf.py build
```

## Generate the thing to flash and flash it

```bash
esptool.py --chip esp32p4 merge_bin -o ./build/combined.bin --flash_mode dio --flash_size 16MB \
    0x2000 build/bootloader/bootloader.bin \
    0x8000 build/partition_table/partition-table.bin \
    0xf000 build/ota_data_initial.bin \
    0x100000 build/launcher.bin \
    0x300000 ../../demos/usecases/esp-idf/build/slint_esp_usecases_mcu.bin \
    0x500000 ../../demos/home-automation/esp-idf/build/home-automation.bin \
    0xa00000 ../../demos/energy-monitor/esp-idf/build/energy-monitor.bin

esptool.py --chip esp32p4 write_flash 0x0 ./build/combined.bin

```