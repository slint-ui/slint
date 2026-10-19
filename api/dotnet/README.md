# Slint for .NET (Preview)

[Slint](https://slint.dev/) is a UI toolkit that supports different programming languages.
Slint for .NET is the integration with C# and .NET 10.

**Warning**
Slint for .NET is a preview: its APIs may still change.

## Prerequisites

- [.NET 10 SDK](https://dotnet.microsoft.com/download)
- macOS, Linux, or Windows, on x64 or arm64

## Quick Start

1. Create a project and add the `Slint` package:

   ```sh
   dotnet new console -o MyApp
   cd MyApp
   dotnet add package Slint --prerelease
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

   using var app = AppWindow.Create();
   app.OnRequestIncreaseValue(() => app.Counter++);
   app.Run();
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

The types map as follows:

| Slint | C# |
|---|---|
| `int` | `int` |
| `float`, `length`, `angle`, `percent` | `float` |
| `duration` | `TimeSpan` |
| `string` | `string` |
| `bool` | `bool` |
| `color` | `Slint.Color` |
| `image` | `Slint.Image` |
| `[T]` | `Slint.Model<T>` |

Other types, such as `brush`, are `object?` for now.

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

## Threading

Slint objects belong to the thread that first uses Slint.
Use them from that thread only, and on macOS, make it the main thread: the one that runs `Main`.

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
