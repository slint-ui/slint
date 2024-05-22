# Printer Demo with Zephyr

## Prerequisites

Before you can run this example, make sure you have the following:

- The Zephyr development environment. Use the Zephyr [getting started guide](https://docs.zephyrproject.org/latest/develop/getting_started/index.html) to install it.
- Rust, with the nightly channel enabled:

      # Install via rustup
      # See https://www.rust-lang.org/tools/install

      # Enable the nightly channel
      rustup toolchain install nightly
      rustup default nightly


## Running the Example in the Simulator

Once you have the prerequisites, navigate to this directory and execute the following comands:

    # Activate the Zephyr virtualenv
    source <path/to>/zephyrproject/.venv/bin/activate

    # Configure for the simulator:
    cmake -GNinja -S . -B ./build -DBOARD=native_sim/native/64

    # Build
    cmake --build ./build -t run

## Running the Example on a Device

This sample has been tested on the [NXP MIMXRT1170-EVKB](https://docs.zephyrproject.org/latest/boards/nxp/mimxrt1170_evk/doc/index.html) with a [RK055HDMIPI4MA0 MIPI display](https://docs.zephyrproject.org/latest/boards/shields/rk055hdmipi4ma0/doc/index.html). The board/debug probe may require configuring as described [here](https://docs.zephyrproject.org/latest/boards/nxp/mimxrt1170_evk/doc/index.html#configuring-a-debug-probe).

    # Activate the Zephyr virtualenv
    source <path/to>/zephyrproject/.venv/bin/activate

    # Configure for NXP MIMXRT1170-EVKB + RK055HDMIPI4MA0 MIPI Display
    cmake -GNinja -S . -B ./build-imxrt1170 -DCMAKE_BUILD_TYPE=Release -DBOARD=mimxrt1170_evk@B/mimxrt1176/cm7 -DSHIELD=rk055hdmipi4ma0

    # Build
    cmake --build ./build-imxrt1170

    # Flash the board. Configure SW1 as described:
    # https://docs.zephyrproject.org/latest/boards/nxp/mimxrt1170_evk/doc/index.html#flashing
    cd <path/to>/zephyrproject
    west flash --build-dir <path/to>/build-imxrt1170
