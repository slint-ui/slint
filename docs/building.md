# SixtyFPS build guide

This page explain how to build and test sixtyfps.

## Prerequisites

### Installing Rust

Install Rust by following the [Rust Getting Started Guide](https://www.rust-lang.org/learn/get-started).

Once this is done, you should have the ```rustc``` compiler and the ```cargo``` build system installed in your path.

## Testing

Most of the project is written in Rust, and compiling and running the test can
done with cargo.

```sh
cargo build
cargo test
```

Note that `cargo test` does not work without first calling `cargo build` because the
C++ tests will not find the dynamic library

### Run the rusttest examples

There are two examples written in rust:

The first one uses the sixtyfps! macro

```sh
cargo run --bin rusttest
```

The second one uses an external .60 file

```sh
cargo run --bin rusttest2
```

## The C++ example

The C++ API comes with a CMake integration, which needs to be built first:

```sh
cargo xtask cmake
```

This creates CMake configuration files in the `target/debug` folder
(or `target/release` if you run `cargo xtask cmake --release`).

Then, from another directory, you can run cmake

```
cmake -DCMAKE_PREFIX_PATH=/path/to/sixtyfps/target/debug /path/to/sixtyfps/example/cpptest .
cmake --build .
./hello
```

## Running the viewer

SixtyFPS also includes a viewer tool that can load `.60`files dynamically at run-time. It is a
cargo-integrated binary and can be run directly on the `.60`files, for example:

```sh
cargo run --bin viewer -- examples/cpptest/hello.60
cargo run --bin viewer -- tests/cases/plusminus.60
```

