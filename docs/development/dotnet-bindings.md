<!-- cSpell: ignore companyname getspecific nint pthread -->
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
  The first call claims the Slint thread and creates the platform there,
  except `slint_dotnet_backend_select`, which claims it so that `BackendSelector` can create another platform.
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

Like the Python module, the generated code runs the `.slint` file with the interpreter.
With `--csharp-unit`, `slint-compiler` also writes the compiled file, with `generator::csharp::bundle`:
the unit of `llr::serialize`, the font files that it imports, and its images as `data:` URLs.
`Slint.targets` embeds it as `slint-unit:` and the runtime path, and the generated code reads it with `GeneratedApi.LoadUnit`.
That returns null for a unit of another Slint version, or with `SLINT_STYLE` set to another style,
and then the generated code compiles the `.slint` file at run time.
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
The generator passes them to `GeneratedApi.Load` as absolute paths.
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
Like them, it compiles each case with the edition of this Slint, since layout fixes such as #407 only apply from edition 1.19,
and a case with an `//edition:` line gets that edition instead.
It compiles each case to a unit too, with the same settings, and the runner embeds them, so every case runs from its unit.
The generated code would compile the case at run time without it, from a project directory that the driver doesn't create,
so a unit that doesn't load fails the case.
The driver generates the cases on every core, which takes it to about a minute on an Apple Silicon Mac.

The ```` ```csharp ```` blocks of a case go into `cases/<case>.test.cs`, which the runner calls after the `test` check.
The runner is the assembly `Slint.TestDriver`, which sees the internal `Slint.Testing`.
`Testing.Init` installs the testing backend with mocked time before the first use of Slint, like `initTesting` in Node.js.
`Slint.Tests` keeps the testing backend of `SLINT_BACKEND=testing`, which runs on real time for its timer tests.

## Packages

`Slint` packs the library, `build/Slint.targets`, the native library of each platform in `runtimes/<rid>/native/`,
and the iOS framework in `ios/slint_dotnet.xcframework`, outside `runtimes/` so that NuGet leaves it alone.
It depends on `Slint.Compiler`, which has `slint-compiler` in `tools/<rid>/`,
and a `buildTransitive` props file that tells `Slint.targets` where to look.
`Slint.targets` picks the binary for the runtime identifier of the SDK, and restores its executable bit, since NuGet doesn't keep it.
NuGet only imports `build/<id>.targets` and `buildTransitive/<id>.props`, so the packed MSBuild files take the name of the package.

```sh
dotnet pack api/dotnet/Slint.Compiler -c Release -p:Version=<version> -p:SlintCompilerBinariesDir=<dir>
dotnet pack api/dotnet/Slint -c Release -p:Version=<version> -p:SlintNativeRuntimesDir=<dir> -p:SlintIOSFramework=<xcframework>
```

Each directory has one sub-directory per runtime identifier:

| Platform | Runtime identifiers | Native library | Compiler |
|---|---|---|---|
| macOS | `osx-arm64`, `osx-x64` | `libslint_dotnet.dylib` | `slint-compiler` |
| Linux | `linux-x64`, `linux-arm64` | `libslint_dotnet.so` | `slint-compiler` |
| Windows | `win-x64`, `win-arm64` | `slint_dotnet.dll` | `slint-compiler.exe` |
| Android | `android-arm64`, `android-x64` | `libslint_dotnet.so`, from `target/<triple>/release` | none |

`-p:SlintAndroid=true` packs the Android build too, and needs the .NET Android workload.
`SlintPackageId` sets the ID, `Slint` by default, and the compiler package is `<id>.Compiler`.
This repository doesn't publish the packages.

## Threading and macOS

winit and AppKit need the event loop on the process main thread.
A console app's `Main` runs there, so call `EventLoop.Run()` from `Main`.

On Windows, winit calls `OleInitialize` for drag and drop when it creates a window,
which panics on a thread in a multithreaded apartment.
The main thread of a .NET console app is one, unless `Main` has `[STAThread]`,
so the README and the todo example use an explicit `[STAThread] Main`.
Top-level statements and `async Main` can't have it.
With the winit backend, `ComponentDefinition.Create` and `Run` throw an `InvalidOperationException` on such a thread,
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

The `testing` feature of the desktop build matters: without it, `SLINT_BACKEND=testing` falls back to winit, which aborts the test host.

The C++ build compiles `slint-compiler` without its `csharp` feature,
so C# code in `tools/compiler` needs `#[cfg(feature = "csharp")]`, which only the C++ jobs of CI catch.

`scripts/upgrade_version.sh` also bumps `api/dotnet/Directory.Build.props`.

`api/dotnet/global.json` pins the .NET SDK to 10.0.401, accepting newer patches of its feature band locally,
and the workflows install exactly that version.
The day a new SDK appears in the release list, its download can still fail, which broke every .NET job once.

## Android

Slint owns the entry point on Android, as in the C++ API:

1. The app's activity derives from `SlintActivity`, a `NativeActivity`.
   Its `OnCreate` registers the C# entry point and event callbacks with `slint_dotnet_android_set_callbacks`.
