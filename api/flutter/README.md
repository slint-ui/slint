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

Three packages live here:

| Package | What it is |
| --- | --- |
| [`slint`](./slint/pubspec.yaml) | The binding itself. Pure Dart, no Flutter dependency: `dart:ffi` natively, WebAssembly on the web. |
| [`slint_generator`](./slint_generator) | The `build_runner` builder that turns a `.slint` file into a typed Dart API. A dev dependency only. |
| [`slint_flutter`](./slint_flutter) | A `SlintView` widget that renders a Slint UI inside a Flutter app. |

## Generate a Typed Dart API

Put `.slint` files under your application's `lib` directory.
For example, use `lib/ui/counter.slint`.

Add `slint` to the application's `pubspec.yaml`, and `slint_generator` with
`build_runner` next to it. Only the generator needs the second package, so it
stays out of the shipped application:

```yaml
dependencies:
  slint:
    path: path/to/slint/api/flutter/slint

dev_dependencies:
  build_runner: ^2.4.9
  slint_generator:
    path: path/to/slint/api/flutter/slint_generator
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
      slint_generator|slint:
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

By default the builder writes `lib/ui/counter.slint.dart` next to the input
file.
Set `output_dir` to generate the wrappers into a custom folder instead,
mirroring each source's path under `lib`:

```yaml
targets:
  $default:
    builders:
      slint_generator|slint:
        options:
          output_dir: lib/generated
```

`lib/ui/counter.slint` then becomes `lib/generated/ui/counter.slint.dart`,
imported as `package:my_app/generated/ui/counter.slint.dart`.
`output_dir` is relative to the package unless it is absolute, and must stay
inside it.
It applies to `.slint` files under the package's `lib` directory.

The builder regenerates a wrapper when its input or one of its package-local
Slint dependencies changes.
Don't edit the generated file.

Import the wrapper through your package:

```dart
import 'package:my_app/ui/counter.slint.dart';

final app = CounterWindow.load();
```

`loadSource` compiles the same component from `.slint` text already in memory,
and still returns the generated type:

```dart
final app = CounterWindow.loadSource(source);
```

By default, `load()` uses a `.slint` path relative to the package directory
where generation ran.
Run the application from that directory, or pass `path:` when it starts with a
different working directory.
The current binding loads Slint source at runtime.
`load()` reads that source from the filesystem.
Packaged Flutter apps should declare the `.slint` file as a Flutter asset,
preload it with `rootBundle.loadString` before `runApp`, and pass the text to
`loadSource`.
If configured include directories move too, pass their deployed locations with
`includePaths:`.

Generated Dart types use UpperCamelCase, and generated fields and methods use
lowerCamelCase:

| Slint declaration | Generated Dart API |
| --- | --- |
| `export component counter-window` | `CounterWindow.load()`, `CounterWindow.loadSource(source)` |
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
walking up from the working directory, the running executable, the running
script, and the linked package root for a `target/release` or `target/debug`
copy, and finally asking the platform loader.
That last step is the one a packaged application takes.

### The `dart:ffi` bindings are generated

The 37 entry points are not declared by hand on both sides. cbindgen writes a
C header from `rust/`, and ffigen turns that into
[`slint/lib/src/ffi.g.dart`](./slint/lib/src/ffi.g.dart):

```sh
cargo install cbindgen        # once
./scripts/generate_slint_dart_bindings.bash
```

`ffi.g.dart` is committed, so building the package needs neither tool. Run the
script with `--check` in CI: it regenerates into a temporary copy and fails if
the result differs, which is what stops a changed Rust signature from silently
disagreeing with Dart.

[`ffigen.yaml`](./slint/ffigen.yaml) carries a rename map so the generated
methods keep the names the rest of the package calls — those aren't derivable
from the C names, since `slint_dart_compiler_build_from_path` is `buildFromPath`
while `slint_dart_instance_show` is `instanceShow`.

[`ffi.dart`](./slint/lib/src/ffi.dart) keeps only what a generator can't
produce: finding the library at runtime, and the `takeEnvelope` / `takeString`
/ `withNativeString` helpers that convert the JSON envelope. Those three are
the single place that casts between the generated `Pointer<Char>` and
`package:ffi`'s `Pointer<Utf8>`.

### Flutter builds the library automatically

The package ships a [build hook](https://dart.dev/tools/hooks)
(`hook/build.dart`): every `flutter build` and `flutter run` that has `slint`
in its dependency graph runs it, invokes `cargo build --release -p slint-dart`,
and bundles the result into the application.
On macOS the library becomes `slint_dart.framework` inside the app bundle,
which `package:slint` finds at runtime.

A Flutter build needs the Rust toolchain (`cargo` and `rustc` on `PATH`) and
only supports the host platform; cross-compiling to another OS or architecture
is not supported yet. iOS takes the route below instead. Android is the
exception: the hook cross-compiles each ABI with `cargo-ndk` against the
Android NDK, so an Android build needs `cargo-ndk` installed and an NDK
(usually from Android Studio). The hook builds one architecture per invocation
and Flutter places each `libslint_dart.so` into the right `jniLibs` ABI
directory (`armeabi-v7a`, `arm64-v8a`, `x86_64`).
Set `SLINT_DART_LIBRARY` if you want to build the library yourself, or pin a
profile for the hook (debug builds are faster to produce):

```yaml
# pubspec.yaml of the Flutter application
hooks:
  user_defines:
    slint:
      cargo_profile: debug
