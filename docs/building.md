<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
<!-- cSpell: ignore xkbcommon fontconfig vcpkg DCMAKE RUSTDOCFLAGS cppdocs frontends -->
# Slint Build Guide

This page explains how to build and test Slint.

## Prerequisites

### Installing Rust

Install Rust by following the [Rust Getting Started Guide](https://www.rust-lang.org/learn/get-started). If you already
have Rust installed, make sure that it's at least version 1.85 or newer. You can check which version you have installed
by running `rustc --version`.

Once this is done, you should have the `rustc` compiler and the `cargo` build system installed in your path.

### Dependencies

- **FFMPEG**

- **Skia** (only few available binaries):

<center>

| Platform                          | Binaries                                           |
| --------------------------------- | -------------------------------------------------- |
| Windows                           | `x86_64-pc-windows-msvc`                           |
| Linux Ubuntu 16+<br />CentOS 7, 8 | `x86_64-unknown-linux-gnu`                         |
| macOS                             | `x86_64-apple-darwin`                              |
| Android                           | `aarch64-linux-android`<br/>`x86_64-linux-android` |
| iOS                               | `aarch64-apple-ios`<br/>`x86_64-apple-ios`         |
| WebAssembly                       | `wasm32-unknown-emscripten`                        |

</center>

- Use Skia capable toolchain `rustup default stable-x86_64-pc-windows-msvc`

### Linux

For Linux a few additional packages beyond the usual build essentials are needed for development and running apps:

- xcb (`libxcb-shape0-dev` `libxcb-xfixes0-dev` on debian based distributions)
- xkbcommon (`libxkbcommon-dev` on debian based distributions)
- fontconfig library (`libfontconfig-dev` on debian based distributions)
- (optional) Qt will be used when `qmake` is found in `PATH`
- FFMPEG library `clang` `libavcodec-dev` `libavformat-dev` `libavutil-dev` `libavfilter-dev` `libavdevice-dev` `libasound2-dev` `pkg-config`
- openssl (`libssl-dev` on debian based distributions)

`xcb` and `xcbcommon` aren't needed if you are only using `backend-winit-wayland` without `backend-winit-x11`.

### macOS

- Make sure the "Xcode Command Line Tools" are installed: `xcode-select --install`
- (optional) Qt will be used when `qmake` is found in `PATH`
- FFMPEG `brew install pkg-config ffmpeg`

### Windows

- See [System Link](#symlinks-in-the-repository-windows)
- Make sure the MSVC Build Tools are installed: `winget install Microsoft.VisualStudio.2022.BuildTools`
- (optional) make sure Qt is installed and `qmake` is in the `Path`
- FFMPEG

  - Option 1:

    - install [vcpkg](https://github.com/microsoft/vcpkg#quick-start-windows)
    - `vcpkg install ffmpeg --triplet x64-windows`
    - Make sure `VCPKG_ROOT` is set to where `vcpkg` is installed
    - Make sure `%VCPKG_ROOT%\installed\x64-windows\bin` is in your path

  - Option 2:
    - Download FFMPEG 4.4 shared and extract (<https://github.com/BtbN/FFmpeg-Builds/releases/tag/latest>)
    - Add FFMPEG to path: `*\ffmpeg\bin` `*\ffmpeg\include\libavutil` `*\ffmpeg\lib`

### C++ API (optional)

To use Slint from C++, the following extra dependencies are needed:

- **[cmake](https://cmake.org/download/)** (3.21 or newer)
- **[Ninja](https://ninja-build.org)** (Optional, or remove the `-GNinja` when invoking `cmake`)
- A C++ compiler that supports C++20 (e.g., **MSVC 2022 17.3** on Windows, or **GCC 10**)

### Node.js API (optional)

To use Slint from Node.js, the following extra dependencies are needed.

- **[Node.js](https://nodejs.org/en/)** (including npm) At this time you will need to use the version 16.
- **[Python](https://www.python.org)**

### Symlinks in the repository (Windows)

The Slint repository makes use of symbolic links to avoid duplication.
On Windows, this require to set a git config before cloning, and have Windows
switched in developer mode or do the git clone as Administrator

```sh
git clone -c core.symlinks=true https://github.com/slint-ui/slint
```

More info: <https://github.com/git-for-windows/git/wiki/Symbolic-Links>

## Building and Testing

Most of the project is written in Rust, and compiling and running the test can
be done with cargo.

```sh
cargo build
cargo test
```

**Important:** Note that `cargo test` does not work without first calling `cargo build` because the
the required dynamic library won't be found.

### Building workspace

To build all examples install the entire workplace to executables
(excluding [UEFI-demo](https://github.com/slint-ui/slint/tree/master/examples/uefi-demo) - different target)

```sh
cargo build --workspace --exclude uefi-demo --release
```

### C++ Tests

The C++ tests are contained in the `test-driver-cpp` crate. It requires the Slint C++ library to be built,
which isn't done by default. Build it explicitly before running the tests:

```sh
cargo build --lib -p slint-cpp
cargo test -p test-driver-cpp
```

### Node.js Tests

The Node.js tests are contained in the `test-driver-nodejs` crate. The node integration will be run
automatically when running the tests:

```sh
cargo build -p test-driver-nodejs
```

### More Info About Tests

For more details about the tests and how they are implemented, see [testing.md](./testing.md).

## C++ API Build

The Slint C++ API is implemented as a normal cmake build:

```sh
mkdir cppbuild && cd cppbuild
cmake -GNinja ..
cmake --build .
```

The build will call cargo to build the Rust libraries, and build the examples.
To install the libraries and everything you need, use:

```sh
cmake --install .
```

You can pass `-DCMAKE_INSTALL_PREFIX` in the first cmake command in order to choose the installation location.

### Node.js API Build

The Slint Node.js API is implemented as npm build. You can build it locally using the following command line:

```sh
cd api/node
npm install
```

To build your own project against the Git version of the Slint Node.js API, add the path to the `api/node` folder
in the dependencies section of your `package.json`:

```json
    "dependencies": {
        "slint-ui": "/path/to/api/node"
    },
```

## Cross-Compiling

Slint can be cross-compiled to different target architectures and environments. For the Rust build we
have had a good experience using [`cross`](https://github.com/rust-embedded/cross). For convenience we're
including a `Cross.toml` configuration file for `cross` in the source tree along with Docker containers that
allow targeting a Debian ARMv7 and ARMv8 based Distribution with X11 or Wayland, out of the box. If you want to use the default Cross containers or your own, make sure the [dependencies](#prerequisites) are in the container.

This includes for example the Raspberry Pi OS. Using the following steps you can run the examples on a
pi:

```sh
cross build --target armv7-unknown-linux-gnueabihf --workspace --exclude slint-node --exclude pyslint --release
scp target/armv7-unknown-linux-gnueabihf/release/printerdemo pi@raspberrypi.local:.
```

Finally on a shell on the Pi:

```sh
DISPLAY=:0 ./printerdemo
```

## Examples

See the [examples](/examples) folder for examples to build, run and test.

## Running the Viewer

Slint also includes a viewer tool that can load `.slint` files dynamically at run-time. It's a
cargo-integrated binary and can be run directly on the `.slint` files, for example:

```sh
cargo run --release --bin slint-viewer -- demos/printerdemo/ui/printerdemo.slint
```

## Generating the Documentation

The Slint documentation consists of five parts:

- The quickstart guide
- The Rust API documentation
- The C++ API documentation
- The Node.js API documentation
- The DSL documentation

The quickstart guide is part of the DSL documentation.

### Quickstart and DSL docs

See [astro/README.md](astro/README.md)

### Rust API docs

Run the following command to generate the documentation using rustdoc in the `target/doc/` sub-folder:

```sh
RUSTDOCFLAGS="--html-in-header=$PWD/docs/astro/src/utils/slint-docs-preview.html --html-in-header=$PWD/docs/astro/src/utils/slint-docs-highlight.html" cargo doc --package slint --no-deps --features slint/document-features,slint/log
```

Note: `--html-in-header` arguments passed to rustdoc via `RUSTDOCFLAGS` are used to enable syntax highlighting and live-preview for Slint example snippets.

### C++ API docs

**Prerequisites**:

- [Doxygen](https://www.doxygen.nl/download.html)

Run the following command to generate the documentation using sphinx/exhale/breathe/doxygen/myst_parser in the `target/cppdocs` sub-folder:

```sh
cargo xtask cppdocs
```

### Node.js API docs

Run the following commands from the `/api/node` sub-folder to generate the docs using [typedoc](https://typedoc.org/) in the `/api/node/docs` sub-folder:

```sh
npm install
npm run docs
```

### Building search database

We use Typesense for document search.

#### Infrastructure

* Typesense Server: The Typesense Server will hold the search index.
* Accessibility: The Typesense server must be accessible from the search bar in documentation site.
* Docker: Docker is needed to run the Typesense Docsearch Scraper.
* Typesense Docsearch Scraper: This tool will be used to index the documentation website.

#### Pre-requisites

* Install docker (<https://docs.docker.com/engine/install/>)

* Install jq

```sh
pip3 install jq
```

#### Testing Locally

* Install and start Typesense server (<https://typesense.org/docs/guide/install-typesense.html#option-2-local-machine-self-hosting>)
  * Note down the API key, the default port, and the data directory.

* Verify that the server is running
  * Replace the port below with the default port
  * It should return {"ok":true} if the server is running correctly.

```sh
curl http://localhost:8108/health
```

#### Testing on Typesense Cloud

* Create an account as per instructions (<https://typesense.org/docs/guide/install-typesense.html#option-1-typesense-cloud>)
  * Note down the API key and the hostname.

#### Creating search index

A helper script is located under `search` sub-folder that will (optionally) build the docs (currently only Slint docs), scrape the documents, and upload the search index to Typesense server.

The script accepts the following arguments

-a : API key to authenticate with Typesense Server (default: `xyz`)

-b : Build Slint docs (for testing locally set this flag ) (default: `false`)

-c : Location of config file (default: `docs/search/scraper-config.json`)

-d : Location of index.html of docs (default: `target/slintdocs/html`)

-i : Name of the search index (default: `local`)

-p : Port to access Typesense server (default: `8108`)

-r : Remote Server when using Typesense Cloud

-u : URL on which the docs will be served (default: `http://localhost:8000`)

Example when running locally

```sh
docs/search/docsearch-scraper.sh -b
```

Example when running on Typesense Cloud, where `$cluster_name` is the name of the cluster on Typesense Cloud

```sh
docs/search/docsearch-scraper.sh -a API_KEY -b -r TYPESENSE_CLOUD_HOST_NAME
```

#### Testing search functionality

Run http server

```sh
python3 -m http.server -d target/slintdocs/html
```

Open browser (<http://localhost:8000>) and use the search bar to search for content
