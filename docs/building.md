# SixtyFPS build guide

This page explains how to build and test SixtyFPS.

## Prerequisites

### Installing Rust

Install Rust by following the [Rust Getting Started Guide](https://www.rust-lang.org/learn/get-started). If you already
have Rust installed, make sure that it's at least version 1.54 or newer. You can check which version you have installed
by running `rustc --version`.

Once this is done, you should have the ```rustc``` compiler and the ```cargo``` build system installed in your path.


### Linux

For Linux a few additional packages beyond the usual build essentials are needed for development and running apps:

  * xcb (`libxcb-shape0-dev` `libxcb-xfixes0-dev` on debian based distributions)
  * xkbcommon (`libxkbcommon-dev` on debian based distributions)
  * fontconfig library (`libfontconfig-dev` on debian based distributions)

### macOS

   * Make sure the "Xcode Command Line Tools" are installed: `xcode-select --install`
### For the NodeJS backend

For the nodejs backend, the following component are needed:

* **node** (including npm)
* **python**

It would be nice if building the nodejs backend was optional, but right now it is part of the workspace.
You can still not build it by doing `cargo build --workspace --exclude sixtyfps-node`. But cargo test will fail.

### For the C++ dev (optional)

* **[cmake](https://cmake.org/download/)** (3.19 or newer)
* A C++ compiler that can do C++17 (e.g., **MSVC 2019** on Windows)

## Testing

Most of the project is written in Rust, and compiling and running the test can
done with cargo.

```sh
cargo build
cargo test
```

**Important:** Note that `cargo test` does not work without first calling `cargo build` because the
C++ tests or the nodejs tests will not find the required dynamic library otherwise

### C++ test

The C++ crate are not included in the workspace's default members, so it need to be build explicitly

```sh
cargo build --lib -p sixtyfps-cpp
cargo test --bin test-driver-cpp
```

### More info about tests

See [testing.md](./testing.md)

## C++ Build

This is just a normal cmake build.

```sh
mkdir cppbuild && cd cppbuild
cmake -GNinja ..
cmake --build .
```

The build will call cargo to build the rust libraries, and build the examples.
In order to install the libraries and everything you need, use:

```sh
cmake --install .
```

You can pass `-DCMAKE_INSTALL_PREFIX` in the first cmake command in order to choose the install location

## Cross-Compiling

SixtyFPS can be cross-compiled to different target architectures and environments. For the Rust build we
have had a good experience using [`cross`](https://github.com/rust-embedded/cross). For convenience we're
including a `Cross.toml` configuration file for `cross` in the source tree along with Docker containers that
allow targeting a Debian ARMv7 and ARMv8 based Distribution with X11 or Wayland, out of the box.

This includes for example the Raspberry Pi OS. Using the following steps you can run the examples on a
pi:

```sh
cross build --target armv7-unknown-linux-gnueabihf --workspace --exclude sixtyfps-node --release
scp target/armv7-unknown-linux-gnueabihf/release/printerdemo pi@raspberrypi.local:.
```

Finally on a shell on the Pi:

```sh
DISPLAY=:0 ./printerdemo
```

## Examples

See the [examples](/examples) folder for examples to build, run and test.

## Running the viewer

SixtyFPS also includes a viewer tool that can load `.60`files dynamically at run-time. It is a
cargo-integrated binary and can be run directly on the `.60`files, for example:

```sh
cargo run --release --bin sixtyfps-viewer -- examples/printerdemo/ui/printerdemo.60
```

## Generating the documentation

### rustdoc

The language reference has snippets in the .60 language which can be previewed by injecting
html to the documentation with the `--html-in-header` rustdoc flag.

Here is how to build the documentation to include preview of the .60 files.

```sh
RUSTDOCFLAGS="--html-in-header=$PWD/docs/resources/sixtyfps-docs-preview.html --html-in-header=$PWD/docs/resources/sixtyfps-docs-highlight.html" cargo doc --no-deps
```

### C++ doc

To generate the C++ API documentation, one need to have doxygen installed, and run this command

```sh
cargo xtask cppdocs
```
