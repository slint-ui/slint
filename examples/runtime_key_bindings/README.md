# Runtime Key Bindings

This example demonstrates creating and assigning keyboard shortcuts at runtime
using `Keys::from_parts`.

Key bindings are normally defined at compile time with `@keys(...)` in `.slint`
files. With `Keys::from_parts`, you can create them at runtime — useful for
user-configurable shortcuts.

It also shows how to capture a key event and convert it into a `Keys` value,
enabling graphical shortcut configuration.

The chosen shortcut is persisted to a `user_shortcut.conf` file using
`Keys::to_parts` and restored on the next launch with `Keys::from_parts`.

## Config file format

```
// runtime_key_bindings - user shortcuts.
// One per line: <action> <part> <part> ...
user Control Shift? z
zoom-in Control Shift? +
scroll-down Control %20
```

The first token of a line is the action name; the rest are the parts that
`Keys::to_parts` produced, and they go straight back into `Keys::from_parts`.
Resolving the action by position rather than with a delimiter means the action
name can never be confused with a part.

The parts themselves are percent-encoded, because `to_parts` emits the key as the
character the shortcut stores rather than as a key name. A part can therefore be a
space, a control character (Tab, Return) or a private-use codepoint (the function
keys) — none of which a plain text file can hold literally. Anything outside
printable ASCII, plus `%` itself, is written as the `%XX` UTF-8 bytes of the
character, so `Ctrl+Space` is `Control %20` and F5 is `%EF%9C%88`. Ordinary
shortcuts are unaffected and stay readable.

Two properties fall out of that encoding:

- Whitespace is a safe separator, since every whitespace character is escaped.
- `//` is a safe comment marker, because an encoded part is either a modifier
  token or a single encoded character — never two slashes. `#` would *not* work:
  it is the `Hash` key, which is printable ASCII and so survives encoding
  unchanged.

## Rust

```bash
cargo run --manifest-path rust/Cargo.toml
```

The same API is available in C++ (`slint::Keys::from_parts` and
`slint::Keys::to_parts`); a C++ version of this example is not kept here to avoid
maintaining the same demo twice. See
[PR #12212](https://github.com/slint-ui/slint/pull/12212) for a C++ variant.
