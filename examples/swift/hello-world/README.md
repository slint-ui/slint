# Hello World — Swift + Slint

A minimal example showing how to use Slint from Swift via the interpreter API.

## Prerequisites

- Rust toolchain
- Swift 6.2+

## Build and Run

```sh
# 1. Build the Rust static library with interpreter support
cargo build --lib -p slint-swift --features interpreter

# 2. Build the Swift package
cd api/swift && swift build && cd ../..

# 3. Run the example
swift -I api/swift/.build/debug -L api/swift/.build/debug \
  -lSlint -lSlintInterpreter -lSlintCBridge \
  examples/swift/hello-world/main.swift
```
