# Installing and Building with CMake

SixtyFPS comes with a CMake integration that automates the compilation step of the `.60` markup language files and
offers a CMake target for convenient linkage.

*Note*: We recommend using the Ninja generator of CMake for the most efficient build and `.60` dependency tracking.
You can select the CMake Ninja backend by passing `-GNinja` or setting the `CMAKE_GENERATOR` environment variable to `Ninja`.

## Building from Sources

The recommended and most flexible way to use the C++ API is to build SixtyFPS from sources.

First you need to install the prerequisites:

* Install Rust by following the [Rust Getting Started Guide](https://www.rust-lang.org/learn/get-started). Once this is done,
  you should have the ```rustc``` compiler and the ```cargo``` build system installed in your path.
* **cmake** (3.16 or newer)
* A C++ compiler that supports C++17 (e.g., **MSVC 2019** on Windows)

You can include SixtyFPS in your CMake project using CMake's [`FetchContent`](https://cmake.org/cmake/help/latest/module/FetchContent.html) feature.
Insert the following snippet into your `CMakeLists.txt` to make CMake download the latest release, compile it and make the CMake integration available:

```cmake
include(FetchContent)
FetchContent_Declare(
    SixtyFPS
    GIT_REPOSITORY https://github.com/sixtyfpsui/sixtyfps.git
    GIT_TAG v0.1.2
    SOURCE_SUBDIR api/sixtyfps-cpp
)
FetchContent_MakeAvailable(SixtyFPS)
```

If you prefer to treat SixtyFPS as an external CMake package, then you can also build SixtyFPS from source like a regular
CMake project, install it into a prefix directory of your choice and use `find_package(SixtyFPS)` in your `CMakeLists.txt`.

### Features

The SixtyFPS run-time library supports different features that can be toggled. You might want to enable a feature that is
not enabled by default but that is revelant for you, or you may want to disable a feature that you know you do not need and
therefore reduce the size of the resulting library.

The CMake configure step offers CMake options for various feature that are all prefixed with `SIXTYFPS_FEATURE_`. For example
you can enable support for the Wayland windowing system on Linux by enabling the `SIXTYFPS_FEATURE_WAYLAND` feature. There are
different ways of toggling CMake options. For example on the command line using the `-D` parameter:

   `cmake -DSIXTYFPS_FEATURE_WAYLAND=ON ...`

Alternatively, after the configure step you can use `cmake-gui` or `ccmake` on the build directory for a list of all features
and their description.

This works when compiling SixtyFPS as a package, using `cmake --build` and `cmake --install`, or when including SixtyFPS
using `FetchContent`.

### Cross-compiling

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

<!-- cSpell:disable -->
```sh
rustup target add aarch64-unknown-linux-gnu
```
<!-- cSpell:enable -->

Set up the environment and build:

<!-- cSpell:disable -->
```sh
. /path/to/yocto/sdk/environment-setup-cortexa53-crypto-poky-linux
cd sixtyfps
mkdir build
cd build
cmake -DRust_CARGO_TARGET=aarch64-unknown-linux-gnu -DCMAKE_INSTALL_PREFIX=/sixtyfps/install/path ..
cmake --build .
cmake --install .
```
<!-- cSpell:enable -->

## Binary Packages

The SixtyFPS continuous integration system is building binary packages to use with C++ so that you do not need to install a rust compiler.
These binaries can be found by clicking on the last
[successful build of the master branch](https://github.com/sixtyfpsui/sixtyfps/actions?query=workflow%3ACI+is%3Asuccess+branch%3Amaster)
and downloading the `cpp_bin` artifact.

After extracting the artifact you can place the `lib` directory into your `CMAKE_PREFIX_PATH` and `find_package(SixtyFPS)` should succeed
in locating the package.

In the next section you will learn how to use the installed library in your application
and load `.60` UI files.
