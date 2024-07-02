<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Printer Demo with Zephyr

## Known Issues

1. Unlike the Espressif integration, we don't provide the platform integration as part of the Slint C++ API. In part, this is due to the way Zephyr OS handles device hardware. Zephyr uses the Device Tree to describe the hardware to the device driver model. In order to register an input event call back we need a pointer to a device obtained from a device tree node, and we also need to know how the driver behaves in order to write our callback function. The existing implementation is generic enough to cover the simulator and display shield drivers. A more general solution could be investigated in the future;
2. Double buffering is not supported as neither the simulator or the hardware used for testing reported it as supported;
3. In the simulator, we convert dirty regions to big-endian. However, where regions overlap, the intersections get converted more than once resulting in incorrect colors on the display. This is a general issue caused by overlapping dirty regions, and should be tackled separately as it also affects the Espressif demo;
4. If there are active animations, we need to make sure to sleep for a fixed period of time, otherwise the event loop runs forever, never re-rendering and never getting to a state where there are no active animations;
5. The example doesn't use the full screen on the hardware;

## Prerequisites

Before you can run this example, make sure you have done the following:

1. Install Rust, with the nightly channel enabled:

   ```bash
   # Install via rustup
   # See https://www.rust-lang.org/tools/install

   # Enable the nightly channel
   rustup toolchain install nightly
   rustup default nightly
   ```

2. Install the Zephyr [dependencies](https://docs.zephyrproject.org/latest/develop/getting_started/index.html#install-dependencies);
3. Install [West](https://docs.zephyrproject.org/latest/develop/west/index.html#west) into a virtual environment:

   ```bash
   # If Slint source is already checked out:
   python3 -m venv ../.venv
   source ../.venv/bin/activate

   # If you do not have Slint source yet:
   mkdir slint-zephyr
   python3 -m venv slint-zephyr/.venv
   source slint-zephyr/.venv/bin/activate

   # Install west
   pip install west
   ```

4. Get the Zephyr source code:

   ```bash
   # If Slint source is already checked out (this adds the Zephyr source next to the Slint source):
   cd ..
   west init -l --mf examples/printerdemo_mcu/zephyr/west.yaml ./slint

   # If you do not have Slint source yet (this checks out Slint and Zephyr source into slint-zephyr):
   west init -m https://github.com/0x6e/slint --mr zephyr --mf examples/printerdemo_mcu/zephyr/west.yaml slint-zephyr
   cd slint-zephyr

   # Checkout the repositories:
   west update
   ```

5. Export a [Zephyr CMake package](https://docs.zephyrproject.org/latest/build/zephyr_cmake_package.html#cmake-pkg). This allows CMake to automatically load boilerplate code required for building Zephyr applications.

   ```bash
   west zephyr-export
   ```

6. Zephyr’s scripts/requirements.txt file declares additional Python dependencies. Install them with pip.

   ```bash
   pip install -r ~/zephyrproject/zephyr/scripts/requirements.txt
   ```

7. [Install the Zephyr SDK](https://docs.zephyrproject.org/latest/develop/getting_started/index.html#install-the-zephyr-sdk) using version [v0.16.8](https://github.com/zephyrproject-rtos/sdk-ng/releases/tag/v0.16.8).

## Build and Run the Example in the Simulator

Once you have the prerequisites, navigate to this directory and execute the following comands:

```bash
# Build
west build -b native_sim/native/64 -p always slint/examples/printerdemo_mcu/zephyr

# Run
./build/zephyr/zephyr.exe
```

The `-p always` option of the build command forces a pristine build. The Zephyr documentation recommends this for new users.

## Build and Run the Example on a Device

This sample has been tested on the [NXP MIMXRT1170-EVKB](https://docs.zephyrproject.org/latest/boards/nxp/mimxrt1170_evk/doc/index.html) with a [RK055HDMIPI4MA0 MIPI display](https://docs.zephyrproject.org/latest/boards/shields/rk055hdmipi4ma0/doc/index.html). The board/debug probe may require configuring as described [here](https://docs.zephyrproject.org/latest/boards/nxp/mimxrt1170_evk/doc/index.html#configuring-a-debug-probe).

```bash
# Build
west build -b mimxrt1170_evk@B/mimxrt1176/cm7 -p always slint/examples/printerdemo_mcu/zephyr -- -DSHIELD=rk055hdmipi4ma0 -DCMAKE_BUILD_TYPE=Release

# Flash
west flash
```
