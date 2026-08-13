# Todo, in Dart

The todo example driven from Dart, shown inside a Flutter application through
[`slint_flutter`](../../../api/flutter/slint_flutter). It uses the same
[`todo.slint`](../ui/todo.slint) as the Rust, C++ and Node.js versions.

Build the native library, generate the platform runner for the platform you
want, and run:

```sh
cargo build --release -p slint-dart
fvm flutter create --platforms=macos --project-name=todo .
fvm flutter run -d macos
```

Use `linux` or `windows` in place of `macos` as needed. The runner directories
are generated, not committed.
