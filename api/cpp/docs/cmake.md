# Installing or Building with CMake

Slint comes with a CMake integration that automates the compilation step of the `.slint` markup language files and
offers a CMake target for convenient linkage.

*Note*: We recommend using the Ninja generator of CMake for the most efficient build and `.slint` dependency tracking.
You can select the CMake Ninja backend by passing `-GNinja` or setting the `CMAKE_GENERATOR` environment variable to `Ninja`.

## Binary Packages

We also provide binary packages of Slint for use with C++, which eliminates the need to have Rust installed in your development environment.

You can download one of our pre-built binaries for Linux or Windows on x86-64 architectures:

1. Open <https://github.com/slint-ui/slint/releases>
2. Click on the latest release
3. From "Assets" download either `slint-cpp-XXX-Linux-x86_64.tar.gz` for a Linux archive
   or `slint-cpp-XXX-win64.exe` for a Windows installer. ("XXX" refers to the version of the latest release)
4. Uncompress the downloaded archive or run the installer.

After extracting the artifact or running the installer, you can place the `lib` sub-directory into your `CMAKE_PREFIX_PATH` and `find_package(Slint)` should succeed in locating the package. 
You also need to place the `lib` sub-directory in the `PATH` envionment variable on Windows, and the `LD_LIBRARY_PATH` on Linux so that
the DLL can be found at runtime.

In the next section you will learn how to use the installed library in your application
and load `.slint` UI files.

## Building from Sources

The recommended and most flexible way to use the C++ API is to build Slint from sources.

First you need to install the prerequisites:

* Install Rust by following the [Rust Getting Started Guide](https://www.rust-lang.org/learn/get-started). If you already
  have Rust installed, make sure that it's at least version 1.60 or newer. You can check which version you have installed
  by running `rustc --version`. Once this is done, you should have the ```rustc``` compiler and the ```cargo``` build system installed in your path.
* **[cmake](https://cmake.org/download/)** (3.19 or newer)
* A C++ compiler that supports C++20 (e.g., **MSVC 2019 16.6** on Windows)

You can include Slint in your CMake project using CMake's [`FetchContent`](https://cmake.org/cmake/help/latest/module/FetchContent.html) feature.
Insert the following snippet into your `CMakeLists.txt` to make CMake download the latest release, compile it and make the CMake integration available:

```cmake
include(FetchContent)
FetchContent_Declare(
    Slint
    GIT_REPOSITORY https://github.com/slint-ui/slint.git
    GIT_TAG release/0.3
    SOURCE_SUBDIR api/cpp
)
FetchContent_MakeAvailable(Slint)
```

If you prefer to treat Slint as an external CMake package, then you can also build Slint from source like a regular
CMake project, install it into a prefix directory of your choice and use `find_package(Slint)` in your `CMakeLists.txt`.

### Features

The Slint run-time library supports different features that can be toggled. You might want to enable a feature that is
not enabled by default but that is revelant for you, or you may want to disable a feature that you know you do not need and
therefore reduce the size of the resulting library.

The CMake configure step offers CMake options for various feature that are all prefixed with `SLINT_FEATURE_`. For example
you can make a build that exclusively supports Wayland on Linux by enabling the `SLINT_FEATURE_BACKEND_WINIT_WAYLAND` feature and turning
off `SLINT_FEATURE_BACKEND_WINIT`. There are different ways of toggling CMake options. For example on the command line using the `-D` parameter:

   `cmake -DSLINT_FEATURE_BACKEND_WINIT=OFF -DSLINT_FEATURE_BACKEND_WINIT_WAYLAND=ON ...`

Alternatively, after the configure step you can use `cmake-gui` or `ccmake` on the build directory for a list of all features
and their description.

This works when compiling Slint as a package, using `cmake --build` and `cmake --install`, or when including Slint
using `FetchContent`.

### Backends

Slint needs a backend that will act as liaison between Slint and the OS.
By default, Slint will use the Qt backend, if Qt is installed, otherwise, it
will use [Winit](https://crates.io/crates/winit) with [Femtovg](https://crates.io/crates/femtovg).
Both backends are compiled in. If you want to not compile one of these you need
to disable the `SLINT_FEATURE_BACKEND_WINIT` and `SLINT_FEATURE_RENDERER_WINIT_FEMTOVG` features and enable
the backend and renderer features you choose.

If you enable the Winit backend, you need to also include a renderer.
`SLINT_FEATURE_RENDERER_WINIT_FEMTOVG` is the only stable renderer, the other ones are experimental
It is also possible to select the backend and renderer at runtime when several
are enabled, using the `SLINT_BACKEND`  environment variable.
 * `SLINT_BACKEND=Qt` selects the Qt backend
 * `SLINT_BACKEND=winit` selects the winit backend
 * `SLINT_BACKEND=winit-femtovg` selects the winit backend with the femtovg renderer
 * `SLINT_BACKEND=winit-skia` selects the winit backend with the skia renderer
 * `SLINT_BACKEND=winit-software` selects the winit backend with the software renderer
If the selected backend is not available, the default will be used.

### Cross-compiling

It is possible to cross-compile Slint to a different target architecture when building with CMake. In order to complete
that, you need to make sure that your CMake setup is ready for cross-compilation. You can find more information about
how to set this up in the [upstream CMake documentation](https://cmake.org/cmake/help/latest/manual/cmake-toolchains.7.html#cross-compiling).
If you are building against a Yocto SDK, it is sufficient to source the SDK's environment setup file.

Since Slint is implemented using the Rust programming language, you need to determine which Rust target
matches the target architecture that you're compiling to. Please consult the [upstream Rust documentation](https://doc.rust-lang.org/nightly/rustc/platform-support.html) to find the correct target name. Now you need to install the Rust toolchain:

```sh
rustup target add <target-name>
```

Then you're ready to invoke CMake and you need to add `-DRust_CARGO_TARGET=<target name>` to the CMake command line.
This ensures that the Slint library is built for the correct architecture.

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
cd slint
mkdir build
cd build
cmake -DRust_CARGO_TARGET=aarch64-unknown-linux-gnu -DCMAKE_INSTALL_PREFIX=/slint/install/path ..
cmake --build .
cmake --install .
```
