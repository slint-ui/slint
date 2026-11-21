<!-- cSpell: ignore myapp mylib -->
# Slint for .NET (Preview)

[Slint](https://slint.dev/) is a UI toolkit that supports different programming languages.
Slint for .NET is the integration with C# and .NET 10.

**Warning**
Slint for .NET is a preview: its APIs may still change.

## Prerequisites

- [.NET 10 SDK](https://dotnet.microsoft.com/download)
- macOS, Linux, or Windows, on x64 or arm64, [embedded Linux](#embedded-linux) without a desktop, [Android](#android-preview) on arm64 and x86_64, or [iOS](#ios-preview)
- [Rust](https://rustup.rs), to build the native library from this repository

## Quick Start

1. Build the native library and the compiler in a checkout of this repository:

   ```sh
   cargo build -p slint-dotnet -p slint-compiler
   ```

   Then create a project, and reference the bindings from the checkout in `MyApp.csproj`:

   ```sh
   dotnet new console -o MyApp
   ```

   ```xml
   <ItemGroup>
     <ProjectReference Include="<checkout>/api/dotnet/Slint/Slint.csproj" />
   </ItemGroup>
   <Import Project="<checkout>/api/dotnet/Slint/build/Slint.targets" />
   ```

   `examples/todo/dotnet` is an app set up this way.

2. Create `ui/app-window.slint`:

   ```slint
   import { Button, VerticalBox } from "std-widgets.slint";

   export component AppWindow inherits Window {
       in-out property <int> counter: 42;
       callback request-increase-value();
       VerticalBox {
           Text {
               text: "Counter: \{root.counter}";
           }
           Button {
               text: "Increase value";
               clicked => {
                   root.request-increase-value();
               }
           }
       }
   }
   ```

3. Add it to `MyApp.csproj`, and change `OutputType` to `WinExe`:

   ```xml
   <PropertyGroup>
     <OutputType>WinExe</OutputType>
   </PropertyGroup>
   <ItemGroup>
     <SlintFile Include="ui/app-window.slint" />
   </ItemGroup>
   ```

   With `Exe`, Windows opens a console window next to the app's window.
   `WinExe` has no console, like a WPF or Avalonia app, and is the same as `Exe` on macOS and Linux.
   On Windows, the app's console output, such as Slint's `debug()`, then goes nowhere.
   Keep `Exe` while you need it.

4. Replace `Program.cs`:

   ```csharp
   using MyApp;

   internal static class Program
   {
       // Windows needs this for the window, see Threading.
       [STAThread]
       private static void Main()
       {
           using var app = AppWindow.Create();
           app.OnRequestIncreaseValue(() => app.Counter++);
           app.Run();
       }
   }
   ```

5. Run it with `dotnet run`.

## Generated Code

For each `<SlintFile>`, the build generates C# classes in the project's root namespace:

| Slint | C# |
|---|---|
| exported component | a class with a static `Create()` method, plus `Show()`, `Hide()`, `Run()`, and `RunAsync()` |
| global | a class, available as a property of each component |
| `in`, `out`, and `in-out` properties | properties; `out` properties are read-only |
| function | a method |
| callback `clicked` | `OnClicked(handler)` to set the handler, and `InvokeClicked(...)` to call it |
| struct | a `record` with `init` properties |
| enum | an `enum` |

Names become PascalCase: `request-increase-value` becomes `RequestIncreaseValue`.
A component exported under another name too, such as with `export { AppWindow as MainWindow }`,
gets a class for each name.
Structs and enums keep only their own name.

To run code after a property changes, such as one that the user edits, watch it through the component's `Instance`:

```csharp
using var watch = app.Instance.WatchProperty("counter", () => Console.WriteLine(app.Counter));
```

Slint checks once per frame, so several changes between two frames call the handler once.
`WatchGlobalProperty` watches a property of a global.

The types map as follows:

| Slint | C# |
|---|---|
| `int` | `int` |
| `float`, `length`, `angle`, `percent` | `float` |
| `duration` | `TimeSpan` |
| `string` | `string` |
| `bool` | `bool` |
| `color` | `Slint.Color` |
| `brush` | `Slint.Brush`; a `Slint.Color` converts to it |
| `Point` | `Slint.LogicalPosition` |
| `image` | `Slint.Image` |
| `keys` | `Slint.Keys` |
| `styled-text` | `Slint.StyledText` |
| `data-transfer` | `Slint.DataTransfer` |
| `[T]` | `Slint.Model<T>` |

`Color` and `Brush` have `Brighter`, `Darker`, `Transparentize`, and `WithAlpha`, and `Color` has `Mix`.

Create an `Image` from a file with `Image.Load`, from a file in memory with `Image.LoadFromData`,
or from pixels that you computed with `Image.FromRgba8` and `Image.FromRgb8`.
For pixels whose colors are already multiplied by their alpha, as SkiaSharp draws them, use `Image.FromPremultipliedRgba8`.

### Generating the Code by Hand

The build runs `slint-compiler` for each `<SlintFile>`.
Run it yourself to generate the code in a script, or for a project that doesn't use `Slint.targets`:

```sh
slint-compiler -f csharp ui/app-window.slint -o AppWindow.g.cs \
    --csharp-namespace MyApp \
    --csharp-runtime-path ui/app-window.slint
```

- `--csharp-namespace` sets the namespace of the generated classes.
- `--csharp-runtime-path` is where the generated code loads the `.slint` file at run time, relative to the application directory.
  Copy the `.slint` files there, as the build does.
- For a project file, pass its directory instead of a file, and the directory as the runtime path.

The other settings map to options:

| Setting | Option |
|---|---|
| `SlintStyle` | `--style fluent` |
| `SlintIncludePaths` | `-I <dir>`, once per directory |
| `<SlintLibraryPath>` | `-L mylib=../mylib/` |
| `SlintTranslationDomain` | `--translation-domain <domain>` |
| `SlintBundledTranslations` | `--bundle-translations <dir>` |
| `SlintNoDefaultTranslationContext` | `--no-default-translation-context` |

Build the compiler with `cargo build -p slint-compiler`, which puts it in `target/debug/`.
Add the generated file to the project with `<Compile Include="AppWindow.g.cs" />`,
and generate it again when the `.slint` file changes.

To build with a `slint-compiler` of your own instead, set `SlintCompilerPath` to it.

## Window

Each component has a `Window`, for the window it shows in:

```csharp
app.Window.LogicalSize = new LogicalSize(800, 600);
app.Window.IsMaximized = true;
app.Window.OnCloseRequested(() =>
    HasUnsavedChanges() ? CloseRequestResponse.KeepWindowShown : CloseRequestResponse.HideWindow);
```

It has the position and size in logical and physical pixels, the scale factor,
`IsFullscreen`, `IsMaximized`, and `IsMinimized`, and `Show`, `Hide`, and `RequestRedraw`.
`DispatchEvent` sends input to the window as if it came from the windowing system, such as for a virtual keyboard:

```csharp
app.Window.DispatchEvent(new WindowEvent.KeyPressed("a"));
app.Window.DispatchEvent(new WindowEvent.KeyReleased("a"));
```

### Drawing With OpenGL

Draw below or on top of the scene, or into a texture that an `Image` shows, with OpenGL.
Require a renderer that uses OpenGL before any other use of Slint, then set a rendering notifier:

```csharp
new BackendSelector().RequireOpenGL().Select();
using var app = AppWindow.Create();
app.Window.SetRenderingNotifier((state, api) =>
{
    switch (state)
    {
        case RenderingState.RenderingSetup:
            // Load the OpenGL functions, such as with api.GetProcAddress("glDrawArrays"), and create resources.
            break;
        case RenderingState.BeforeRendering:
            // Draw below the scene, or into a texture: app.Texture = Image.FromBorrowedOpenGLTexture(texture, width, height);
            break;
        case RenderingState.RenderingTeardown:
            // Release the resources.
            break;
    }
});
app.Run();
```

The notifier runs with the window's OpenGL context current, so restore the state that it changes, such as bound buffers.
`GraphicsApi` is valid during the call only, while the function addresses stay valid with the context.
WGPU and Metal give C# no access to Slint's device, so drawing with them isn't available.

## Models

Use a `ListModel<T>` for the rows of a `for` repeater, or any array property.
Slint updates the UI whenever you change the list:

```csharp
var todos = new ListModel<TodoItem>();
app.Todos = todos;
todos.Add(new TodoItem { Title = "Write the docs" });
```

`AddRange`, `InsertRange`, and `RemoveRange` change several rows with one notification.

To show your own data, derive from `Model<T>`,
and call `NotifyRowChanged`, `NotifyRowAdded`, `NotifyRowRemoved`, or `NotifyReset` when it changes.
Override `InsertRow` and `RemoveRow` to let `.slint` code add and remove rows, as `ListModel` does.
Each notification is a call into Slint, so for rows that change many times per frame,
store the changes and notify each changed row once per frame.

`Filter`, `Sort`, `Map`, and `Reverse` return views of a model that follow its changes:

```csharp
var todos = new ListModel<TodoItem>();
app.OpenTodos = todos.Filter(todo => !todo.Done).Sort((a, b) => a.Title.CompareTo(b.Title));
```

`Filter` and `Sort` return a `FilterModel` and a `SortModel`.
Their `SourceRow` maps a row of the view to a row of the source,
and `Reset` applies a condition that changed outside the model, such as a search text.

## Timers and Async Code

Use `Slint.Timer` to run code after a delay, or repeatedly:

```csharp
using var timer = new Slint.Timer();
timer.Start(TimerMode.Repeated, TimeSpan.FromSeconds(1), () => app.Counter++);
```

While the event loop runs, `await` continues on the Slint thread.
So async callbacks can update properties after awaiting:

```csharp
app.OnRefresh(async () =>
{
    var data = await httpClient.GetStringAsync(url);
    app.Status = data;
});
```

From another thread, call `EventLoop.InvokeFromEventLoop` to run code on the Slint thread.

## Translations

Mark strings for translation with `@tr()` in `.slint` files.
To bundle gettext `.po` files into the application, set in your project file:

```xml
<PropertyGroup>
  <SlintTranslationDomain>myapp</SlintTranslationDomain>
  <SlintBundledTranslations>translations</SlintBundledTranslations>
</PropertyGroup>
```

The build bundles `translations/<language>/LC_MESSAGES/myapp.po`.
After creating a component, switch the language at run time:

```csharp
Translations.SelectBundledTranslation("de");
```

An empty string selects the strings as written in the `.slint` file.
To translate with your own code instead, such as a .NET gettext library,
implement `ITranslator` and pass it to `Translations.SetTranslator`.
It translates the components that you compiled without bundled translations.
Set `SlintNoDefaultTranslationContext` to `true` if your `.po` files have no context for strings without one.

## Threading

Slint objects belong to the thread that first uses Slint.
Use them from that thread only, and make it the main thread: the one that runs `Main`.

On Windows, mark `Main` with `[STAThread]`.
The windowing backend sets up OLE for drag and drop, which needs a single-threaded apartment,
and without one the application crashes when it creates its first window.
Top-level statements can't take the attribute, so write `Main` out, as in the [Quick Start](#quick-start).
The attribute does nothing on macOS and Linux, so keep it for every platform.

Don't make `Main` `async`, since an `async Main` runs in a multithreaded apartment.
For asynchronous code, call `EventLoop.Run(async () => { ... })` from a synchronous `Main`.

## Publishing

`dotnet publish -c Release -r <runtime identifier>` writes the app, the `slint_dotnet` library,
and the `.slint` files, which the app loads at run time.
With `SlintEmbedFiles` set to `true`, the assembly embeds the `.slint` files instead,
with the images and fonts they use and the bundled translations, and the compiler reads them from memory.
It's on by default for a single-file publish, which then writes one executable:

```sh
dotnet publish -c Release -r win-x64 -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true
```

With Native AOT, the output is the executable and the `slint_dotnet` library.
Bundled translations are the exception to memory: the app writes their `.po` files to a directory of its temporary files at startup.
A path with a space in it doesn't work for the embedded files.

## Android (Preview)

The bindings also run on Android 8 (API level 26) and later, on `arm64-v8a` and `x86_64` devices and emulators.
`examples/todo/dotnet-android` is an app set up with the steps below,
and `dotnet build -t:Run -p:SlintAndroid=true` installs it on the running emulator or the connected device.

1. Build the native library for Android, as [the developer guide](../../docs/development/dotnet-bindings.md#android) describes.
   Install the .NET Android workload, create a project,
   and reference the bindings from the checkout as in the [Quick Start](#quick-start):

   ```sh
   dotnet workload install android
   dotnet new android -o MyApp
   ```

   Build it with `-p:SlintAndroid=true`, which builds the bindings for Android too.

2. In `MyApp.csproj`, change `SupportedOSPlatformVersion` to 26, add the runtime identifiers that Slint has libraries for,
   and add your `.slint` file:

   ```xml
   <PropertyGroup>
     <SupportedOSPlatformVersion>26</SupportedOSPlatformVersion>
     <RuntimeIdentifiers>android-arm64;android-x64</RuntimeIdentifiers>
   </PropertyGroup>
   <ItemGroup>
     <SlintFile Include="ui/app-window.slint" />
   </ItemGroup>
   ```

   `dotnet new android` sets `SupportedOSPlatformVersion` to 24: change that line rather than adding a second one.

3. Create `ui/app-window.slint`, the same as in the [Quick Start](#quick-start):

   ```slint
   import { Button, VerticalBox } from "std-widgets.slint";

   export component AppWindow inherits Window {
       in-out property <int> counter: 42;
       callback request-increase-value();
       VerticalBox {
           Text {
               text: "Counter: \{root.counter}";
           }
           Button {
               text: "Increase value";
               clicked => {
                   root.request-increase-value();
               }
           }
       }
   }
   ```

   Without a `<SlintFile>` item for it, the build generates no `AppWindow` class,
   and fails with "The name 'AppWindow' does not exist".

4. Replace `MainActivity.cs`: derive the main activity from `SlintActivity`, and create the components in `SlintMain`:

   ```csharp
   using Slint.Android;

   namespace MyApp;

   [Activity(MainLauncher = true, ConfigurationChanges = SlintActivity.ConfigurationChanges)]
   [MetaData(SlintActivity.LibraryNameKey, Value = SlintActivity.LibraryName)]
   public class MainActivity : SlintActivity
   {
       protected override void SlintMain()
       {
           using var app = AppWindow.Create();
           app.OnRequestIncreaseValue(() => app.Counter++);
           app.Run();
       }
   }
   ```

   Both attributes are needed: .NET Android only reads them from your own activity.
   The `Resources/layout/activity_main.xml` of `dotnet new android` is no longer used, and you can delete it.

5. Connect a device with USB debugging, or start an emulator, and run the app:

   ```sh
   dotnet build -t:Run
   ```

   To install the APK of a Debug build with `adb install` instead,
   set `<EmbedAssembliesIntoApk>true</EmbedAssembliesIntoApk>`, since a Debug build leaves the assemblies out of the APK otherwise.

`SlintMain` and every callback run on Slint's thread, not on the Android UI thread.
Slint's objects belong to that thread, so an app has one `SlintActivity`, and it ends the process when Android destroys it,
such as after the Back button.

Android also ends the process of an app in the background when it needs memory,
and starts a new one when the user returns.
To keep what the user was doing, return it from `SlintSaveState`, and take it back in `SlintRestoreState`:

```csharp
protected override byte[]? SlintSaveState() => JsonSerializer.SerializeToUtf8Bytes(settings);

protected override void SlintRestoreState(byte[] state) => Apply(JsonSerializer.Deserialize<Settings>(state));
```

`SlintRestoreState` runs once, in the event loop, after `SlintMain` created the components.
`SlintPause`, `SlintResume`, and `SlintLowMemory` report the other activity events.
All of them run on Slint's thread, unlike `OnPause` and the other methods of `Activity`.
Android drops the saved state when the user closes the app, so keep what must outlast that in files or preferences.

The assembly embeds the `<SlintFile>` and `<SlintProject>` files, with the images and fonts they use, as on iOS.
With `SlintEmbedFiles` set to `false`, the build puts only the `.slint` files into the APK, and `SlintActivity` extracts them at startup.
`examples/todo/dotnet-android` is a complete example.

## iOS (Preview)

Slint runs on iOS 15 and later with Skia on Metal, tested in the iPhone simulator and on an iPhone 13.
Build the native library for iOS with `scripts/build_dotnet_ios.sh`, and reference the bindings as in the [Quick Start](#quick-start).
`examples/todo/dotnet-ios` is an app set up with the steps below.

1. Install the .NET iOS workload, and create a project for `net10.0-ios` with your `.slint` file:

   ```xml
   <Project Sdk="Microsoft.NET.Sdk">
     <PropertyGroup>
       <TargetFramework>net10.0-ios</TargetFramework>
       <SupportedOSPlatformVersion>15.0</SupportedOSPlatformVersion>
       <OutputType>Exe</OutputType>
       <ApplicationId>com.example.myapp</ApplicationId>
     </PropertyGroup>
     <ItemGroup>
       <SlintFile Include="ui/app-window.slint" />
     </ItemGroup>
   </Project>
   ```

2. Add an `Info.plist` that names Slint's scene delegate, which iOS 27 requires,
   like the one of [the todo example](../../examples/todo/dotnet-ios/Info.plist):

   ```xml
   <key>UIApplicationSceneManifest</key>
   <dict>
     <key>UIApplicationSupportsMultipleScenes</key>
     <false/>
     <key>UISceneConfigurations</key>
     <dict>
       <key>UIWindowSceneSessionRoleApplication</key>
       <array>
         <dict>
           <key>UISceneConfigurationName</key>
           <string>Slint</string>
           <key>UISceneDelegateClassName</key>
           <string>SlintWindowSceneDelegate</string>
         </dict>
       </array>
     </dict>
   </dict>
   ```

3. Create the components in `Main`, and call `Run`, where Slint starts the app and doesn't return:

   ```csharp
   internal static class Program
   {
       private static void Main()
       {
           using var app = AppWindow.Create();
           app.OnRequestIncreaseValue(() => app.Counter++);
           app.Run();
       }
   }
   ```

   There's no `AppDelegate` and no `UIApplication.Main`: Slint takes their place.

4. Build and start it in the simulator, or with `-r ios-arm64` and a signing identity on a device:

   ```sh
   dotnet build -t:Run
   ```

- The build embeds Slint's framework into the app, and puts the `.slint` files into the app bundle.
- `SlintLivePreview` isn't available on iOS yet; the build ignores it, with the warning `SLINT002`.
- iOS ends apps in the background without notice. Keep what the user was doing when the app leaves the screen,
  with the UIKit notifications of .NET for iOS, which arrive on the Slint thread:

  ```csharp
  var file = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments), "state.json");
  using var observer = UIKit.UIApplication.Notifications.ObserveDidEnterBackground((_, _) => File.WriteAllBytes(file, SaveState(app)));
  ```

  Read the file back after `Create`, as [the todo example](../../examples/todo/dotnet-ios/Program.cs) does.
- Native AOT works on devices: set `<PublishAot>true</PublishAot>` in the project file, not with `-p:`,
  which would also apply it to the Slint libraries, and run `dotnet publish -c Release -r ios-arm64`.
  Builds for the simulator keep using Mono.

## Embedded Linux

On a device without a desktop, such as a board with a touchscreen,
Slint draws straight to the display with its `linuxkms` backend, and reads input with libinput.
Publish for the device's runtime identifier, and start the app on its console:

```sh
dotnet publish -c Release -r linux-arm64 --self-contained
SLINT_BACKEND=linuxkms ./MyApp
```

- Give the app access to `/dev/dri` and `/dev/input`: run it as root, or add its user to the `video` and `input` groups.
- Stop the desktop first, if the device has one, since it holds the display.
- `linuxkms` picks a renderer; `linuxkms-software`, `linuxkms-femtovg`, and `linuxkms-skia-opengl` choose one.
- The device needs libinput and libudev, and for OpenGL, GBM and EGL, which the app loads at run time.

See [Slint's LinuxKMS guide](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_linuxkms/) for the renderers and the display settings.

## Without Generated Code

`Compiler` compiles `.slint` files at run time,
and `ComponentInstance` accesses properties, callbacks, and functions by name:

```csharp
using var compiler = new Compiler();
using var result = compiler.BuildFromPath("app-window.slint");
using var definition = result.Component("AppWindow")!;
using var app = definition.Create();
app.SetCallback("request-increase-value", _ => app.SetProperty("counter", (double)app.GetProperty("counter")! + 1));
app.Run();
```

## Building From Source

[The developer guide](../../docs/development/dotnet-bindings.md) explains how the bindings are built and tested.

## Slint Language Manual

The [Slint Language Documentation](https://slint.dev/docs/slint) covers the Slint UI description language in detail.
