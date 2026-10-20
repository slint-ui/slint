# .NET Bindings

> Note for AI coding assistants (agents):
> **When to load this document:** Working on `api/dotnet/`, the C# code generator,
> or `tests/driver/dotnet`.
> For the other language bindings, see `ffi-language-bindings.md`.

## Overview

The .NET bindings follow the same shape as the Python and Node.js bindings:

| Part | Python | Node.js | .NET |
|---|---|---|---|
| Native crate wrapping `slint-interpreter` | `api/python/slint` | `api/node` | `api/dotnet/native` |
| Idiomatic host-language layer | `api/python/slint/slint/` | `api/node/typescript/` | `api/dotnet/Slint/` |
| Typed code generator | `generator/python.rs` | `generator/typescript.rs` | `generator/csharp.rs` |
| `tests/cases` driver | `tests/driver/python` | `tests/driver/nodejs` | `tests/driver/dotnet` |

The managed library targets `net10.0` and is AOT-compatible.

The bindings don't reuse `internal/interpreter/ffi.rs`.
That module exposes C++ layouts such as `SharedString` and `StructOpaque`,
which C# would have to mirror byte for byte.
`api/dotnet/native` instead exports a handle-based C ABI designed for P/Invoke.

## Layout

```
api/dotnet/
├── native/          # Rust crate `slint-dotnet`, builds libslint_dotnet.{dylib,so,dll}
├── Slint/           # C# library `Slint.csproj`, and build/Slint.targets
├── Slint.Compiler/  # packs slint-compiler for the Slint.Compiler package
├── Slint.Tests/     # MSTest tests on Microsoft.Testing.Platform
├── README.md        # the user documentation, also the README of the packages
└── Slint.slnx
```

## Native ABI

Every export in `api/dotnet/native` follows these rules:

- An object crosses the boundary as an opaque pointer from `Box::into_raw`,
  and each type has a matching `slint_dotnet_<type>_free`.
  C# wraps each one in a `SafeHandle`.
- Strings passed in are UTF-8 pointer and length pairs.
- Strings passed out are `OwnedString` structs, returned by value and released with `slint_dotnet_string_free`.
  Neither side frees memory the other allocated.
- A fallible function returns `bool`, or null for a pointer.
  `slint_dotnet_last_error` then returns the message for the current thread,
  and C# throws `SlintException`.
- Each export runs inside `catch_unwind`, since unwinding into managed code aborts the process.
  Release builds use `panic = "abort"`, so there a panic still aborts.
- A managed callback is a function pointer plus a `GCHandle` as user data.
  Rust calls the drop function pointer when it drops the closure, which frees the `GCHandle`.
- Slint objects are `!Send`.
  The first call claims the Slint thread and creates the platform there.
  Calls from any other thread fail with an error.
- Objects released on another thread, such as by the .NET finalizer, are dropped on the Slint thread through `invoke_from_event_loop`.
- An exception thrown by a .NET callback is kept until the native call returns, then rethrown.
  Inside the event loop, it also stops the loop, and `EventLoop.Run` rethrows it.

C# declares the exports with `[LibraryImport]` in `Slint/Interop/NativeMethods.cs`.

## Value Mapping

| Slint | C# |
|---|---|
| `string` | `string` |
| `int`, `float`, `length`, `duration`, … | `double` |
| `bool` | `bool` |
| `color`, solid `brush` | `Slint.Color` |
| struct | `Slint.Struct` |
| array, model | `Slint.Model<T>`; any `IEnumerable` converts to a copy |
| enum | `Slint.EnumValue` |
| `image` | `Slint.Image` |
| anything else, such as a gradient | `Slint.OpaqueValue`, passed back unchanged |

Generated code narrows the numeric types and maps structs and enums to C# records and enums.

## Models

A `Slint.Model<T>` that Slint uses is wrapped in a `ManagedModel` in `api/dotnet/native/models.rs`.
The `ManagedModel` holds a strong `GCHandle`, so Slint keeps the .NET model alive.
The .NET model holds only a `Weak` to the `ManagedModel`, to notify Slint of changes.
There's no strong reference back, so nothing forms a cycle across the two heaps.
Once Slint drops the model, a later conversion creates a new `ManagedModel`.

Reading a model property returns the original .NET object.
A model that Slint created, such as an array literal, comes back as a `Model<object?>` over the native value.

## Generated Code

