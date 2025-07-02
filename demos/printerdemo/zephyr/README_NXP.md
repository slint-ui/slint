<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Slint Demo on Zephyr

A fictional user interface for the touch screen of a printer implemented in Slint and Rust.

![Screenshot of the Printer Demo](https://slint.dev/resources/printerdemo_screenshot.png "Printer Demo")
<!----- Boards ----->
[![License badge](https://img.shields.io/badge/License-MIT-red)](https://github.com/slint-ui/slint/blob/master/LICENSES/MIT.txt)
[![Language badge](https://img.shields.io/badge/Language-Rust-yellow)](https://github.com/rust-lang/rust) [![Language badge](https://img.shields.io/badge/Language-Slint-yellow)](https://github.com/slint-ui/slint/tree/master)
[![Board badge](https://img.shields.io/badge/Board-MIMXRT1170&ndash;EVK-blue)](https://www.nxp.com/pip/MIMXRT1170-EVK)
[![Category badge](https://img.shields.io/badge/Category-GRAPHICS-yellowgreen)](https://mcuxpresso.nxp.com/appcodehub?category=graphics) [![Category badge](https://img.shields.io/badge/Category-HMI-yellowgreen)](https://mcuxpresso.nxp.com/appcodehub?category=hmi) [![Category badge](https://img.shields.io/badge/Category-USER%20INTERFACE-yellowgreen)](https://mcuxpresso.nxp.com/appcodehub?category=ui) [![Category badge](https://img.shields.io/badge/Category-TOOLS-yellowgreen)](https://mcuxpresso.nxp.com/appcodehub?category=tools)

## Table of Contents

1. [Pre-requisites](#1-pre-requisites)
2. [Hardware](#2-hardware)
3. [Setup](#3-setup)
4. [FAQs](#4-faqs)
5. [Support](#5-support)

## 1. Pre-requisites

Before you can run this demo, make sure you have done the following:

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
   west init -l --mf demos/printerdemo/zephyr/west.yaml ./slint

   # If you do not have Slint source yet (this checks out Slint and Zephyr source into slint-zephyr):
   west init -m https://github.com/slint-ui/slint --mr zephyr --mf demos/printerdemo/zephyr/west.yaml slint-zephyr
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

## 2. Hardware

This sample has been tested on the [NXP MIMXRT1170-EVKB](https://docs.zephyrproject.org/latest/boards/nxp/mimxrt1170_evk/doc/index.html) with a [RK055HDMIPI4MA0 MIPI display](https://docs.zephyrproject.org/latest/boards/shields/rk055hdmipi4ma0/doc/index.html). The board/debug probe may require configuring as described [here](https://docs.zephyrproject.org/latest/boards/nxp/mimxrt1170_evk/doc/index.html#configuring-a-debug-probe).

```bash
# Build
west build -b mimxrt1170_evk@B/mimxrt1176/cm7 -p always slint/demos/printerdemo/zephyr -- -DSHIELD=rk055hdmipi4ma0 -DCMAKE_BUILD_TYPE=Release

# Flash
west flash
```

## 3. Setup

### 3.1 Build and Run in Simulator

Once you have the prerequisites, navigate to this directory and execute the following commands:

```bash
# Build
west build -b native_sim/native/64 -p always slint/demos/printerdemo/zephyr

# Run
./build/zephyr/zephyr.exe
```

The `-p always` option of the build command forces a pristine build. The Zephyr documentation recommends this for new users.

### 3.2 Build and Run on Device

This demo has been tested on the [NXP MIMXRT1170-EVKB](https://docs.zephyrproject.org/latest/boards/nxp/mimxrt1170_evk/doc/index.html) with a [RK055HDMIPI4MA0 MIPI display](https://docs.zephyrproject.org/latest/boards/shields/rk055hdmipi4ma0/doc/index.html). The board/debug probe may require configuring as described [here](https://docs.zephyrproject.org/latest/boards/nxp/mimxrt1170_evk/doc/index.html#configuring-a-debug-probe).

```bash
# Build
west build -b mimxrt1170_evk@B/mimxrt1176/cm7 -p always slint/demos/printerdemo/zephyr -- -DSHIELD=rk055hdmipi4ma0 -DCMAKE_BUILD_TYPE=Release

# Flash
west flash
```

## 4. FAQs

<https://github.com/slint-ui/slint/blob/master/FAQ.md>

## 5. Support

Questions regarding the content/correctness of this demo can be entered as Issues within this GitHub repository.

E-Mail: <info@slint.dev>

Chat: <https://chat.slint.dev>

GitHub Issues: <https://github.com/slint-ui/slint/issues>

GitHub Discussions: <https://github.com/slint-ui/slint/discussions>

>**Warning**: For more general technical questions regarding NXP Microcontrollers and the difference in expected functionality, enter your questions on the [NXP Community Forum](https://community.nxp.com/)

[![Follow us on Youtube](https://img.shields.io/badge/Youtube-Follow%20us%20on%20Youtube-red.svg)](https://www.youtube.com/slint-ui) [![Follow us on LinkedIn](https://img.shields.io/badge/LinkedIn-Follow%20us%20on%20LinkedIn-blue.svg)](https://www.linkedin.com/company/slint-ui/) [![Follow us on Twitter](https://img.shields.io/badge/X-Follow%20us%20on%20X-black.svg)](https://twitter.com/slint_ui)
