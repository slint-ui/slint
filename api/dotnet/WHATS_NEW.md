# What's New in Slint for .NET

## Unreleased

- Send key, pointer, and other events to a window with `Window.DispatchEvent`.
- Attach an object of your application to a drag with `DataTransfer.UserData`.
- A component that inherits `SystemTrayIcon` has `Show` and `Hide`, but no `Run` or `Window`, since it has no window.

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
