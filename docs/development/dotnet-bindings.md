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
| gradient | `Slint.Brush` |
| `Point` | `Slint.Struct`, `Slint.LogicalPosition` in generated code |
| `keys`, `styled-text`, `data-transfer` | `Slint.Keys`, `Slint.StyledText`, `Slint.DataTransfer` |
| struct | `Slint.Struct` |
| array, model | `Slint.Model<T>`; any `IEnumerable` converts to a copy |
| enum | `Slint.EnumValue` |
| `image` | `Slint.Image` |
| anything else | `Slint.OpaqueValue`, passed back unchanged; no public property type gives one today |

Generated code narrows the numeric types and maps structs and enums to C# records and enums.
Numbers convert to integers with saturation and truncation, like Rust's `as`, which Slint uses.

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
A record initializes its fields with the struct's field defaults, which the compiler has already folded to constants.
A field that a struct from the application lacks takes the same default.
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

`<SlintProject Include="ui" />` compiles a project directory, the way `slint_build::compile(".")` does in Rust.
The generator gets the directory, and the generated code loads it at run time,
so the interpreter reads `slint-project.json` there too, with its edition, main file, and paths.
A target copies the project file and the `.slint` files under the directory, since wildcards can't expand per item outside a target.
A project gets no `-I` and `-L` from `SlintIncludePaths` and `<SlintLibraryPath>`,
because paths from the command line replace the ones of the project file.
The generation touches its outputs, since `slint-compiler` leaves an unchanged output as it is,
which would keep the target out of date.

### Test Driver

`tests/driver/dotnet` builds the generated code of all cases into one runner,
since a `dotnet build` per case takes seconds.
A case whose generated code doesn't compile is dropped from the runner and fails on its own.
The driver builds `slint-dotnet` with the `testing` feature,
which uses the test fonts and reports Windows as the operating system, like the interpreter driver.
It passes the include and library paths and the bundled translations of a case like the other drivers do.
Like them, it sets `SLINT_EDITION` to the edition of this Slint, for the compiler and for the runner,
since layout fixes such as #407 only apply from edition 1.19.

