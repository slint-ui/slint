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
If `flutter run` fails because Xcode cannot access
`macos/Flutter/ephemeral/Packages/.packages/FlutterFramework`, delete `macos/`
and run `flutter create` again.
A runner generated without Swift Package Manager package references cannot be
migrated in place.
Keep the generator running while you edit the `.slint` file:

```sh
fvm dart run build_runner watch --delete-conflicting-outputs
```

Widget tests use the headless Slint backend and a debug build of `libslint_dart`
with `backend-testing`, not the release library the hook bundles for apps:

```sh
cargo build -p slint-dart --features backend-testing
fvm dart run build_runner build --delete-conflicting-outputs
SLINT_DART_LIBRARY="$PWD/../../../target/debug/libslint_dart.dylib" \
  SLINT_BACKEND=testing fvm flutter test
```

Use `libslint_dart.so` on Linux and `slint_dart.dll` on Windows.
