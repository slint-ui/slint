# SixtyFPS-rs

[![Crates.io](https://img.shields.io/crates/v/sixtyfps)](https://crates.io/crates/sixtyfps)
[![Docs.rs](https://docs.rs/sixtyfps/badge.svg)](https://docs.rs/sixtyfps)

# A Rust UI toolkit

[SixtyFPS](https://sixtyfps.io/) is a UI toolkit that supports different programming languages.
SixtyFPS-rs is the Rust API to interact with a SixtyFPS UI design from Rust.

The complete Rust documentation can be viewed online at https://sixtyfps.io/docs/rust/sixtyfps/.

## Getting Started

The [crate documentation](https://sixtyfps.io/docs/rust/sixtyfps/) shows how to use this crate.

### Hello World

The most basic "Hello world" application can be achieved with a few lines of code:

In your `Cargo.toml` add:

```toml
[dependencies]
sixtyfps = "0.1.6"
```

And in your `main.rs`:

```rust
sixtyfps::sixtyfps!{
    HelloWorld := Window {
        Text {
            text: "hello world";
            color: green;
        }
    }
}
fn main() {
    HelloWorld::new().run();
}
```

The [`sixtyfps` crate documentation](https://sixtyfps.io/docs/rust/sixtyfps/)
contains more advanced examples and alternative ways to use this crate.

To quickly get started, you can use the [Template Repository](https://github.com/sixtyfpsui/sixtyfps-rust-template) with
the code of a minimal application using SixtyFPS that can be used as a starting point to your program.

```bash
cargo install cargo-generate
cargo generate --git https://github.com/sixtyfpsui/sixtyfps-rust-template
```

## More examples

You can quickly try out the [examples](/examples) by cloning this repo and running them with `cargo run`

```sh
# Runs the "printerdemo" example
cargo run --release --bin printerdemo
```

### Minimum Supported Rust Version

 This crate's minimum supported `rustc` version is `1.56.0`.