```

The hook caches its result and re-runs cargo only when the Rust sources, the
crate manifest, or the workspace lockfile change.

### iOS embeds an xcframework

The build hook doesn't cross-compile, so on iOS it builds nothing and leaves
the library to an xcframework you build once:

```sh
./scripts/build_slint_dart_xcframework.bash
```

That produces `target/SlintDart.xcframework` holding `slint_dart.framework`
for the device (`arm64`), the simulator (`arm64` and `x86_64`) and macOS
(`arm64` and `x86_64`). Add it to the Runner target's *Frameworks, Libraries,
and Embedded Content* with **Embed & Sign**. These are ordinary dynamic
frameworks — the same shape the hook already bundles on macOS — so
`package:slint` opens them from the app bundle with no special case.

The slices carry only the software renderer, since the Dart binding always
draws through the embedded surface on Apple platforms. `SLINT_DART_FEATURES`,
`SLINT_DART_PROFILE`, `SLINT_DART_XCFRAMEWORK` and the usual
`IPHONEOS_DEPLOYMENT_TARGET` / `MACOSX_DEPLOYMENT_TARGET` override the feature
set, the cargo profile, the output path and the minimum OS versions.

### The web loads a WebAssembly module

A browser has no `dart:ffi`, so `package:slint` reaches the same Rust code
through WebAssembly instead: `api/flutter/rust/wasm.rs` exposes the runtime to
JavaScript with `wasm-bindgen`, and
[`backend_web.dart`](./slint/lib/src/backend_web.dart) calls it over
`dart:js_interop`. Everything above that line — properties, callbacks, the
software renderer — is the code every other platform runs.

Build the module into the application's `web/` directory:

```sh
./scripts/build_slint_dart_wasm.bash path/to/app/web
```

That writes `slint_dart.js` and `slint_dart_bg.wasm` (about 15 MB, roughly
4 MB over the wire once the server compresses it). Loading is asynchronous, so
`await initSlint()` before the first component:

```dart
Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await initSlint();
  runApp(const MyApp());
}
```

It returns immediately on every other platform, so call it unconditionally.
`initSlint(scriptUrl: './slint_dart.js')` points elsewhere; the argument is a
module specifier, so a relative path needs the leading `./`.

Two entry points have no meaning in a browser and throw `SlintException`:
`loadFile()`, because there is no filesystem — fetch the `.slint` source and
use `loadSource()` — and `run()`/`runEventLoop()`, because the browser owns
the event loop. `SlintView` drives the frames instead.

Because the software renderer rasterizes every pixel itself, this build turns
on `i-slint-core`'s `image-decoders` and `svg` features, which a wasm build
normally leaves to the browser. Without them `std-widgets` icons panic with
"The image cannot be rendered".

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
For a packaged app, preload the `.slint` file from the Flutter asset bundle
and compile it with `loadSource`:

```dart
WidgetsFlutterBinding.ensureInitialized();
final source = await rootBundle.loadString('lib/ui/counter.slint');
runApp(MyApp(source: source));

// Inside a widget tree:
SlintView(load: () => CounterWindow.loadSource(source))
```

If the source is on the filesystem instead, wrap the factory and provide the
deployed path:

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

The Dart tests need a backend that opens no window, and they must load the
library built with that backend — `dart test` runs the build hook, which
produces a default-feature library, so pin `SLINT_DART_LIBRARY`:

```sh
cargo build -p slint-dart --features backend-testing
cd slint
SLINT_DART_LIBRARY="$PWD/../../../target/debug/libslint_dart.dylib" \
  SLINT_BACKEND=testing fvm dart test
cd ../slint_flutter
SLINT_DART_LIBRARY="$PWD/../../../target/debug/libslint_dart.dylib" \
  SLINT_BACKEND=testing fvm flutter test
```

Running the tests also needs the Rust toolchain, because the build hook
compiles the library as part of the test build.

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
- Generated wrappers load the original `.slint` source at runtime.
  Packaged Flutter apps should bundle that file as a Flutter asset and call
  `loadSource` with the preloaded text.
  `load()` and `loadFile()` still read from the filesystem, and neither exists
  on the web.
- The web carries the whole Slint compiler in the WebAssembly module, because
  the wrappers compile `.slint` at runtime. That is most of its size.
- A Rust panic on the web aborts the module instead of unwinding, so the
  `catch_unwind` guards that turn a panic into a `SlintException` elsewhere do
  not apply there. The message still reaches the browser console.
