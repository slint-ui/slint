
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

The [Rust template](https://github.com/slint-ui/slint-rust-template) is a minimal Slint app.
To start your project from it, follow these steps:

1. Download and extract the [ZIP archive of the template](https://github.com/slint-ui/slint-rust-template/archive/refs/heads/main.zip).
2. Rename the extracted directory and change into it:
   ```bash
   mv slint-rust-template-main my-project
   cd my-project
   ```
3. Build and run it with `cargo run`.

## Examples

Browse the [examples](https://github.com/slint-ui/slint/tree/master/examples) to see what you can build with Slint.

## License

Slint is open source and available under three licenses: you pick the one that fits your app.

- If your app is open source, use the [GNU GPLv3](https://github.com/slint-ui/slint/blob/master/LICENSES/GPL-3.0-only.txt), a widely used, OSI-approved open-source license.
  It covers any platform, including embedded devices.
- If your app is closed source and runs on desktop, mobile, or the web,
  use the [Royalty-free License](https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md).
  It's free of charge, and only asks you to credit Slint, with the `AboutSlint` widget or the Slint badge on your website.
- If your app is closed source and runs on an embedded device,
  buy a [commercial license](https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Software-3.0.md).
  See the [pricing](https://slint.dev/pricing).

Your own code can stay under a permissive license such as MIT or Apache-2.0.
Anyone who forks it chooses their own Slint license, so they can reuse it in a closed-source app.
The [Licensing FAQ](https://github.com/slint-ui/slint/blob/master/FAQ.md#licensing) covers the details.
