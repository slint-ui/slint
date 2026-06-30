# Runtime Key Bindings

This example demonstrates creating and assigning keyboard shortcuts at runtime
from Rust and C++ using `Keys::from_parts`.

Key bindings are normally defined at compile time with `@keys(...)` in `.slint`
files. With `Keys::from_parts`, you can create them at runtime — useful for
user-configurable shortcuts.

It also shows how to capture a key event and convert it into a `Keys` value,
enabling graphical shortcut configuration.

The chosen shortcut is persisted to a `user_shortcut.conf` file using
`Keys::to_parts` and restored on the next launch with `Keys::from_parts`.

## Rust

```bash
cargo run --manifest-path rust/Cargo.toml
```

## C++

Build from the top-level cmake build directory:

```bash
cd /path/to/slint/build && ninja runtime_key_bindings
./examples/runtime_key_bindings/cpp/runtime_key_bindings
```
