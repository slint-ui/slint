# Slint for Dart and Flutter

Write the user interface in `.slint`, the logic in Dart.

```dart
import 'package:slint/slint.dart';

void main() {
  final app = loadFile('ui/todo.slint');
  app['todo-model'] = [
    {'title': 'Write the Dart part', 'checked': false},
  ];
  app.setCallback('todo-added', (args) {
    final items = app['todo-model']! as List<Object?>;
    app['todo-model'] = [...items, {'title': args[0], 'checked': false}];
  });
  app.run();
}
```

Two packages live here:

| Package | What it is |
| --- | --- |
| [`slint`](./slint/pubspec.yaml) | The binding itself. Pure Dart, `dart:ffi`, no Flutter dependency. |
| [`slint_flutter`](./slint_flutter) | A `SlintView` widget that renders a Slint UI inside a Flutter app. |

## Building

The Dart side talks to `libslint_dart`, a small C ABI over `slint-interpreter`:

```sh
cargo build --release -p slint-dart
```

`package:slint` finds the library by looking at `SLINT_DART_LIBRARY` first, then
walking up from the working directory and from the running script for a
`target/release` or `target/debug` copy, and finally asking the platform loader.
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
native event loop has to live. You get a `SlintException` saying so rather than
a crash. On Linux and Windows it works.

### Slint draws into a buffer you own

[`SlintSurface`](./slint/lib/src/embedded.dart) installs Slint's software renderer
and hands you the frame as pixels. There is no event loop and no thread
requirement, so it works everywhere — including inside Flutter, which is what
`slint_flutter` builds on:

```dart
import 'package:slint_flutter/slint_flutter.dart';

// Inside a widget tree:
SlintView(load: () => loadFile('ui/todo.slint'))
```

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