2. `NativeActivity` loads `libslint_dotnet.so`, named by the `android.app.lib_name` meta-data,
   and android-activity runs `android_main` in `api/dotnet/native/android.rs` on a thread of its own.
3. `android_main` installs Slint's Android platform, claims that thread as the Slint thread, and calls the entry point,
   which extracts the `.slint` assets, if there are any, and runs `SlintMain`.
4. The platform's event listener passes Android's activity events back to C#, while `Run` polls them:
   the saved state, and the pause, resume, and low-memory events.

Slint's state is thread-local, and `SLINT_THREAD` stays with the first thread,
so a second `android_main` in the same process can't use Slint.
`SlintActivity` handles configuration changes itself, and ends the process in `OnDestroy`.

An APK has no output directory, so `SlintEmbedFiles` is on by default on Android and iOS,
and the assembly embeds the `.slint` files with the images and fonts they use.
With `SlintEmbedFiles=false`, `Slint.targets` adds only the files of `SlintFile` and `SlintProject` items as assets under `slint/`,
at the paths that the generated code loads, and their runtime paths stay relative even with `SlintCopyToOutput=false`.
`SlintActivity` then extracts them to its files directory and sets `GeneratedApi.BaseDirectory`.

The `net10.0-android` build of `Slint.csproj` needs the .NET Android workload, so it's only built with `SlintAndroid=true`,
in `obj/android` and `bin/android`.
Build the native library with the NDK, for `aarch64-linux-android` on devices and Apple Silicon emulators,
and `x86_64-linux-android` for other emulators:

```sh
NDK=$HOME/Library/Android/sdk/ndk/<version>
BIN=$NDK/toolchains/llvm/prebuilt/darwin-x86_64/bin   # linux-x86_64 on Linux
export ANDROID_NDK_ROOT=$NDK
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$BIN/aarch64-linux-android26-clang
export CC_aarch64_linux_android=$BIN/aarch64-linux-android26-clang
export CXX_aarch64_linux_android=$BIN/aarch64-linux-android26-clang++
export AR_aarch64_linux_android=$BIN/llvm-ar
cargo build --release -p slint-dotnet --target aarch64-linux-android
dotnet build examples/todo/dotnet-android -p:SlintAndroid=true
scripts/dotnet_android_smoke_test.sh examples/todo/dotnet-android/bin/Debug/net10.0-android/dev.slint.dotnet.todo-Signed.apk dev.slint.dotnet.todo
```

Apps that reference `Slint.csproj` get the release builds from `target/<triple>/release`,
since a debug build makes every APK hundreds of megabytes.
The package has them in `runtimes/android-arm64` and `runtimes/android-x64` instead.
The ".NET Android" job of CI builds the todo example for `x86_64` and starts it in an emulator with the smoke test script.


## iOS

Slint owns the app on iOS too, as in the C++ API:
the C# `Main` creates components and calls `Run`, where the winit backend calls `UIApplicationMain`, which doesn't return.
There's no C# app delegate; the app's `Info.plist` names `SlintWindowSceneDelegate`, the scene delegate of the winit backend.

An iOS app embeds `slint-dotnet` as a dynamic framework, like SkiaSharp and the .NET runtime's own libraries:

- `scripts/build_dotnet_ios.sh` builds it with `cargo rustc --crate-type cdylib` for `aarch64-apple-ios` and `aarch64-apple-ios-sim`,
  with the winit backend and Skia only, as `slint_dotnet.framework`, and combines them into `target/ios/slint_dotnet.xcframework`.
- A static library would be 216 MB for the two, since it keeps all the code of Rust's standard library and Skia,
  and stripping it breaks the link; the frameworks, already linked and stripped, are 43 MB, 21 MB in a package.
- For a `net10.0-ios` app, `Slint.targets` adds it as a `NativeReference` of kind `Framework`,
  from `ios/` in the package, outside `runtimes/` so that NuGet doesn't treat it as native assets.
- `NativeLibraryResolver` loads `@rpath/slint_dotnet.framework/slint_dotnet` on iOS, so `Slint.dll` needs no iOS build.
- The assembly embeds the `.slint` files, as on Android.
  With `SlintEmbedFiles=false`, `Slint.targets` adds them as bundle resources instead, at the paths that the generated code loads relative to the bundle.
- `Slint.targets` keeps the desktop library that `Slint.csproj` copies out of the app.

```sh
scripts/build_dotnet_ios.sh
dotnet build examples/todo/dotnet-ios -t:Run
scripts/dotnet_ios_smoke_test.sh examples/todo/dotnet-ios/bin/Debug/net10.0-ios/iossimulator-arm64/TodoIOS.app dev.slint.dotnet.todo
```

The ".NET iOS" job of CI builds the library for both targets and the todo example, and starts it in an iPhone simulator with the smoke test script.
Each .NET for iOS requires one Xcode version, so CI installs the workload set `10.0.401`, whose .NET for iOS 26.5 takes the runner's default Xcode 26.6;
newer sets require Xcode 27, which the runner lacks.
