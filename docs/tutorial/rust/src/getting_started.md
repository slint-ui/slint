# Getting Started

We assume that you are a somewhat familiar with Rust, and that you know how to create a Rust application with
`cargo new`. The [Rust Getting Started Guide](https://www.rust-lang.org/learn/get-started) can help you get set up.

First, we create a new cargo project:

```sh
cargo new memory
cd memory
```

Then we edit `Cargo.toml` to add the sixtyfps dependency:

```toml
[package]
#...
edition = "2021"

[dependencies]
sixtyfps = "0.1.6"
```

Finally we copy the hello world program from the [Slint documentation](https://sixtyfps.io/docs/rust/sixtyfps/) into our `src/main.rs`:

```rust,noplayground
{{#include main_initial.rs:main}}
```

We run this example with `cargo run` and a window will appear with the green "Hello World" greeting.

![Screenshot of initial tutorial app showing Hello World](https://sixtyfps.io/blog/memory-game-tutorial/getting-started.png "Hello World")
