<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

### `dotnet-viewer`

A C# version of `slint-viewer`: it shows any `.slint` file, or a project directory with a `slint-project.json`,
with the `Compiler` API of the .NET bindings instead of generated code.
Use it for the examples whose logic is all in `.slint`, such as `layouts`, `dial`, or `fancy-switches`.

```sh
dotnet run --project examples/dotnet-viewer -- examples/dial/dial.slint
dotnet run --project examples/dotnet-viewer -- examples/fancy-switches/demo.slint --style material
dotnet run --project examples/dotnet-viewer -- examples/repeater/demo.slint --live
```

It shows the last exported component, or the one that `--component` names.
With `--live`, it reloads the component when you save the file or a file it imports.
