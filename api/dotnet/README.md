# Slint for .NET (Preview)

[Slint](https://slint.dev/) is a UI toolkit that supports different programming languages.
Slint for .NET is the integration with C# and .NET 10.

**Warning**
Slint for .NET is a preview: its APIs may still change.

## What's New

See [What's New](https://github.com/yurkinh/slint/blob/dotnet_bindings/api/dotnet/WHATS_NEW.md) for the changes in each preview.

## Documentation

The [Slint for .NET documentation](https://yurkinh.github.io/slint/) covers more than this README,
such as deployment, troubleshooting, and samples.
It's community documentation, not affiliated with SixtyFPS GmbH.

## Prerequisites

- [.NET 10 SDK](https://dotnet.microsoft.com/download)
- macOS, Linux, or Windows, on x64 or arm64, [embedded Linux](#embedded-linux) without a desktop, [Android](#android-preview) on arm64 and x86_64, or [iOS](#ios-preview)

## Quick Start

From the next preview, after 1.19.0-preview.9, a template creates the app in one step:

```sh
dotnet new install yurkinh.slint.net.Templates
dotnet new slint -o MyApp
cd MyApp
dotnet run
```

The app has the counter below, live preview in Debug builds for `dotnet watch`, and Native AOT in Release builds.
`--style fluent`, `material`, `cupertino`, or `cosmic` picks the widget style.

| Template        | Creates                                                                                    |
| --------------- | ------------------------------------------------------------------------------------------ |
| `slint`         | A desktop app for macOS, Linux, and Windows.                                               |
| `slint-android` | An [Android](#android-preview) app.                                                        |
| `slint-ios`     | An [iOS](#ios-preview) app.                                                                |
| `slint-xplat`   | A solution with a desktop, an Android, and an iOS project that share the UI and the logic. |

`--app-id com.example.myapp` sets the Android package name and the iOS bundle ID.
`dotnet new slint-xplat` puts the UI in `ui/`, the C# that the platforms share in `Shared/`,
and the settings that they share in `Directory.Build.props`.
Without the Android or iOS workload, pick the projects with `--platforms`:

```sh
dotnet new slint-xplat -o MyApp --platforms desktop --platforms android
```

Or set up the project yourself:

1. Create a project and add the `yurkinh.slint.net` package:

   ```sh
   dotnet new console -o MyApp
   cd MyApp
   dotnet add package yurkinh.slint.net --prerelease
   ```

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

3. Add it to `MyApp.csproj`:

   ```xml
   <ItemGroup>
     <SlintFile Include="ui/app-window.slint" />
   </ItemGroup>
   ```

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
| exported component | a class with a static `Create()` method, plus `Show()`, `Hide()`, and `Run()` |
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

### Charts With Microcharts

[Microcharts](https://github.com/microcharts-dotnet/Microcharts) draws bar, line, point, donut, pie, radar, and gauge charts with SkiaSharp.
Its `Microcharts.Slint`, in the `slint` branch of [the fork](https://github.com/yurkinh/Microcharts/tree/slint) until Microcharts takes it in,
shows them in a Slint `Image` with a `ChartRenderer`, on the desktop, Android, and iOS.
Its README has the code, and `Sources/Microcharts.Samples.Slint` a gallery; both need the next preview, after 1.19.0-preview.9.
`ToRgba8` returns the pixels.

The build copies the `.slint` files to the output directory, and the generated code loads them at run time.
Mark files that another `<SlintFile>` imports with `Generate="false"`:

```xml
<SlintFile Include="ui/app-window.slint" />
<SlintFile Include="ui/widgets.slint" Generate="false" />
```

Set these properties to change the generated code:

- `SlintNamespace`: the namespace, instead of the root namespace.
- `SlintStyle`: the widget style, such as `fluent` or `cupertino`.
- `SlintCopyToOutput`: `false` to load the `.slint` files from the project directory instead of copying them.
- `SlintIncludePaths`: directories that imports search, separated by semicolons.

To use a [project file](https://snapshots.slint.dev/master/docs/slint/guide/tooling/project-file/), add its directory instead of a file:

```xml
<SlintProject Include="ui" />
```

The build generates the classes of the project's main file, and copies the project file and the `.slint` files under the directory.
The application then compiles with the edition and the paths of `ui/slint-project.json`, at build and at run time.
`SlintIncludePaths` and `<SlintLibraryPath>` don't apply to a project, since its project file has its own paths.

For imports from a library, such as `import { Button } from "@mylib/button.slint"`, add a library path:

```xml
<SlintLibraryPath Include="mylib" Path="../mylib/" />
```

Paths are relative to the project.
The generated code loads imports from these directories where they are,
so an application that runs elsewhere needs the imported files copied, such as with `<SlintFile Generate="false" />`.

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
| `SlintLivePreview` | `--csharp-live-preview` |

The package `yurkinh.slint.net.Compiler` has the compiler in `tools/<rid>/`,
such as `~/.nuget/packages/yurkinh.slint.net.compiler/<version>/tools/osx-arm64/slint-compiler`.
NuGet doesn't keep its executable bit, so run `chmod +x` on it first.
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
The examples [`opengl_underlay`](https://github.com/yurkinh/slint/tree/dotnet_bindings/examples/opengl_underlay/dotnet)
and [`opengl_texture`](https://github.com/yurkinh/slint/tree/dotnet_bindings/examples/opengl_texture/dotnet) show both ways:

| A shader below the scene | A cube rendered into a texture |
| --- | --- |
| ![The OpenGL underlay example: an animated grid of diamonds below Slint's text and checkbox](https://yurkinh.github.io/slint/images/opengl-underlay.jpg) | ![The OpenGL texture example: a lit blue cube in a Slint Image, above sliders for its color](https://yurkinh.github.io/slint/images/opengl-texture.png) |

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
store the changes and notify each changed row once per frame, as `examples/lols` does.

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

## Hot Reload

Change the UI and the code of a running application without restarting it:
`.slint` files reload through Slint's live reload, and C# code through .NET Hot Reload.

### `.slint` Files

Turn on live reload for Debug builds in your project file:

```xml
<PropertyGroup Condition="'$(Configuration)' == 'Debug'">
  <SlintLivePreview>true</SlintLivePreview>
</PropertyGroup>
```

The generated code then loads the `.slint` files from the project directory,
and each component reloads in its window when you save a file it uses.
The properties and callbacks you set, models included, carry over to the new version.
A file with errors keeps the previous version until you fix it.

To reload a single file, set `LivePreview="true"` on its `<SlintFile>` instead.
`ComponentInstance.Reloaded` tells you when a component reloaded,
and `ComponentInstance.CreateLive` creates a reloading component without generated code.
Use live reload during development only.
On Android, the app loads its `.slint` files from the APK, so live reload isn't available there yet:
the build ignores `SlintLivePreview` with the warning `SLINT002`, and a changed file needs a rebuild and redeploy.

### C# Code

.NET Hot Reload applies C# changes, such as the body of a callback handler, to the running application.
Use it in one of these ways:

- Run `dotnet watch`, which applies each change when you save.
- Start the application in the debugger of Visual Studio, VS Code with C# Dev Kit, or Rider,
  and apply changes with the IDE's Hot Reload command, or on save if the IDE has that turned on.

A handler you passed to an `On…` method runs the new code from its next call,
because Slint keeps calling the same delegate.

On Android, `dotnet watch` deploys and starts the app with .NET 10, but doesn't apply C# changes,
for any .NET Android app and not only Slint ones.

### Both at Once

With `SlintLivePreview` on, run:

```sh
dotnet watch
```

Saving a `.slint` file reloads the UI, and saving a `.cs` file applies the C# change,
both without a restart and without losing the state of the application.

### What Needs a Restart

- Renaming or removing a property, callback, or function that your C# code uses.
  The reload prints a warning for each member it can't set again,
  and the generated classes still have the old members, which throw `SlintException` until you rebuild.
- A C# change that .NET Hot Reload can't apply, such as a new method signature or a new type.
  `dotnet watch` offers to restart the application.
- Changes to the project file, such as turning `SlintLivePreview` on or off.

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

## Android (Preview)

The package also runs on Android 8 (API level 26) and later, on `arm64-v8a` and `x86_64` devices and emulators.

From the next preview, after 1.19.0-preview.9, `dotnet new slint-android -o MyApp` creates the app in one step.
Then `dotnet build -t:Run` installs it on the running emulator or the connected device.
To set up the project yourself:

1. Install the .NET Android workload, then create a project and add the package:

   ```sh
   dotnet workload install android
   dotnet new android -o MyApp
   cd MyApp
   dotnet add package yurkinh.slint.net --prerelease
   ```

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

   The template already sets `SupportedOSPlatformVersion` to 24: change that line rather than adding a second one.

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
   The template's `Resources/layout/activity_main.xml` is no longer used, and you can delete it.

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

The build puts the `<SlintFile>` and `<SlintProject>` files into the APK, and `SlintActivity` extracts them at startup.
Add the images that `.slint` files use as `<SlintFile Include="ui/images/logo.png" Generate="false" />`,
under the project directory, so that they keep their paths relative to the `.slint` files.
`examples/todo/dotnet-android` is a complete example,
and `examples/lols/dotnet-android` runs the [LOLs per second](https://github.com/jonathanpeppers/lols) test on Android.

## iOS (Preview)

Slint runs on iOS 15 and later with Skia on Metal, tested in the iPhone simulator and on an iPhone 13.
The packages don't have the library for iOS yet: in the Slint repository, run `scripts/build_dotnet_ios.sh`,
and reference `api/dotnet/Slint/Slint.csproj` and its `build/Slint.targets`, as `examples/todo/dotnet-ios` does.
From the next preview, after 1.19.0-preview.9, the package has the library,
and `dotnet new slint-ios -o MyApp --app-id com.example.myapp` creates the app with the steps below already done.

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
   like the one of [the todo example](https://github.com/yurkinh/slint/blob/dotnet_bindings/examples/todo/dotnet-ios/Info.plist):

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

  Read the file back after `Create`, as [the todo example](https://github.com/yurkinh/slint/blob/dotnet_bindings/examples/todo/dotnet-ios/Program.cs) does.
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
- In 1.19.0-preview.9 and earlier, the OpenGL renderers fail with "Error adding gbm buffer as framebuffer"
  on display drivers without modifier support, such as `virtio-gpu` in virtual machines.
  Use `linuxkms-software` there, or the next preview.

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

To build the native library, the compiler, and the packages yourself, such as for a platform or a change that isn't published,
follow [Building the Packages Locally](https://github.com/yurkinh/slint/blob/dotnet_bindings/docs/development/dotnet-bindings.md#building-the-packages-locally).

## Slint Language Manual

The [Slint Language Documentation](https://slint.dev/docs/slint) covers the Slint UI description language in detail.
