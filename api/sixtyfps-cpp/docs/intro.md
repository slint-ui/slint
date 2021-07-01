# SixtyFPS-cpp

## A C++ UI toolkit

[SixtyFPS](https://sixtyfps.io/) is a UI toolkit that supports different programming languages.
SixtyFPS.cpp is the C++ API to interact with a SixtyFPS UI from C++.

The complete C++ documentation can be viewed online at https://sixtyfps.io/docs/cpp/.

If you are new to SixtyFPS, you might also consider going through our [Walk-through tutorial](https://sixtyfps.io/docs/tutorial/cpp).

**Warning: Pre-Alpha**
SixtyFPS is still in the early stages of development: APIs will change and important features are still being developed.

## Installing or Building SixtyFPS

SixtyFPS comes with a CMake integration that automates the compilation step of the `.60` markup language files and
offers a CMake target for convenient linkage.

*Note*: We recommend using the Ninja generator of CMake for the most efficient build and `.60` dependency tracking.
You can select the CMake Ninja backend by passing `-GNinja` or setting the `CMAKE_GENERATOR` environment variable to `Ninja`.

### Building from Sources

The recommended and most flexible way to use the C++ API is to build SixtyFPS from sources.

First you need to install the prerequisites:

* Install Rust by following the [Rust Getting Started Guide](https://www.rust-lang.org/learn/get-started). Once this is done,
  you should have the ```rustc``` compiler and the ```cargo``` build system installed in your path.
* **cmake** (3.16 or newer)
* A C++ compiler that supports C++17 (e.g., **MSVC 2019** on Windows)

You can include SixtyFPS in your CMake project using CMake's `FetchContent` feature. Insert the following snippet into your
`CMakeLists.txt` to make CMake download the latest release, compile it and make the CMake integration available:

```cmake
include(FetchContent)
FetchContent_Declare(
    SixtyFPS
    GIT_REPOSITORY https://github.com/sixtyfpsui/sixtyfps.git
    GIT_TAG v0.1.0
    SOURCE_SUBDIR api/sixtyfps-cpp
)
FetchContent_MakeAvailable(SixtyFPS)
```

If you prefer to treat SixtyFPS as an external CMake package, then you can also build SixtyFPS from source like a regular
CMake project, install it into a prefix directory of your choice and use `find_package(SixtyFPS)` in your `CMakeLists.txt`.

#### Cross-compiling

It is possible to cross-compile SixtyFPS to a different target architecture when building with CMake. In order to complete
that, you need to make sure that your CMake setup is ready for cross-compilation. You can find more information about
how to set this up in the [upstream CMake documentation](https://cmake.org/cmake/help/latest/manual/cmake-toolchains.7.html#cross-compiling).
If you are building against a Yocto SDK, it is sufficient to source the SDK's environment setup file.

Since SixtyFPS is implemented using the Rust programming language, you need to determine which Rust target
matches the target architecture that you're compiling to. Please consult the [upstream Rust documentation](https://doc.rust-lang.org/nightly/rustc/platform-support.html) to find the correct target name. Now you need to install the Rust toolchain:

```sh
rustup target add <target-name>
```

Then you're ready to invoke CMake and you need to add `-DRust_CARGO_TARGET=<target name>` to the CMake command line.
This ensures that the SixtyFPS library is built for the correct architecture.

For example if you are building against an embedded Linux Yocto SDK targeting an ARM64 board, the following commands
show how to compile:

Install the Rust targe toolchain once:

```sh
rustup target add aarch64-unknown-linux-gnu
```

Set up the environment and build:

```sh
. /path/to/yocto/sdk/environment-setup-cortexa53-crypto-poky-linux
cd sixtyfps
mkdir build
cd build
cmake -DRust_CARGO_TARGET=aarch64-unknown-linux-gnu -DCMAKE_INSTALL_PREFIX=/sixtyfps/install/path ..
cmake --build .
cmake --install .
```

### Binary Packages

The SixtyFPS continuous integration system is building binary packages to use with C++ so that you do not need to install a rust compiler.
These binaries can be found by clicking on the last
[successful build of the master branch](https://github.com/sixtyfpsui/sixtyfps/actions?query=workflow%3ACI+is%3Asuccess+branch%3Amaster)
and downloading the `cpp_bin` artifact.

After extracting the artifact you can place the `lib` directory into your `CMAKE_PREFIX_PATH` and `find_package(SixtyFPS)` should succeed
in locating the package.

## Usage via CMake

A typical example looks like this:

```cmake
cmake_minimum_required(VERSION 3.16)
project(my_application LANGUAGES CXX)

# Note: Use find_package(SixtyFPS) instead of the following three commands, if you prefer the package
# approach.
include(FetchContent)
FetchContent_Declare(
    SixtyFPS
    GIT_REPOSITORY https://github.com/sixtyfpsui/sixtyfps.git
    GIT_TAG v0.1.0
    SOURCE_SUBDIR api/sixtyfps-cpp
)
FetchContent_MakeAvailable(SixtyFPS)

add_executable(my_application main.cpp)
target_link_libraries(my_application PRIVATE SixtyFPS::SixtyFPS)
sixtyfps_target_60_sources(my_application my_application_ui.60)
```

The `sixtyfps_target_60_sources` cmake command allows you to add .60 files to your build. Finally it is
necessary to link your executable or library against the `SixtyFPS::SixtyFPS` target.

## Tutorial

If you are new to SixtyFPS, then you may be interested in reading our walk-through [SixtyFPS Memory Game Tutorial Tutorial](../tutorial/cpp).
It will guide you through the `.60` mark-up language and the C++ API by building a little memory game.
