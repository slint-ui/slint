<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Generic MCU Environment Setup

We aim to support many different MCUs and their respective software development environments.
For those environments where we can't provide an out-of-the-box integration, we provide the
following generic instructions on what's needed to compile and use Slint.

## Prerequisites

* Install Rust by following the [Rust Getting Started Guide](https://www.rust-lang.org/learn/get-started). If you already
  have Rust installed, make sure that it's at least version 1.82 or newer. You can check which version you have installed
  by running `rustc --version`. Once this is done, you should have the `rustc` compiler and the `cargo` build system installed in your path.

* A C++ cross-compiler compiler that supports C++20.

* **[cmake](https://cmake.org/download/)** (3.21 or newer)

  * Slint comes with a CMake integration that automates the compilation step of the `.slint` markup language files and offers a CMake target for convenient linkage.

  * *Note*: We recommend using the Ninja generator of CMake for the most efficient build and `.slint` dependency tracking. Install [Ninja](https://ninja-build.org) and select the CMake Ninja backend by passing `-GNinja` or set the `CMAKE_GENERATOR` environment variable to `Ninja`.

  * A build environment for [cross-compilation with CMake](https://cmake.org/cmake/help/latest/manual/cmake-toolchains.7.html#cross-compiling), such as a toolchain file.

## Compiling Slint

To target an MCU environment, all of the following additional CMake configuration options must be set when compiling Slint:

| Option                                                        | Description                                                          |
|---------------------------------------------------------------|----------------------------------------------------------------------|
| `-DSLINT_FEATURE_FREESTANDING=ON`                             | Enables building for environments without a standard library.        |
| `-DBUILD_SHARED_LIBS=OFF`                                     | Disables shared library support and instead builds Slint statically. |
| `-DSLINT_FEATURE_RENDERER_SOFTWARE=ON`                        | Enable support for the software renderer.                            |
| `-DDEFAULT_SLINT_EMBED_RESOURCES=embed-for-software-renderer` | Default to pre-compiling images and fonts.                           |


For example, if you're targeting an MCU with a ARM Cortex-M processor, the complete command line for CMake could look like this:

```sh
cmake -DRust_CARGO_TARGET=thumbv7em-none-eabihf -DSLINT_FEATURE_FREESTANDING=ON
      -DBUILD_SHARED_LIBS=OFF -DSLINT_FEATURE_RENDERER_SOFTWARE=ON
      -DDEFAULT_SLINT_EMBED_RESOURCES=embed-for-software-renderer
      ..
```

## Next Steps

 - Check out the [](../getting_started.md) instructions for a generic "Hello World" with C++.
 - Study the [](../api/library_root), in particular the `slint::platform` namespace for
   writing a Slint platform integration to handle touch input and render pixel, which you
   need to forward to your MCU's display driver.
 - For more details about the Slint language, check out the [Slint Language Documentation](slint-reference:).
 - Learn about the [](../types.md) between Slint and C++.
