<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Home Automation Demo with Zephyr

A fictional Home Automation User Interface implemented in Slint and C++.

![Screenshot of the Home Automation Demo](https://github.com/user-attachments/assets/3856b9cf-e7c7-478e-8efe-0f7e8aa43e85 "Home Automation Demo")

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

7. [Install the Zephyr SDK](https://docs.zephyrproject.org/latest/develop/getting_started/index.html#install-the-zephyr-sdk) using version [v1.0.1](https://github.com/zephyrproject-rtos/sdk-ng/releases/tag/v1.0.1).

## Build and Run the Example in the Simulator

Once you have the prerequisites, navigate to this directory and execute the following commands:

```bash
# Build
west build -b native_sim/native/64 -p always slint/demos/home-automation/zephyr

# Run
./build/zephyr/zephyr.exe
```

The `-p always` option of the build command forces a pristine build. The Zephyr documentation recommends this for new users.

## Build and Run the Example on a Device

This sample has been tested on the [NXP MIMXRT1170-EVKB](https://docs.zephyrproject.org/latest/boards/nxp/mimxrt1170_evk/doc/index.html) with a [RK055HDMIPI4MA0 MIPI display](https://docs.zephyrproject.org/latest/boards/shields/rk055hdmipi4ma0/doc/index.html). The board/debug probe may require configuring as described [here](https://docs.zephyrproject.org/latest/boards/nxp/mimxrt1170_evk/doc/index.html#configuring-a-debug-probe).

```bash
# Build
west build -b mimxrt1170_evk@B/mimxrt1176/cm7 -p always slint/demos/home-automation/zephyr -- -DSHIELD=rk055hdmipi4ma0 -DCMAKE_BUILD_TYPE=Release

# Flash
west flash
```

This sample has also been tested on the [NXP FRDM-MCXN947](https://docs.zephyrproject.org/latest/boards/nxp/frdm_mcxn947/doc/index.html) with an [LCD-PAR-S035 shield](https://www.nxp.com/part/LCD-PAR-S035).

If your Zephyr checkout is v4.4.0 or later, first apply a required upstream fix (not yet backported to the v4.4.x release branch) for a display-corrupting bug in the MCUX eDMA driver on EDMA-V4/no-DMAMUX parts like MCXN947 (fixed upstream in [`05aa0414f2b`](https://github.com/zephyrproject-rtos/zephyr/commit/05aa0414f2b04b64f06da5ab38ed85fff6e8976a), which itself fixes the regression introduced by [`efe48ea674c`](https://github.com/zephyrproject-rtos/zephyr/commit/efe48ea674cda08910347c660706e5c6155d21f9)). Without it, the FlexIO-driven parallel display on this board renders mostly static/garbage instead of the UI. On Zephyr v4.0.0 this bug does not exist, so the patch is unnecessary there.

```bash
(cd zephyr && git apply ../slint/demos/zephyr-common/patches/0001-drivers-dma-mcux_edma-handle-m2m-on-V4-instances-without-channel-mux.patch)
```

```bash
# Build
west build -b frdm_mcxn947/mcxn947/cpu0 -p always slint/demos/home-automation/zephyr -- -DSHIELD=lcd_par_s035_8080 -DCMAKE_BUILD_TYPE=Release
```

`west flash` requires [LinkServer](https://www.nxp.com/design/design-center/software/development-software/mcuxpresso-software-and-tools-/linkserver-for-microcontrollers:LINKERSERVER), which this workflow does not assume is installed. As a workaround, flash with `pyocd`, preceded by an `nxpdebugmbox` reset (plain `pyocd flash` alone times out on this board's flash driver):

```bash
nxpdebugmbox -i mcu-link tool reset -f mcxn947 && \
pyocd flash \
  -O pack.debug_sequences.disabled_sequences=ResetSystem,ResetCatchClear \
  -M halt -e chip -t mcxn947 -a 0x10000000 \
  build/zephyr/zephyr.bin
```

If the `nxpdebugmbox` reset itself fails with `TransferTimeoutError`, the board's debug port may be unresponsive (observed after the board ran arbitrary firmware for a while). Put the target into ISP mode first, then retry the command above:

1. Press and hold the ISP button (SW3, bottom-right corner).
2. Press and release the Reset button (SW1, upper-left corner).
3. Release SW3.