The ```` ```csharp ```` blocks of a case go into `cases/<case>.test.cs`, which the runner calls after the `test` check.
The runner is the assembly `Slint.TestDriver`, which sees the internal `Slint.Testing`.
`Testing.Init` installs the testing backend with mocked time before the first use of Slint, like `initTesting` in Node.js.
`Slint.Tests` keeps the testing backend of `SLINT_BACKEND=testing`, which runs on real time for its timer tests.

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

On Windows, winit calls `OleInitialize` for drag and drop when it creates a window,
which panics on a thread in a multithreaded apartment.
The main thread of a .NET console app is one, unless `Main` has `[STAThread]`,
so the README, the samples, and the templates use an explicit `[STAThread] Main`.
Top-level statements and `async Main` can't have it.
With the winit backend, `ComponentDefinition.Create`, `ComponentInstance.CreateLive`, and `Run` throw an `InvalidOperationException` on such a thread,
before winit can abort the process.
`slint_dotnet_backend_is_winit` reads `SLINT_BACKEND` the way the backend selector does.

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

`api/dotnet/global.json` pins the .NET SDK to 10.0.401, accepting newer patches of its feature band locally,
and the workflows install exactly that version.
The day a new SDK appears in the release list, its download can still fail, which broke every .NET job once.

## Roadmap

The numbers stay stable, so that commits and discussions can refer to them.

Done:

- **1.** Native crate and C# library: compile from a string or file, properties, callbacks, functions, globals, event loop.
- **2.** Models, timers, `async` through a `SynchronizationContext`, lifetime tests.
- **3.** `-f csharp` generator, MSBuild `<SlintFile>` items, API match check, `tests/driver/dotnet`.
- **4.** CI workflow, user documentation, an example, NuGet packages per runtime identifier.
  `.github/workflows/dotnet_test_reusable.yaml` runs the tests on macOS, Linux, and Windows,
  and `examples/memory/dotnet` is the example.
- **5.** Live reload of `.slint` files, see [Live Reload](#live-reload).
  Together with .NET Hot Reload, through `dotnet watch` or a debugger, both `.slint` and C# changes apply without a restart.
- **6.** Publishing to nuget.org with trusted publishing, see [Packages](#packages).
  Until the Slint project publishes them, a fork publishes community packages under its own IDs,
  such as `yurkinh.slint.net` and `yurkinh.slint.net.Compiler`.
  `api/dotnet/WHATS_NEW.md` lists what each preview added.
- **7.** The proposal to upstream the bindings, in [discussion #3550](https://github.com/slint-ui/slint/discussions/3550).
  The pull request itself is item 14.
- **8.** API gaps compared to Python and Node.js:
  - a `Window` API, for size, position, scale factor, and close requests
  - typed `brush`, gradients, `LogicalPosition`, and `LogicalSize`
  - typed `keys`, styled text, and data transfer
  - include and library paths in generated code
  - translations, bundled and through an `ITranslator`
  - the model adapters, in C# in `Slint/ModelAdapters.cs`
  - C# names for exported aliases of components; structs and enums keep only their own name
- **9.** `csharp` test blocks in `tests/cases`, see [Test Driver](#test-driver).
  The 18 cases with Python blocks and `callbacks/handler` have one.
  They use `Slint.Testing` for clicks, keys, and mocked time.
  Writing them found that generated records ignored struct field defaults,
  and that numbers converted to integers by rounding, and threw on overflow.
- **12.** Project directories, with `<SlintProject>`, see [MSBuild](#msbuild).
  They compile and load with the edition and the settings of their `slint-project.json`.
  A `.slint` path still compiles without a project file, and so without an edition.
- **11.** On Windows, a clear exception in `Create` and `Run` for a thread that isn't in a single-threaded apartment,
  instead of a winit panic, see [Threading and macOS](#threading-and-macos).
  Turning off drag and drop in winit would make such threads work, but lose file drops,
  and Slint doesn't expose that option.

Next:

- **10.** A C# tab in the pages of the documentation website that have a Python tab, and in `live-preview.mdx`,
  once the bindings are part of a Slint release.
  The tutorial needs its C# code next to the other languages in `docs/astro/src/content/code/`.
  Until then, a community page on the fork's `gh-pages` branch, at <https://yurkinh.github.io/slint/>,
  extends the README with deployment, troubleshooting, and samples.
  It says it isn't affiliated with SixtyFPS, links to `docs.slint.dev` for the language, and doesn't use the Slint logo.
  The branch shares no history with `dotnet_bindings`, so it stays out of the upstream pull request.
- **13.** More `csharp` blocks, for the cases that test input, animations, and models,
  ported from their `rust` or `js` blocks.
  Cases with pinch gestures or flickable physics first need those helpers in `Slint.Testing`.
- **14.** The upstream pull request, once the maintainers agree in #3550:
  squash the branch, with the official `Slint` package IDs and a `ChangeLog:` trailer.
  Until then, `master` of the fork mirrors `slint-ui/slint`, and `dotnet_bindings` merges it after each sync.
  The README links to `WHATS_NEW.md` and the community page on the fork, which the pull request points at `slint-ui/slint` instead.

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
- The `Live` setters use the `try_` setters of `LiveReloadingComponent`, which record a member only if it exists.
  Its error hook prints a warning for a recorded member that a reload removed, where the default hook panics.
- `Run` doesn't keep the instance borrowed during the event loop, since a reload replaces it.
- The reload hook calls a `LiveEvents` object, which raises `ComponentInstance.Reloaded`.
  It doesn't reference the instance, so the hook doesn't keep the instance alive.

Without the feature in the native library, generated code warns and creates components that don't reload.

`Slint.targets` writes the generator options to `obj/.../slint/options.txt`, an input of the generation,
because changing an option such as `-p:SlintLivePreview=true` changes no other input.
In live mode, the `.slint` files get `Watch="false"`, so that `dotnet watch` doesn't restart the application for them.

Renaming a member that code sets only warns at the next reload; calls to the old member throw until a rebuild.

`LiveReloadTests` changes files in the test output directory, not in the system temporary directory:
that one can be behind a symbolic link, such as `/var` on macOS, and the file watcher doesn't resolve links.
The tests wait for `Reloaded` with a timeout.
Only the check that a file with errors doesn't reload waits a fixed two seconds.

The `.NET` tab of `live-preview.mdx` waits until the bindings are part of a release.
