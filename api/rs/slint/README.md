
# Slint

[![Crates.io](https://img.shields.io/crates/v/slint)](https://crates.io/crates/slint)
[![Docs.rs](https://docs.rs/slint/badge.svg)](https://docs.rs/slint)

[Slint](https://slint.rs) is a Rust GUI toolkit to build native user interfaces for any device.
It scales from desktop apps to mobile apps on Android and iOS,
and down to embedded Linux and microcontrollers such as STM32, ESP32, and RP2040.
It also runs on the web.

You describe the UI in the `.slint` language, and this crate connects it to your Rust code.
The [crate documentation](https://slint.dev/docs/rust/slint/) covers the full API, with more examples.

## Hello World

Add Slint to your `Cargo.toml`:

```toml
[dependencies]
slint = "1.19.0"
```

Then write the UI and run it in your `main.rs`:

```rust,no_run
slint::slint!{
    #[edition = "1.19"]
    export component HelloWorld {
        Text {
            text: "hello world";
            color: green;
        }
    }
}
fn main() {
    HelloWorld::new().unwrap().run().unwrap();
}
```

## Start From the Template

The [Rust template](https://github.com/slint-ui/slint-rust-template) is a minimal Slint app to start your project from:

1. Download and extract the [ZIP archive of the template](https://github.com/slint-ui/slint-rust-template/archive/refs/heads/main.zip).
2. Rename the extracted directory and change into it:
   ```bash
   mv slint-rust-template-main my-project
   cd my-project
   ```
3. Build and run it with `cargo run`.
