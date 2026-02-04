# Printer Demo MCU with Zephyr

## Known Issues

1. Unlike the Espressif integration, we don't provide the platform integration as part of the Slint C++ API. In part, this is due to the way Zephyr OS handles device hardware. Zephyr uses the Device Tree to describe the hardware to the device driver model. In order to register an input event call back we need a pointer to a device obtained from a device tree node, and we also need to know how the driver behaves in order to write our callback function. The existing implementation is generic enough to cover the simulator and display shield drivers. A more general solution could be investigated in the future;
2. Double buffering is not supported as neither the simulator or the hardware used for testing reported it as supported;

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
   west init -l --mf demos/zephyr-common/west.yaml ./slint

   # If you do not have Slint source yet (this checks out Slint and Zephyr source into slint-zephyr):
   west init -m https://github.com/slint-ui/slint --mr zephyr --mf demos/zephyr-common/west.yaml slint-zephyr
   cd slint-zephyr

   # Checkout the repositories:
   west update
   ```

5. Export a [Zephyr CMake package](https://docs.zephyrproject.org/latest/build/zephyr_cmake_package.html#cmake-pkg). This allows CMake to automatically load boilerplate code required for building Zephyr applications.

   ```bash
   west zephyr-export
   ```

6. Zephyr's scripts/requirements.txt file declares additional Python dependencies. Install them with pip.

   ```bash
   pip install -r ~/zephyrproject/zephyr/scripts/requirements.txt
   ```

7. [Install the Zephyr SDK](https://docs.zephyrproject.org/latest/develop/getting_started/index.html#install-the-zephyr-sdk) using version [v0.16.8](https://github.com/zephyrproject-rtos/sdk-ng/releases/tag/v0.16.8).

## Build and Run the Example in the Simulator

Once you have the prerequisites, navigate to this directory and execute the following commands:

```bash
# Build
west build -b native_sim/native/64 -p always slint/demos/printerdemo_mcu/zephyr

# Run
./build/zephyr/zephyr.exe
```

The `-p always` option of the build command forces a pristine build. The Zephyr documentation recommends this for new users.

## Build and Run the Example on a Device

### NXP FRDM-MCXN947 + LCD-PAR-S035

This sample has been tested on the [NXP FRDM-MCXN947](https://docs.zephyrproject.org/latest/boards/nxp/frdm_mcxn947/doc/index.html) with a [LCD-PAR-S035](https://docs.zephyrproject.org/latest/boards/shields/lcd_par_s035/doc/index.html) display shield.

```bash
# Build
west build -b frdm_mcxn947/mcxn947/cpu0 -p always slint/demos/printerdemo_mcu/zephyr -- -DSHIELD=lcd_par_s035_8080
```

The standard `west flash` does not work reliably with this board. Use the following workaround with pyocd and nxpdebugmbox:

```bash
# Flash
nxpdebugmbox -i mcu-link tool reset -f mcxn947 && \
pyocd flash -O pack.debug_sequences.disabled_sequences=ResetSystem,ResetCatchClear \
  -M halt -e chip -t mcxn947 -a 0x10000000 build/zephyr/zephyr.bin
```

**Note on touch input:** On Zephyr < v4.4.0, the GT911 touch controller lacks `touchscreen-common` support, so touch coordinate transforms are handled in C++ code. On Zephyr >= v4.4.0 (which includes upstream commit `0f07faa14b3`), uncomment the device tree properties in the [board overlay](../../zephyr-common/boards/frdm_mcxn947_mcxn947_cpu0.overlay) and remove the corresponding workaround in [slint-zephyr.cpp](../../zephyr-common/slint-zephyr.cpp).
