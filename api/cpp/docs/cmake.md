<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
<!-- cSpell: ignore ccmake dslint femtovg -->

# Set Up Development Environment

## Prerequisites

* A C++ compiler that supports C++20 (e.g., **MSVC 2019 16.6** on Windows)

* **[cmake](https://cmake.org/download/)** (3.21 or newer)

  * Slint comes with a CMake integration that automates the compilation step of the `.slint` markup language files and offers a CMake target for convenient linkage.

  * *Note*: We recommend using the Ninja generator of CMake for the most efficient build and `.slint` dependency tracking. Install [Ninja](https://ninja-build.org) and select the CMake Ninja backend by passing `-GNinja` or set the `CMAKE_GENERATOR` environment variable to `Ninja`.

## Install Slint

To install Slint, either download the [binary packages](#install-binary-packages) or [build from sources](#build-from-sources).

*Note*: Binary packages are available for only Linux and Windows on x86-64 architecture. The recommended and most flexible way to use the C++ API is to build Slint from sources.

### Install Binary Packages

The Slint binary packages work without any Rust development environment.

Steps:

1. Open <https://github.com/slint-ui/slint/releases>

2. Click on the latest release

3. From "Assets" ("XXX" refers to the version of the latest release),

   * for Linux x86-64 architecture - download `slint-cpp-XXX-Linux-x86_64.tar.gz`
   * for Windows x86-64 architecture - download `slint-cpp-XXX-win64-MSVC.exe`

4. Unpack the downloaded archive (Linux) or run the installer executable (Windows).

5. Set environment variables

   * set `CMAKE_PREFIX_PATH` to the installation directory of Slint. Alternatively you can pass `-DCMAKE_PREFIX_PATH=/path/to/installed/slint` argument when invoking cmake. This helps `find_package(Slint)` to find Slint from within a `CMakeLists.txt` file.

   * add the `lib` sub-directory in the installation directory of Slint to `LD_LIBRARY_PATH` (Linux) or to the `PATH` environment variable (Windows). This is necessary to find the Slint libraries when running a Slint program.

In the next section you will learn how to use the installed library in your application and how to work with `.slint` UI files.

### Build From Sources

First you need to install the prerequisites:

* Install Rust by following the [Rust Getting Started Guide](https://www.rust-lang.org/learn/get-started). If you already
  have Rust installed, make sure that it's at least version 1.82 or newer. You can check which version you have installed
  by running `rustc --version`. Once this is done, you should have the `rustc` compiler and the `cargo` build system installed in your path.

You can either choose to compile Slint from source along with your application or include Slint as an external CMake package.

* To compile Slint along with your application, include Slint into your CMake project using CMake's [`FetchContent`](https://cmake.org/cmake/help/latest/module/FetchContent.html) feature. Insert the following snippet into your `CMakeLists.txt` to make CMake download the latest released 1.x version, compile it, and make the CMake
integration available:

```cmake
include(FetchContent)
FetchContent_Declare(
    Slint
    GIT_REPOSITORY https://github.com/slint-ui/slint.git
    # `release/1` will auto-upgrade to the latest Slint >= 1.0.0 and < 2.0.0
    # `release/1.0` will auto-upgrade to the latest Slint >= 1.0.0 and < 1.1.0
    GIT_TAG release/1
    SOURCE_SUBDIR api/cpp
)
FetchContent_MakeAvailable(Slint)
```

* To include Slint as an external CMake package, build Slint from source like a regular CMake project, install it into a prefix directory of your choice and use `find_package(Slint)` in your `CMakeLists.txt`.


### Features

The Slint library supports a set of features, not all of them enabled by default.
You might want to adapt the set of enabled features to optimize your binary
size. For example you might want to support only the wayland stack on Linux.
Enable the `backend-winit-wayland` feature while turning off the
`backend-winit-x11` feature to do so.

Slint's CMake configuration uses CMake options prefixed with `SLINT_FEATURE_` to
expose Slint's feature flags at compile time. To have a wayland-only stack with
the CMake setup you would for example use:

   `cmake -DSLINT_FEATURE_BACKEND_WINIT=OFF -DSLINT_FEATURE_BACKEND_WINIT_WAYLAND=ON ...`

Alternatively, you can use `cmake-gui` or `ccmake` for a more interactive way
to discover and toggle features.

This works when compiling Slint as a package, using `cmake --build` and
`cmake --install`, or when including Slint using `FetchContent`.

If you need to check in your application's `CMakeLists.txt` whether a feature is enabled
or disabled, read the `SLINT_ENABLED_FEATURES` and `SLINT_DISABLED_FEATURES` target
properties from the `Slint::Slint` cmake target:

```cmake
get_target_property(slint_enabled_features Slint::Slint SLINT_ENABLED_FEATURES)
if ("BACKEND_WINIT" IN_LIST slint_enabled_features)
    ...
endif()
```

Similarly, if you need to check for features at compile-time, check for the existence
of `SLINT_FEATURE_<NAME>` pre-processor macros:

```
#include <slint.h>

#if defined(SLINT_FEATURE_BACKEND_WINIT)
// ...
#endif
```

### Rust Flags

Slint uses [Corrosion](https://github.com/corrosion-rs/corrosion) to build Slint, which is developed in Rust. You can utilize [Corrosion's global CMake variables](https://corrosion-rs.github.io/corrosion/usage.html#global-corrosion-options) to control certain aspects of the Rust build process.

Furthermore, you can set the `SLINT_LIBRARY_CARGO_FLAGS` cache variable to specify additional flags for the Slint runtime during the build.

### Platform Backends

In Slint, a backend is the module that encapsulates the interaction with the operating system,
in particular the windowing sub-system. Multiple backends can be compiled into Slint and one
backend is selected for use at run-time on application start-up. You can configure Slint without
any built-in backends, and instead develop your own backend by implementing Slint's platform
abstraction and window adapter interfaces.

For more information about the available backends, their system requirements, and configuration
options, see the {{ '[Backend & Renderers Documentation]({})'.format(slint_href_backends_and_renderers) }}.

By default Slint will include both the Qt and
[winit](https://crates.io/crates/winit) back-ends -- if both are detected at
compile time. You can enable or disable back-ends using the
`SLINT_FEATURE_BACKEND_` features. For example, to exclude the winit back-end,
you would disable the `SLINT_FEATURE_BACKEND_WINIT` option in your CMake
project configuration.

The winit back-end needs a renderer. `SLINT_FEATURE_RENDERER_FEMTOVG` and
`SLINT_FEATURE_RENDERER_SKIA` are the only stable renderers, the other ones are
experimental.

### Cross-compiling

It's possible to cross-compile Slint to a different target architecture when
building with CMake. You need to make sure your CMake setup is ready for
cross-compilation, as documented in the [upstream CMake documentation](https://cmake.org/cmake/help/latest/manual/cmake-toolchains.7.html#cross-compiling).

If you are building against a Yocto SDK, it is sufficient to source the SDK's environment setup file.

Since Slint is implemented using the Rust programming language, you need to
determine which Rust target matches the target architecture that you're
compiling for. Please consult the [upstream Rust documentation](https://doc.rust-lang.org/nightly/rustc/platform-support.html) to find the correct target name. Now you need to install the Rust toolchain:

```sh
rustup target add <target-name>
```

Then you're ready to iconfigure your CMake project you need to add
`-DRust_CARGO_TARGET=<target name>` to the CMake command line.
This ensures that the Slint library is built for the correct architecture.

For example if you are building against an embedded Linux Yocto SDK targeting
an ARM64 board, the following commands show how to compile:

Install the Rust targe toolchain once:

<!-- cSpell:disable -->
```sh
rustup target add aarch64-unknown-linux-gnu
```
<!-- cSpell:enable -->

Set up the environment and build:

<!-- cSpell:disable -->
```sh
. /path/to/yocto/sdk/environment-setup-cortexa53-crypto-poky-linux
cd <PROJECT_ROOT>
mkdir build
cd build
cmake -DRust_CARGO_TARGET=aarch64-unknown-linux-gnu -DCMAKE_INSTALL_PREFIX=/slint/install/path ..
cmake --build .
cmake --install .
```
