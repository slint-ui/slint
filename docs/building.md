# SixtyFPS build guide

This page explain how to build and test sixtyfps.

## Testing

Most of the project is written in rust, and compiling and running the test can
done with cargo.

```
cargo build
cargo test
```

Note that `cargo test` does not work without first calling `cargo build` because the
C++ tests will not find the dynamic library

### Run the rusttest examples

There are two examples written in rust:

The first one uses the sixtyfps! macro

```
cargo run --bin rusttest
```

The second one uses an external .60 file

```
cargo run --bin rusttest2
```

## The C++ example

First, it is required to build the cmakelists.txt

```
cargo xtask cmake
```

Then, from another directory, one can run cmake and make

```
cmake /path/to/sixtyfps/example/cpptest
make
./hello
```

## Running the viewer

One the viewer on a few .60 files, for example:

```
cargo run --bin viewer -- examples/cpptest/hello.60
cargo run --bin viewer -- tests/cases/plusminus.60
```