`slint-compiler -f csharp` generates a class per exported component and global,
a `record` per struct, and an `enum` per enum.
Properties become C# properties, functions become methods,
and each callback becomes an `On…` method that sets the handler and an `Invoke…` method.
Names are PascalCase; one that collides with another member gets a `_` suffix.
`--csharp-namespace` sets the namespace,
and `--csharp-runtime-path` sets where the code loads the `.slint` file from, relative to the application directory.

Like the Python module, the generated code loads the `.slint` file with the interpreter at run time.
Before creating a component, it checks that the file still has every member with the type the code was generated for,
and throws `SlintException` listing the differences otherwise.
The check is in C#, in `GeneratedApi`, because the generated code already lists each member.
Python instead embeds the API as JSON and compares it in Rust.

Struct and enum conversions are registered with `Slint.Generated.Conversion` when the first component is created.

### MSBuild

`Slint/build/Slint.targets` turns `<SlintFile>` items into generated code under `obj/`,
and copies them to the output directory, where the generated code loads them from:

```xml
<SlintFile Include="ui/app.slint" />
<SlintFile Include="ui/widgets.slint" Generate="false" />
```

Mark the files that `app.slint` imports with `Generate="false"`, so that they're copied but generate nothing.
`SlintNamespace` defaults to `RootNamespace`, and `SlintStyle` selects the widget style.
`SlintCompilerPath` defaults to `target/debug/slint-compiler` in the repository, then to `slint-compiler` on `PATH`.
A package reference imports the targets automatically; with a project reference, import `Slint/build/Slint.targets`.

### Test Driver

`tests/driver/dotnet` builds the generated code of all cases into one runner,
since a `dotnet build` per case takes seconds.
A case whose generated code doesn't compile is dropped from the runner and fails on its own.
The driver builds `slint-dotnet` with the `testing` feature,
which uses the test fonts and reports Windows as the operating system, like the interpreter driver.
It skips cases that need include paths, library paths, or bundled translations,
because the generated code doesn't pass those to the interpreter yet.

## Packages

There are two packages.
`Slint` has the library, `build/Slint.targets`, and the native library of each platform in `runtimes/<rid>/native/`.
It depends on `Slint.Compiler`, which has `slint-compiler` in `tools/<rid>/`,
and a `buildTransitive` props file that tells `Slint.targets` where to look.
`Slint.targets` picks the binary for the runtime identifier of the SDK, and restores its executable bit,
since NuGet doesn't keep it.

Pack them with directories that have one sub-directory per runtime identifier:

```sh
dotnet pack api/dotnet/Slint.Compiler -c Release -p:Version=<version> -p:SlintCompilerBinariesDir=<dir>
dotnet pack api/dotnet/Slint -c Release -p:Version=<version> -p:SlintNativeRuntimesDir=<dir>
```

`.github/workflows/publish_nuget_package.yaml` builds the binaries on each platform,
packs them, builds and runs a project with the packages on macOS, Linux, and Windows,
and publishes them to nuget.org when asked to.
NuGet license expressions only know SPDX licenses, so the packages carry `LICENSE.md` instead.

## Threading and macOS

winit and AppKit need the event loop on the process main thread.
A console app's `Main` runs there, so call `EventLoop.Run()` from `Main`.

While the loop runs, `SynchronizationContext.Current` is a `SlintSynchronizationContext`,
which posts through `invoke_from_event_loop`.
So `await` in a callback continues on the Slint thread,
and `EventLoop.Run(Func<Task>)` runs an `async` main function until it completes.

Hiding the last window queues a quit, even while the loop isn't running.
The next `EventLoop.Run` then returns right away.

Test runners call tests from thread pool threads.
`Slint.Tests` runs every test on one dedicated thread and uses the testing backend
(`SLINT_BACKEND=testing`, crate feature `backend-testing`).

## Lifetime

A callback that captures its own `ComponentInstance` forms a cycle:
Rust holds a `GCHandle` to the delegate, and the delegate holds the instance.
Dispose the instance explicitly, or capture a `WeakReference` in the callback.

## Building and Testing

```sh
cargo build -p slint-dotnet --features testing
cargo build -p slint-compiler
dotnet test --project api/dotnet/Slint.Tests
cargo test --manifest-path tests/Cargo.toml -p test-driver-dotnet
```

`Slint.csproj` copies `target/debug/libslint_dotnet.*` to the output of every project that references it.
Build with `-p:SlintCargoProfile=release` to use the release build instead,
and rebuild the C# project after building the native library for the first time.
At run time, `SLINT_DOTNET_NATIVE_LIBRARY` overrides the library path.

## Roadmap

