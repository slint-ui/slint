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


## Running the Example

Once you have the prerequisites, navigate to this directory and execute the following comands:

    # Activate the Zephyr virtualenv
    source <path/to>/zephyrproject/.venv/bin/activate

    # CMake only:
    cmake -GNinja -S . -B ./build -DBOARD=native_sim/native/64
    cmake --build ./build -t run
