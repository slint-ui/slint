# Todo, in Dart

The todo example driven from Dart, shown inside a Flutter application through
[`slint_flutter`](../../../api/flutter/slint_flutter).
It loads the same UI as the Rust, C++ and Node.js versions from
[`assets/ui/todo.slint`](assets/ui/todo.slint).

Build the native library, generate the platform runner for the platform you
want, and run:

```sh
cargo build --release -p slint-dart
fvm flutter create --platforms=macos --project-name=todo .
fvm flutter run -d macos
```

Use `linux` or `windows` in place of `macos` as needed. The runner directories
are generated, not committed.
