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
`SlintIncludePaths` and `<SlintLibraryPath Include="name" Path="..." />` items become `-I` and `-L`.
The generator passes them to `GeneratedApi.Load` and `CreateLive` as absolute paths.
`SlintCompilerPath` defaults to `target/debug/slint-compiler` in the repository, then to `slint-compiler` on `PATH`.
A package reference imports the targets automatically; with a project reference, import `Slint/build/Slint.targets`.

### Test Driver

`tests/driver/dotnet` builds the generated code of all cases into one runner,
since a `dotnet build` per case takes seconds.
A case whose generated code doesn't compile is dropped from the runner and fails on its own.
The driver builds `slint-dotnet` with the `testing` feature,
which uses the test fonts and reports Windows as the operating system, like the interpreter driver.
It passes the include and library paths of a case like the other drivers do.
It skips cases with bundled translations, which the generated code doesn't load yet.

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

NuGet only imports `build/<id>.targets` and `buildTransitive/<id>.props`,
so the packed MSBuild files take the name of the package.
`SlintPackageId` sets the ID, `Slint` by default, and the compiler package is `<id>.Compiler`.
`SlintCommunityPackage=true` packs a build that the Slint project doesn't publish:
it takes the authors and repository from `SlintPackageAuthors` and `SlintPackageRepositoryUrl`,
leaves out the Slint logo, and says so in the description.
The workflow switches to it when the repository variable `SLINT_NUGET_PACKAGE_ID` is set,
and refuses to pack `Slint` outside `slint-ui/slint`.

Publishing uses nuget.org trusted publishing, so the repository stores no API key.
`NuGet/login` trades the GitHub OIDC token for an API key that's valid for an hour.
It needs a nuget.org policy for the repository, the workflow file `publish_nuget_package.yaml`,
and the `nuget` environment, plus the secret `NUGET_USER` with the nuget.org profile name that owns the policy.

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
5. Live reload of `.slint` files, see [Live Reload](#live-reload) (done).
   Together with .NET Hot Reload, through `dotnet watch` or a debugger, both `.slint` and C# changes apply without a restart.
6. Publishing to nuget.org with trusted publishing, see [Packages](#packages) (done).
   Until the Slint project publishes them, a fork publishes community packages under its own IDs,
   such as `yurkinh.slint.net` and `yurkinh.slint.net.Compiler`, first released as `1.19.0-preview.1`.

Next:

7. Upstreaming: agree on it with the Slint maintainers, then squash the branch into a pull request,
   with the official `Slint` package IDs and a `ChangeLog:` trailer.
8. API gaps compared to Python and Node.js:
   - a `Window` API, for size, position, scale factor, and close requests (done)
   - typed `brush`, gradients, `keys`, styled text, data transfer, `LogicalPosition`, and `LogicalSize`
   - include and library paths in generated code (done)
   - translations, the model adapters, and C# names for exported aliases
9. `csharp` test blocks in `tests/cases`, to check the generated API like the other drivers do.
10. A `.NET` tab in `live-preview.mdx` and the other pages of the documentation website, once the bindings are released.

## Live Reload

Generated code with `--csharp-live-preview`, from the MSBuild property `SlintLivePreview`,
loads the main `.slint` file from its absolute source path,
and creates components with `ComponentInstance.CreateLive`.
They reload when the file or a file it imports changes, like the live preview of Rust and C++
(`docs/astro/src/content/docs/guide/tooling/live-preview.mdx`).
Combined with `dotnet watch`, C# changes apply with .NET Hot Reload and `.slint` changes with live reload.

The native crate reuses `LiveReloadingComponent` from `internal/live-preview/live_component.rs`,
behind the `live-preview` feature, which is on by default:

- `LiveReloadingComponent` watches the files, compiles on a thread of its own,
  and on the event loop thread creates the new version in the existing window.
  It then sets every property and callback that it recorded again.
- The instance handle is `DotnetInstance` in `api/dotnet/native/instance.rs`:
  `Static` for a component from `ComponentDefinition.Create`, `Live` for a reloading one.
  Every `slint_dotnet_component_instance_*` export works on both, so the C# API is the same.
- `LiveReloadingComponent` panics when it sets a member that doesn't exist,
  so the `Live` setters first set the member on the current instance, which fails instead, and then record it.
- `Run` doesn't keep the instance borrowed during the event loop, since a reload replaces it.
- The reload hook calls a `LiveEvents` object, which raises `ComponentInstance.Reloaded`.
  It doesn't reference the instance, so the hook doesn't keep the instance alive.

Without the feature in the native library, generated code warns and creates components that don't reload.

`Slint.targets` writes the generator options to `obj/.../slint/options.txt`, an input of the generation,
because changing an option such as `-p:SlintLivePreview=true` changes no other input.
In live mode, the `.slint` files get `Watch="false"`, so that `dotnet watch` doesn't restart the application for them.

Renaming a member that code sets still ends the application at the next reload, as in Rust and C++.
A check of the generated API after each reload would come too late:
`LiveReloadingComponent` sets the recorded members before the reload hook runs.

`LiveReloadTests` changes files in the test output directory, not in the system temporary directory:
that one can be behind a symbolic link, such as `/var` on macOS, and the file watcher doesn't resolve links.
The tests wait for `Reloaded` with a timeout.
Only the check that a file with errors doesn't reload waits a fixed two seconds.

The `.NET` tab of `live-preview.mdx` waits until the bindings are part of a release.