1. Native crate and C# library: compile from a string or file, properties, callbacks, functions, globals, event loop (done).
2. Models, timers, `async` through a `SynchronizationContext`, lifetime tests (done).
3. `-f csharp` generator, MSBuild `<SlintFile>` items, API match check, `tests/driver/dotnet` (done).
4. CI workflow, user documentation, an example, NuGet packages per runtime identifier (done).
   `.github/workflows/dotnet_test_reusable.yaml` runs the tests, and `examples/memory/dotnet` is the example.
5. Live reload of `.slint` files, see [Planned: Live Reload](#planned-live-reload).

## Planned: Live Reload

Reload `.slint` files in a running application when they change on disk,
like the live preview of Rust and C++ (`docs/astro/src/content/docs/guide/tooling/live-preview.mdx`).
Combined with `dotnet watch`, this gives C# hot reload and `.slint` reload in one session.

### How Rust and C++ Do It

Both use `LiveReloadingComponent` in `internal/live-preview/live_component.rs`:

- With live preview enabled, the generator emits a wrapper over the interpreter instead of compiled code
  (`generator/rust_live_preview.rs`, `generator/cpp_live_preview.rs`).
- A `notify` watcher follows the main file and its imports, from the `watch_paths` of each compilation result.
- A background thread compiles, debounced by `REBUILD_DEBOUNCE`, and drops results that a newer change made stale.
- On the event loop thread, the new instance is created in the existing window with `create_with_existing_window`.
  Every property and callback the application set is then set again, so models and handlers survive.
- A file with errors keeps the previous UI.
  Renaming a member that code uses can terminate the application.
- It's enabled at build time: `SLINT_LIVE_PREVIEW=1` with the `live-preview` feature in Rust,
  `SLINT_FEATURE_LIVE_PREVIEW` in CMake for C++.
  C++ reaches it through the `slint_live_preview_*` C ABI.

### Design for .NET

The generated C# code already runs on the interpreter, so .NET needs no second generator mode.
Reuse `LiveReloadingComponent` instead of re-implementing it with `FileSystemWatcher`,
which would duplicate it and still need new exports such as creating an instance in an existing window.

- Native: a `live-preview` feature that enables `i-slint-live-preview/live-component`.
  The instance handle becomes `enum DotnetInstance { Static(ComponentInstance), Live(Rc<RefCell<LiveReloadingComponent>>) }`,
  and every `slint_dotnet_component_instance_*` export dispatches on it, so the C# API doesn't change.
  Add `slint_dotnet_live_component_new` with the compiler options, built like `slint_live_preview_new`,
  and an export that sets `set_post_reload_hook`.
- C#: `GeneratedApi.CreateLive`, which returns a `ComponentInstance`,
  and a `ComponentInstance.Reloaded` event.
  After each reload, run the `GeneratedApi` check again and warn about mismatches,
  instead of failing later with a panic.
- Generator: `--csharp-live-preview`, with which `Create()` loads the absolute source path with live reload,
  instead of the copy in the output directory.
- MSBuild: `<SlintLivePreview>true</SlintLivePreview>` passes that option.
  `.slint` files must stay out of the files that `dotnet watch` restarts the application for.

Open decisions:

- Whether the NuGet native library includes the feature, or a separate development package does,
  like `slint-dev` for Python and the dev binary for Node.js.
  Including it is simpler, since it stays inactive until generated code opts in.
- Whether Debug builds enable it by default.
  Rust and C++ make it opt-in, and so should .NET.

### Steps

| Step | Estimate |
|---|---|
| Native: `DotnetInstance`, the `live-preview` feature, the constructor and reload hook exports | 1–1.5 days |
| C#: `CreateLive`, the `Reloaded` event, the API check after reloads | 0.5 day |
| Generator option and `SlintLivePreview` MSBuild property | 0.5 day |
| Tests: rewrite a file, wait for `Reloaded`, check that properties, callbacks, and models survive, and that errors keep the old UI | 1 day |
| Docs: the package README, this document, a .NET tab in `live-preview.mdx` | 0.5 day |
| CI and packages on macOS, Linux, and Windows | 0.5 day |

The estimate is four to five days in total.

### Risks

- File watching timing differs per platform, such as macOS FSEvents latency and editors that save through a temporary file.
  Tests must wait for `Reloaded` with a generous timeout, not sleep.
- `LiveReloadingComponent` panics when it sets a property that no longer exists.
  Debug builds turn that into an exception from `EventLoop.Run`, release builds abort.
- A change to the API that code uses still needs a rebuild and restart, like in Rust and C++.
