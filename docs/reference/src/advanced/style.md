<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Selecting a Widget Style

The widget style is selected at compile time of your project. The details depend on which programming
language you're using Slint with.

<details data-snippet-language="rust">
<summary>Selecting a Widget Style when using Slint with Rust:</summary>

Before you start your compilation, you can select the style by setting the `SLINT_STYLE` variable
to one of the style names, such as `fluent` for example.

## Using the `slint_build` Crate

Select the style with the [`slint_build::compile_with_config()`](https://docs.rs/slint-build/newest/slint_build/fn.compile_with_config.html) function in the compiler configuration argument.

## Using the `slint_interpreter` Crate

Select the style with the [`slint_interpreter::ComponentCompiler::set_style()`](https://docs.rs/slint-interpreter/newest/slint_interpreter/struct.ComponentCompiler.html#method.set_style) function.

</details>

<details data-snippet-language="cpp">
<summary>Selecting a Widget Style when using Slint with C++:</summary>

Select the style by defining a `SLINT_STYLE` CMake cache variable to hold the style name as a string. This can be done for example on the command line:

```sh
cmake -DSLINT_STYLE="material" /path/to/source
```

</details>

## Previewing Designs With `slint-viewer`

Select the style either by setting the `SLINT_STYLE` environment variable, or passing the style name with the `--style` argument:

```sh
slint-viewer --style material /path/to/design.slint
```

## Previewing Designs With The Slint Visual Studio Code Extension

Select the style by first opening the Visual Studio Code settings editor:

-   On Windows/Linux - File > Preferences > Settings
-   On macOS - Code > Preferences > Settings

Then enter the style name under Extensions > Slint > Preview:Style

## Previewing Designs With The Generic LSP Process

Select the style by setting the `SLINT_STYLE` environment variable before launching the process.
Alternatively, if your IDE integration allows passing command line parameters, you can specify the style via `--style`.
