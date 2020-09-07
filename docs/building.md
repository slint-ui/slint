# SixtyFPS build guide

This page explain how to build and test sixtyfps.

## Prerequisites

### Installing Rust

Install Rust by following the [Rust Getting Started Guide](https://www.rust-lang.org/learn/get-started).

Once this is done, you should have the ```rustc``` compiler and the ```cargo``` build system installed in your path.

### NodeJS backend

For the nodejs backend, the following component are needed:

 * **node** (including npm)
 * **python**

It would be nice if building the nodejs backend was optional, but right now it is part of the workspace.
We can still not build it by doing `cargo build --workspace --exclude sixtyfps-node`. But cargo test will fail.

### C++ dev (optional)

 * **cmake** (3.16 or newer)
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

## Examples

See the [examples](/examples) folder for examples to build, run and test.

## Running the viewer

SixtyFPS also includes a viewer tool that can load `.60`files dynamically at run-time. It is a
cargo-integrated binary and can be run directly on the `.60`files, for example:

```sh
cargo run --bin viewer -- examples/printerdemo/ui/printerdemo.60
```

