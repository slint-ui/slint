<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Getting Started

This tutorial assumes that you are somewhat familiar with Rust. We recommend using [rust-analyzer](https://rust-analyzer.github.io) and [our editor integrations for `.slint` files](https://github.com/slint-ui/slint/tree/master/editors) for following this tutorial.

Slint has an application template you can use to create a project with dependencies already set up that follows recommended best practices.

Before using the template, install `[cargo-generate](https://github.com/cargo-generate/cargo-generate)`:

```sh
cargo install cargo-generate
```

Use the template to create a new project with the following command:


```sh
cargo generate --git https://github.com/slint-ui/slint-rust-template --name memory
cd memory
```

Replace the contents of `src/main.rs` with the hello world program from the [Slint documentation](https://slint.dev/docs/rust/slint/):

```rust,noplayground
{{#include main_initial.rs:main}}
```

Run the example with `cargo run` and a window appears with the green "Hello World" greeting.

![Screenshot of an initial tutorial app showing Hello World](https://slint.dev/blog/memory-game-tutorial/getting-started.png "Hello World")
