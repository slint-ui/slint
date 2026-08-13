# Slint for Dart and Flutter

Write the user interface in `.slint`, the logic in Dart.

```dart
import 'package:my_app/ui/counter.slint.dart';

void main() {
  final app = CounterWindow.load()
    ..statusMessage = 'Ready from Dart'
    ..onCountChanged((value) => print('Count: $value'));

  app.currentCount = 1;
  app.invokeResetCounter();
  app.run();
}
```

Two packages live here:

| Package | What it is |
| --- | --- |
| [`slint`](./slint/pubspec.yaml) | The binding itself. Pure Dart, `dart:ffi`, no Flutter dependency. |
| [`slint_flutter`](./slint_flutter) | A `SlintView` widget that renders a Slint UI inside a Flutter app. |

## Generate a Typed Dart API

Put `.slint` files under your application's `lib` directory.
For example, use `lib/ui/counter.slint`.

Add `slint` and `build_runner` to the application's `pubspec.yaml`:

```yaml
dependencies:
  slint:
    path: path/to/slint/api/flutter/slint

dev_dependencies:
  build_runner: ^2.4.9
```

Build `libslint_dart` before invoking the generator.
When the application is outside the Slint repository, set
`SLINT_DART_LIBRARY` to the built library's absolute path:

```sh
# From the Slint repository root:
cargo build --release -p slint-dart
# macOS:
export SLINT_DART_LIBRARY=/path/to/slint/target/release/libslint_dart.dylib
```

Use `libslint_dart.so` on Linux.
Set `SLINT_DART_LIBRARY` to the `slint_dart.dll` path on Windows.

Generate the Dart wrapper once:

```sh
dart run build_runner build --delete-conflicting-outputs
```

Keep the generator running during development:

```sh
dart run build_runner watch --delete-conflicting-outputs
```

Configure import search paths and the widget style in the application's
`build.yaml`:

```yaml
targets:
  $default:
    builders:
      slint|slint_generator:
        options:
          style: material
          include_paths:
            - lib/ui/includes
```

Each `include_paths` entry is relative to the application package unless it is
absolute.
Imported files must remain inside the package so `build_runner` can watch them.
The generated `load()` method uses the configured style and include paths as
its runtime defaults.
Relative include paths remain relative to the package directory where
generation ran, so the output doesn't contain a developer-specific package path.
Callers can still override its `style:` and `includePaths:` arguments.

The builder writes `lib/ui/counter.slint.dart` next to the input file.
It regenerates that wrapper when the input or one of its package-local Slint
dependencies changes.
Don't edit the generated file.

Import the wrapper through your package:

```dart
import 'package:my_app/ui/counter.slint.dart';

final app = CounterWindow.load();
```

By default, `load()` uses a `.slint` path relative to the package directory
where generation ran.
Run the application from that directory, or pass `path:` when it starts with a
different working directory.
The current binding loads Slint source at runtime, so packaged Flutter apps
must copy these files and resources into a filesystem location and pass that
location to `load(path: ...)`.
If configured include directories move too, pass their deployed locations with
`includePaths:`.
Flutter asset-bundle loading isn't available yet.

Generated Dart types use UpperCamelCase, and generated fields and methods use
lowerCamelCase:

| Slint declaration | Generated Dart API |
| --- | --- |
| `export component counter-window` | `CounterWindow.load()` |
| `in-out property <int> current-count` | `currentCount` |
| `callback count-changed(int)` | `onCountChanged(...)`, `invokeCountChanged(...)` |
| `public function reset_counter()` | `invokeResetCounter()` |

The generated wrapper keeps each exact Slint spelling for runtime lookup.
Only the public Dart identifier changes, so `current-count` and `reset_counter`
aren't reconstructed from their Dart names.

Code generation is optional.
Use `loadFile()` or `loadSource()` and the string-based `ComponentInstance` API
when the component isn't known at build time:

```dart
import 'package:slint/slint.dart';

final app = loadFile('ui/todo.slint');
app['todo-model'] = [
  {'title': 'Write the Dart part', 'checked': false},
];
app.setCallback('todo-added', (args) {
  final items = app['todo-model']! as List<Object?>;
  app['todo-model'] = [...items, {'title': args[0], 'checked': false}];
});
```

See the [`slint` code-generation example](./slint/example) for a complete package.

## Building

The Dart side talks to `libslint_dart`, a small C ABI over `slint-interpreter`:

```sh
cargo build --release -p slint-dart
```

