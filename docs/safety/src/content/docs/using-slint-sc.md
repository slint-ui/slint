---
title: Using Slint SC
description: Components included in Slint SC and use cases.
---
## Components of Slint SC

This is what is included in Slint SC:

* A Slint Compiler, from the internal `i-slint-compiler` crate
* The `slint_build` crate which provides a Rust API for the compiler.
* Individual slint language features
* Features offered by specific crates that are part of the Slint Rust library

Each of these things can have a **Usage** and a **Constraints** section.

Each feature of the language or a library can map to a Requirement ID, and have 1 or more code test-examples.

# Use Cases (ISO 26262:8 11.4.5.1)

## Using Slint SC in a project

* **ID** : UC_ADD_SLINT_TO_PROJECT
* **Input** : Cargo.toml file in the root of a rust project file tree
* **Output** : Modified Cargo.toml file that includes Slint SC as a dependency
* **Environment Constraints**: (TODO)

When Slint SC is available, it will be available as a crate on crates.io.
To add it to a project, one simply specifies Slint SC as a dependency in `Cargo.toml`.

(TODO - show example)

## Compiling a .slint file into Rust

* **ID** : UC_COMPILE_SLINT_FILE
* **Input** : a .slint file
* **Output** : Rust code that can be compiled into the final executable.
* **Environment Constraints**: (TODO)

Rust developers using Slint SC can instantiate and configure a `CompilerConfiguration` from the [`slint_build`](https://docs.slint.dev/latest/docs/rust/slint_build/) crate to compile `.slint` files into Rust.

This structure is typically created and used from a [Rust build script](https://doc.rust-lang.org/cargo/reference/build-scripts.html), `build.rs`, located in the root directory of the package. After it has the correct
values, it can be passed to `slint_build::compile_with_config`.
Here is a simple example:

```rust
fn main() {
    let mut config = slint_build::CompilerConfiguration::new();
    // [ ... ] set some values on config here
    slint_build::compile_with_config("mainFile.slint", config).unwrap();
}
```

## Constraints

The standard essentially views a **Requirement** as what the system *must do* (or a property it must have), whereas a **Constraint** is a boundary condition that *limits the solution space*.

For APIs, the Constraints might explain that some functions are experimental and can not be used safely yet. Or, that certain values passed as parameters into functions are not supported in Slint SC. In other words, certain features can only be used a certain way to be safe.

Individual Constraints can have a section each here, with a descriptive ID that begins with CON_, and a Rationale, Impact, and Mitigation.
