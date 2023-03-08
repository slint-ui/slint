<!-- cSpell: ignore ccmake dslint femtovg skia winit -->

# Installing Or Building With CMake

Slint comes with a CMake integration that automates the compilation step of the `.slint` markup language files and
offers a CMake target for convenient linkage.

*Note*: We recommend using the Ninja generator of CMake for the most efficient build and `.slint` dependency tracking.
You can select the CMake Ninja backend by passing `-GNinja` or setting the `CMAKE_GENERATOR` environment variable to `Ninja`.

## Binary Packages

We offer binary packages of Slint for use with C++. These work without any Rust
development environment.

You can download one of our pre-built binaries for Linux or Windows on x86-64 architectures:

1. Open <https://github.com/slint-ui/slint/releases>
2. Click on the latest release
3. From "Assets" download either `slint-cpp-XXX-Linux-x86_64.tar.gz` for a Linux archive
   or `slint-cpp-XXX-win64.exe` for a Windows installer. ("XXX" refers to the version of the latest release)
4. Unpack the downloaded archive or run the installer.

After extracting the artifact or running the installer, you need to place the
`lib` sub-directory into your `CMAKE_PREFIX_PATH`. `find_package(Slint)` will
then be able to find Slint from within a `CMakeLists.txt` file.

At runtime you might also need to add the `lib` sub-directory to the `PATH`
environment variable on Windows or the `LD_LIBRARY_PATH` on Linux. This is
necessary to find the Slint libraries when trying to run your program.

In the next section you will learn how to use the installed library in your application
and how to work with `.slint` UI files.

## Building From Sources

The recommended and most flexible way to use the C++ API is to build Slint from
sources.

First you need to install the prerequisites:

* Install Rust by following the [Rust Getting Started Guide](https://www.rust-lang.org/learn/get-started). If you already
  have Rust installed, make sure that it's at least version 1.60 or newer. You can check which version you have installed
  by running `rustc --version`. Once this is done, you should have the ```rustc``` compiler and the ```cargo``` build system installed in your path.
* **[cmake](https://cmake.org/download/)** (3.23 or newer)
* A C++ compiler that supports C++20 (e.g., **MSVC 2019 16.6** on Windows)

You can include Slint into your CMake project using CMake's
[`FetchContent`](https://cmake.org/cmake/help/latest/module/FetchContent.html)
feature. Insert the following snippet into your `CMakeLists.txt` to make CMake
download the latest released 1.x version, compile it, and make the CMake
integration available:

```cmake
include(FetchContent)
FetchContent_Declare(
    Slint
    GIT_REPOSITORY https://github.com/slint-ui/slint.git
    GIT_TAG release/1
    SOURCE_SUBDIR api/cpp
)
FetchContent_MakeAvailable(Slint)
```

If you prefer to use Slint as an external CMake package, then you build Slint from source like a regular
CMake project, install it into a prefix directory of your choice and use `find_package(Slint)` in your `CMakeLists.txt`.

### Resource Embedding

You may change the default resource embedding the Slint compiler will use via
the `SLINT_EMBED_RESOURCES` target property, which accepts `as-paths`,
`as-contents` and `as-textures`. The latter optimizes for the Slint
software-renderer and falls back to `as-contents` if that isn't used.

Typically you will want to effect resource embedding for all targets in a
CMake project. This is done with the `DEFAULT_SLINT_EMBED_RESOURCES` option,
which initializes the `SLINT_EMBED_RESOURCES` target property.

Typically you will want to set `DEFAULT_SLINT_EMBED_RESOURCES` to `as-paths`
for debug builds as this reduces build times. When doing a release, you will
want to switch to `as-contents` to create self-contained binary that will work
on all systems.

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

### Back-Ends

Slint needs a back-end that acts as liaison between Slint and the OS. Several
back-ends can be built into the Slint library at the same time, but only one
is used a run time.

#### Compile Time Back-End Selection

By default Slint will include both the Qt and
[winit](https://crates.io/crates/winit) back-ends -- if both are detected at
compile time. You can enable or disable back-ends using the
`SLINT_FEATURE_BACKEND_` features. For example, to exclude the winit back-end,
you would disable the `SLINT_FEATURE_BACKEND_WINIT` option in your CMake
project configuration.

The winit back-end needs a renderer. `SLINT_FEATURE_RENDERER_WINIT_FEMTOVG` is
the only stable renderer, the other ones are experimental. If you disable the
`SLINT_FEATURE_BACKEND_WINIT`, you will also want to disable the renderer!

#### Run Time Back-End Selection

It's also possible to select any of the compiled in back-ends and renderer at
runtime, using the `SLINT_BACKEND` environment variable.

 * `SLINT_BACKEND=Qt` selects the Qt back-end
 * `SLINT_BACKEND=winit` selects the winit back-end
 * `SLINT_BACKEND=winit-femtovg` selects the winit back-end with the femtovg renderer
 * `SLINT_BACKEND=winit-skia` selects the winit back-end with the skia renderer
 * `SLINT_BACKEND=winit-software` selects the winit back-end with the software renderer

If the selected back-end or renderer isn't available, the default will be used
instead.

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
cmake -DRust_CARGO_TARGET=aarch64-unknown-linux-gnu -DCMAKE_INSTALL_PREFIX=/slint/install/path ...
cmake --build .
cmake --install .
```
