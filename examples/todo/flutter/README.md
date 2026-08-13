# Todo, in Dart

The todo example driven from Dart, shown inside a Flutter application through
[`slint_flutter`](../../../api/flutter/slint_flutter).
It uses a generated `MainWindow` wrapper, the same compiled component the Rust
version gets from `slint::include_modules!()`.
The UI file lives at [`lib/ui/todo.slint`](lib/ui/todo.slint) so `build_runner`
can emit the typed API, and the same file is bundled as a Flutter asset so a
packaged app can compile it at runtime with `MainWindow.loadSource`.

Build the native library, generate the typed wrapper and the platform runner
for the platform you want, and run:

```sh
cargo build --release -p slint-dart
fvm dart run build_runner build --delete-conflicting-outputs
fvm flutter create --platforms=macos --project-name=todo .
fvm flutter run -d macos
```

Use `linux` or `windows` in place of `macos` as needed.
The runner directories are generated, not committed.
Keep the generator running while you edit the `.slint` file:

```sh
fvm dart run build_runner watch --delete-conflicting-outputs
```
