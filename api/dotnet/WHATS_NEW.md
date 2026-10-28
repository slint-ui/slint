# What's New in Slint for .NET

## Next Preview

- On embedded Linux, the OpenGL renderers of the `linuxkms` backend work with display drivers without modifier support,
  such as `virtio-gpu` in virtual machines, instead of failing with "Error adding gbm buffer as framebuffer".
- The native library builds without its `live-preview` feature.
- iOS: the package embeds Slint as a framework into `net10.0-ios` apps, which run on devices and in the simulator.
  The C# `Main` creates the components and calls `Run`, and the app's `Info.plist` names Slint's `SlintWindowSceneDelegate`.
  Native AOT works with `PublishAot` in the project; see the README for keeping state when iOS ends the app.
- Templates: `dotnet new install yurkinh.slint.net.Templates`, then `dotnet new slint -o MyApp` creates a desktop app
  with live preview in Debug builds and Native AOT in Release builds.

## 1.19.0-preview.9

- `ListModel.InsertRange` and `ListModel.RemoveRange` change several rows with one notification.
- Structs, and so records, cross into Slint in one call per struct instead of three per field,
  with numbers passed inline and field names encoded once.
  This takes about 6% off the Slint thread's work in the lols example on an Android emulator.
- Draw with OpenGL below or on top of the scene, or into a texture that an `Image` shows:
  `BackendSelector` requires a renderer that uses OpenGL, `Window.SetRenderingNotifier` runs code in each phase of rendering,
  and `Image.FromBorrowedOpenGLTexture` shows a texture.
  The examples `opengl_underlay` and `opengl_texture` have C# versions that use them.

## 1.19.0-preview.8

- On Android, keep what the user was doing when Android ends the process in the background:
  return it from `SlintSaveState`, and take it back in `SlintRestoreState`.
  `SlintPause`, `SlintResume`, and `SlintLowMemory` report the other activity events.
- On Android, `SlintLivePreview` no longer makes the app fail at startup, looking for the `.slint` files at their paths on the build machine.
  The build ignores it there, with the warning `SLINT002`, since live reload isn't available on Android yet.
- Update to the latest Slint, where `SLINT_ENABLE_EXPERIMENTAL_FEATURES` takes the names of the experimental features,
  separated by commas, such as `match,closures`, instead of `1`.

## 1.19.0-preview.7

- Run your app on Android, with a main activity derived from `SlintActivity`.
  See [Android](README.md#android-preview) and `examples/todo/dotnet-android`.
- `.slint` code can add and remove rows of a `ListModel`, with `push`, `insert`, and `remove` of an array.
  Override `InsertRow` and `RemoveRow` to support them in your own `Model<T>`.
- A global exported under more names gets a class for each, like a component, such as `MyGlobal` and `GlobalAlias` for `export { MyGlobal as GlobalAlias }`.
- On Windows, a `<SlintFile>` outside the project directory, such as `../ui/app.slint`, loads from the output directory.
  Before, the application failed at startup because it looked for the file outside the output directory.
- Notifying Slint of model changes no longer allocates.
- Measure Slint with `examples/lols`, a port of the [LOLs per second](https://github.com/jonathanpeppers/lols) test, on the desktop and Android.

## 1.19.0-preview.6

- Send key, pointer, and other events to a window with `Window.DispatchEvent`.
- Attach an object of your application to a drag with `DataTransfer.UserData`.
- Map a row of a filtered or sorted view back to its source with `SourceRow`, and apply a changed condition with `Reset`.
- Save a keyboard shortcut with `Keys.ToParts`, and create it again with `Keys.FromParts`.
- Create images from pixels with `Image.FromRgba8` and `Image.FromRgb8`, decode image files in memory with `Image.LoadFromData`, and read the pixels with `ToRgba8`.
- A component that inherits `SystemTrayIcon` has `Show` and `Hide`, but no `Run` or `Window`, since it has no window.
- Find C# versions of 27 examples and demos next to their Rust, C++, Node.js, and Python versions, such as `examples/todo/dotnet`,
  and show any `.slint` file with `examples/dotnet-viewer`.

## 1.19.0-preview.5

- Use a `slint-project.json` with `<SlintProject Include="ui" />`.
  Your application then compiles with the project's edition and paths.
- Changing a `.slint` file no longer makes every later build generate the C# code again.
- The README links to the [Slint for .NET documentation](https://yurkinh.github.io/slint/).

## 1.19.0-preview.4

- Generated records start with the default values that a struct declares for its fields.
  A field that a struct from your code lacks takes its default, too.
- Numbers convert to integer types with saturation and truncation, like in Slint,
  instead of throwing an `OverflowException` or rounding.
- Update to the latest Slint, including Slint editions.
- When a live reload removes a property or callback that your code sets, the application prints a warning and keeps running.
  See [What Needs a Restart](README.md#what-needs-a-restart).
- On Windows, `Create`, `CreateLive`, and `Run` throw an `InvalidOperationException` on a thread without `[STAThread]`,
  instead of failing later in the windowing system.

## 1.19.0-preview.3

- Use the [`Window`](README.md#window) API to show, hide, move, resize, and maximize windows.
- Pass `Brush`, colors, and typed positions and sizes to properties.
- Translate your UI with [translations](README.md#translations).
- Filter, sort, map, and reverse [models](README.md#models) with model adapters.
- Use `Keys`, `StyledText`, `DataTransfer`, and component aliases.
- Pass include and library paths to the generated code with `SlintIncludePaths` and `<SlintLibraryPath>`.

## 1.19.0-preview.2

- See your changes to `.slint` files without restarting, with [hot reload](README.md#hot-reload).

## 1.19.0-preview.1

- The first preview: generate C# from `.slint` files, set and read properties, handle callbacks,
  and use models, timers, and async code.