`package:slint` finds the library by looking at `SLINT_DART_LIBRARY` first, then
walking up from the working directory, the running executable, and the running
script for a `target/release` or `target/debug` copy, and finally asking the
platform loader.
That last step is the one a packaged application uses; ship the library next to
the executable.

<!-- ponytail: no `ffiPlugin` packaging yet, so a released Flutter app has to
     place libslint_dart itself. Add the per-platform Flutter plugin build files
     that invoke cargo if these packages are ever published. -->

## Two ways to show a UI

### Slint owns the window

`ComponentInstance.run()` opens a native window and runs Slint's event loop,
the way the Python and Node.js bindings do. Use this for a plain
`dart run` application.

**This does not work on macOS**, and not inside Flutter on any platform: the
Dart VM does not run `main()` on the process main thread, which is where a
native event loop has to live.
Unwind-enabled builds report this as a `SlintException`.
The workspace release profile aborts on panic, so don't call `run()` in these
environments from a release build.
On Linux and Windows it works.

### Slint draws into a buffer you own

[`SlintSurface`](./slint/lib/src/embedded.dart) installs Slint's software renderer
and hands you the frame as pixels. There is no event loop and no thread
requirement, so it works everywhere — including inside Flutter, which is what
`slint_flutter` builds on:

```dart
import 'package:slint_flutter/slint_flutter.dart';
import 'package:my_app/ui/counter.slint.dart';

// Inside a widget tree:
SlintView(load: CounterWindow.load)
```

This direct factory form assumes the Slint source is available on the
filesystem at its generated package-relative path and the package directory is
the working directory.
For a packaged app, wrap the factory and provide the deployed path:

```dart
SlintView(load: () => CounterWindow.load(
  path: deployedSlintPath,
  includePaths: [deployedIncludesPath],
))
```

The dynamic API works here too: `SlintView(load: () => loadFile('ui/todo.slint'))`.

Driving it yourself is the same three steps `SlintView` performs each frame:

```dart
final surface = SlintSurface()..resize(800, 600, scaleFactor: 2.0);
final app = loadFile('ui/todo.slint')..show();   // after the surface exists

surface.tick();                                  // advance timers, animations
final pixels = surface.render();                 // RGBA, premultiplied, or null
surface.dispatchPointer(PointerEventKind.moved, x: 10, y: 20);
```

## Values

Values cross the boundary as JSON, which means they arrive in Dart as ordinary
data:

| Slint | Dart |
| --- | --- |
| `int`, `float`, `length`, `duration`, `percent` | `num` |
| `string` | `String` |
| `bool` | `bool` |
| `[T]` (a model) | `List` |
| a struct | `Map<String, Object?>` |
| `color`, `brush` | `String`, CSS-style: `'#00c1e2'`, `'#00c1e2ff'` |
| an enum | `String`, the variant name |
| a callback with no return | the handler returns `null` |

Models are read and written whole. Dart owns the list, and assigning the
property publishes it:

```dart
final items = [...app['todo-model']! as List<Object?>];
items.add({'title': 'One more', 'checked': false});
app['todo-model'] = items;
```

Globals work the same way through `app.global('PrinterQueue')`.

## Testing

```sh
cargo test -p slint-dart
```

The Dart tests need a backend that opens no window:

```sh
cargo build -p slint-dart --features backend-testing
cd slint && SLINT_BACKEND=testing fvm dart test
cd ../slint_flutter && SLINT_BACKEND=testing fvm flutter test
```

## Toolchain

The Dart and Flutter SDK is pinned with [FVM](https://fvm.app) in `.fvmrc`, so
every command above is available as `fvm dart …` and `fvm flutter …`. Run
`fvm install` once to fetch the pinned version.

## Examples

- [`examples/todo/flutter`](../../examples/todo/flutter) — the todo example.
- [`demos/printerdemo/flutter`](../../demos/printerdemo/flutter) — the printer demo.

## Limitations

- One `SlintSurface` per isolate: the software renderer owns a single surface.
- Everything must be used from the main isolate, which is where the Slint
  platform lives. This matches the Python and Node.js bindings.
- Images and translations are not exposed yet. `@image-url` inside `.slint`
  works; passing an image in from Dart does not.
- Generated wrappers load the original `.slint` source at runtime. Packaged
  Flutter apps must deploy it to the filesystem and pass its path to `load()`;
  Flutter asset-bundle loading isn't implemented yet.
