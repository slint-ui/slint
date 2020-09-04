# Examples

These examples demonstrate the main features of SixtyFPS and how to use them in different language environments.

Example | Source File | `.60` Design | Description
--- | --- | --- | ---
`cpptest` | [`main.cpp`](./cpptest/main.cpp)  | [`hello.60`](./cpptest/hello.60) | A minimal example to show a `.60` design in a window.
`rusttest2` | [`main.rs`](./rusttest2/src/main.rs) | [`hello.60`](./cpptest/hello.60) | A minimal example to show a `.60` design in a window.

Demo | `.60` Design | Rust Source | C++ Source | Description
--- | --- | --- | --- | ---
`printerdemo`| [`ui.60`](./printerdemo/ui/printerdemo.60) | [`main.rs`](./printerdemo/rust/main.rs) | [`main.cpp`](./printerdemo/cpp/main.cpp) | A fictional user interface for the touch screen of a printer
`gallery` |[`gallery.60`](./gallery/gallery.60) | [`main.rs`](./gallery/main.rs) | [`main.cpp`](./gallery/main.cpp) | A gallery of different widgets

## Running the Rust Examples

You can run the examples either by going into the rust sub-folder and use `cargo run`, for example:

```sh
cd examples/printerdemo/rust
cargo run
```

or you can run them from anywhere in the Cargo workspace by name:

```sh
cargo run --bin printerdemo
```

## Running the C++ Examples

The C++ API comes with a CMake integration, which needs to be built first:

```sh
cargo xtask cmake
```

This creates CMake configuration files in the `target/debug` folder
(or `target/release` if you run `cargo xtask cmake --release`).

Then, from another directory, you can run cmake in the `cpp` folder of an example:

```
cd examples/printerdemo/cpp
mkdir build
cd build
cmake -DCMAKE_PREFIX_PATH=../../../../target/debug/ ..
cmake --build .
./printerdemo
```
