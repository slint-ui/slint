# AGENTS.md — api/flutter

This file provides guidance to AI coding assistants working in the Dart and
Flutter bindings for Slint. The UI is written in `.slint`, the logic in Dart.

Four pieces live here, plus the Rust side they call into:

| Piece | What it is |
| --- | --- |
| [`rust/`](./rust) | `slint-dart`, a Rust `cdylib` exposing a plain C ABI over `slint-interpreter`. |
| [`slint/`](./slint) | `package:slint`, the binding itself. Pure Dart over `dart:ffi`, no Flutter dependency. |
| [`slint_generator/`](./slint_generator) | The `build_runner` builder that turns a `.slint` file into a typed Dart API. Dev dependency only. |
| [`slint_flutter/`](./slint_flutter) | A `SlintView` widget that renders a Slint UI inside a Flutter app. |

## Prerequisites

The Dart and Flutter SDK is pinned with [FVM](https://fvm.app) in `.fvmrc`
(`"flutter": "stable"`), so every command below is available as `fvm dart …` and
`fvm flutter …`. Run `fvm install` once to fetch the pinned version.

The Dart tests need a native library that `cargo` builds, so the Rust toolchain
(`cargo`, `rustc`, and `cbindgen` for regenerating bindings) is also required.

## Build Commands

The `slint-dart` crate lives in its own sub-workspace but shares the root
`target/` directory (configured in the repository's `.cargo/config.toml`).

```sh
cargo build --release -p slint-dart      # the native library the bindings load
cd api/flutter/slint && fvm dart pub get
```

The `cdylib` is named `libslint_dart` (`libslint_dart.dylib` on macOS,
`libslint_dart.so` on Linux, `slint_dart.dll` on Windows) and lands in the
shared `target/release/`.

`package:slint` finds the library by reading `SLINT_DART_LIBRARY` first, then
walking up from the working directory, the running executable, the running
script, and the linked package root for a `target/release` or `target/debug`
copy, and finally asking the platform loader (see
`slint/lib/src/ffi.dart`). Point `SLINT_DART_LIBRARY` at a built library to
override discovery.

## Testing

The Dart tests must open no window, so they run against the `backend-testing`
feature and pin the library via `SLINT_DART_LIBRARY`:

```sh
cargo build -p slint-dart --features backend-testing
cd api/flutter/slint
SLINT_DART_LIBRARY="$PWD/../../../target/debug/libslint_dart.dylib" \
  SLINT_BACKEND=testing fvm dart test
cd ../slint_flutter
SLINT_DART_LIBRARY="$PWD/../../../target/debug/libslint_dart.dylib" \
  SLINT_BACKEND=testing fvm flutter test
```

The `slint_generator` builder tests use a fake generator and never load the
native library, so they need no `SLINT_DART_LIBRARY`:

```sh
cd api/flutter/slint_generator && fvm dart test && fvm dart analyze
```

`dart test` runs the build hook, which produces a default-feature library — that
is why the test commands pin `SLINT_DART_LIBRARY` to the `backend-testing`
build.

## The FFI bindings are generated

The C entry points are not declared by hand on both sides. cbindgen writes a C
header from `rust/`, and ffigen turns that into `slint/lib/src/ffi.g.dart`:

```sh
cargo install cbindgen        # once
./scripts/generate_slint_dart_bindings.bash
```

`ffi.g.dart` is committed, so building the package needs neither tool. Run the
script with `--check` in CI: it regenerates into a temporary copy and fails if
the result differs, which stops a changed Rust signature from silently
disagreeing with Dart.

If you change a signature in `rust/`, or add or remove an entry point, you must
regenerate `ffi.g.dart` (and `target/slint_dart.h`) with
`./scripts/generate_slint_dart_bindings.bash` and commit the result. The
`ffigen.yaml` rename map and the hand-written conversions in `ffi.dart` are
documented in the README.

## Architecture

### The Rust ABI (`rust/`)

- `rust/lib.rs` — the C ABI over `slint-interpreter`: compiler, instance,
  callbacks, timers, and the JSON envelope. `#[unsafe(no_mangle)]` exports are
  FFI; the handle types (`SlintCompiler`, `SlintComponentDefinition`, …) cross
  the ABI only behind opaque pointers declared in `cbindgen.toml`.
- `rust/embedded.rs` — embedded mode: Slint renders into a caller-owned buffer
  instead of opening a native window. This is what Flutter uses, because the
  Dart VM does not run `main()` on the process main thread and a second native
  window would not compose with the widget tree.

### The binding (`slint/`)

- `slint/lib/slint.dart` — the public API users see: `loadFile`/`loadSource`,
  `ComponentInstance`, `SlintGlobal`, `runEventLoop`, `SlintTimer`, and the
  callback dispatch over `NativeCallable.isolateLocal`.
- `slint/lib/src/ffi.dart` — `SlintFfi` adds only how the library is found and
  the JSON-envelope helpers (`takeEnvelope`, `takeString`, `withNativeString`);
  everything else lives in the generated `ffi.g.dart`. These three are the only
  place that casts between `Pointer<Char>` and `package:ffi`'s `Pointer<Utf8>`.
- `slint/lib/src/diagnostics.dart` — `Diagnostic` and `SlintException`.
- `slint/lib/src/embedded.dart` — `SlintSurface` and the input enums, mirroring
  `rust/embedded.rs`.
- `slint/hook/build.dart` — the Dart build hook: every `flutter build`/`run`
  that depends on `slint` runs it, invokes `cargo build -p slint-dart`, and
  bundles the `cdylib` into the application (as a framework on macOS). iOS
  builds nothing here; it uses the xcframework instead. Android cross-compiles
  each ABI with `cargo-ndk` against the Android NDK.

### The generator (`slint_generator/`)

- `slint_generator/lib/builder.dart` — the `slintBuilder` factory used by
  `build.yaml`.
- `slint_generator/lib/src/builder.dart` — `SlintBuilder`, the `build_runner`
  builder. Reads each `.slint` file, calls the native compiler's `generate`,
  writes the `.slint.dart` wrapper, and registers compiler dependencies.
- The builder's `buildExtensions` getter is dynamic: the default emits
  `.slint.dart` next to the source, while an `output_dir` option relocates
  outputs into a custom folder via a capture group. `build_to: source` only
  allows writing to `allowedOutputs`, which derives from the instance's
  `buildExtensions` (authoritative over `build.yaml`'s static value).
- `options` split: `include_paths` and `style` are passed to the native
  compiler; `output_dir` is a build_runner concern and is not.

### The widget (`slint_flutter/`)

- `slint_flutter/lib/slint_flutter.dart` — `SlintView`, a widget that drives a
  `SlintSurface` each frame and dispatches pointer/key input to it.

## Key Patterns

- Everything must be used from the main isolate, where the Slint event loop
  lives. This matches the Python and Node.js bindings.
- `package:slint` is pure Dart; it depends only on `dart:ffi`, `ffi`, `path`,
  and the build-hook packages. It must not depend on Flutter. Flutter-only code
  goes in `slint_flutter`.
- Values cross the boundary as JSON: `num`, `String`, `bool`, `List`, and
  `Map<String, Object?>` for structs. Colors and brushes are CSS-style strings.
- FFI modules and generated files follow the existing conventions — match the
  surrounding code, and keep `ffi.g.dart` in sync with `rust/`.
- Generated code is excluded from the analyzer (via each package's
  `analysis_options.yaml` `analyzer.exclude`) and is not reformatted:
  `ffi.g.dart` comes from ffigen and the `.slint.dart` wrappers come from the
  `build_runner` generator. The `.slint.dart` files are gitignored, so they
  must never be edited by hand. Don't run `dart format` on generated files.
- Code style is enforced in CI: `dart format`/analyzer for Dart, `rustfmt` for
  Rust.

## Version Control (Git)

- Default branch is `master`; prefer linear history (rebase or squash).
- Follow the repository's [Writing Style Guide](../../docs/internal/writing-style-guide.md)
  for all comments, doc comments, and commit messages.

## Deep Dive Documentation

- `api/flutter/README.md` — the authoritative user-facing guide for this
  directory: building, testing, packaging (build hook, iOS xcframework), and
  the two ways to show a UI (native window vs. `SlintSurface`).
- Root `AGENTS.md` — repository-wide build, test, and architecture context.
