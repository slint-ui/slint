# Slint for .NET (Preview)

[Slint](https://slint.dev/) is a UI toolkit that supports different programming languages.
Slint for .NET is the integration with C# and .NET 10.

**Warning**
Slint for .NET is a preview: its APIs may still change.

## Prerequisites

- [.NET 10 SDK](https://dotnet.microsoft.com/download)
- macOS, Linux, or Windows, on x64 or arm64

## Quick Start

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

For imports from a library, such as `import { Button } from "@mylib/button.slint"`, add a library path:

```xml
<SlintLibraryPath Include="mylib" Path="../mylib/" />
```

Paths are relative to the project.
The generated code loads imports from these directories where they are,
so an application that runs elsewhere needs the imported files copied, such as with `<SlintFile Generate="false" />`.

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

## Models

Use a `ListModel<T>` for the rows of a `for` repeater, or any array property.
Slint updates the UI whenever you change the list:

```csharp
var todos = new ListModel<TodoItem>();
app.Todos = todos;
todos.Add(new TodoItem { Title = "Write the docs" });
```

To show your own data, derive from `Model<T>`,
and call `NotifyRowChanged`, `NotifyRowAdded`, `NotifyRowRemoved`, or `NotifyReset` when it changes.

`Filter`, `Sort`, `Map`, and `Reverse` return views of a model that follow its changes:

```csharp
var todos = new ListModel<TodoItem>();
app.OpenTodos = todos.Filter(todo => !todo.Done).Sort((a, b) => a.Title.CompareTo(b.Title));
```

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

### C# Code

.NET Hot Reload applies C# changes, such as the body of a callback handler, to the running application.
Use it in one of these ways:

- Run `dotnet watch`, which applies each change when you save.
- Start the application in the debugger of Visual Studio, VS Code with C# Dev Kit, or Rider,
  and apply changes with the IDE's Hot Reload command, or on save if the IDE has that turned on.

A handler you passed to an `On…` method runs the new code from its next call,
because Slint keeps calling the same delegate.

### Both at Once

With `SlintLivePreview` on, run:

```sh
dotnet watch
```

Saving a `.slint` file reloads the UI, and saving a `.cs` file applies the C# change,
both without a restart and without losing the state of the application.

### What Needs a Restart

- Renaming or removing a property, callback, or function that your C# code uses.
  The generated classes still have the old members, and the application ends at the next reload.
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

## Slint Language Manual

The [Slint Language Documentation](https://slint.dev/docs/slint) covers the Slint UI description language in detail.
