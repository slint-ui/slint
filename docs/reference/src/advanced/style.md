<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Selecting a Widget Style

Slint offers a variety of [built-in widgets](../language/widgets/widgets.md) which can be imported from `"std-widgets.slint"`. You can modify the look of these widgets by choosing a style.

The styles available include:

| Style Name | Light Variant | Dark Variant | Description |
|------------|--------------|--------------|------------|
| `fluent`   | `fluent-light`| `fluent-dark`| These variants belong to the **Fluent** style, which is based on the [Fluent Design System](https://fluent2.microsoft.design/). |
| `material` | `material-light`| `material-dark`| These variants are part of the **Material** style, which follows the [Material Design](https://m3.material.io). |
| `cupertino`| `cupertino-light`| `cupertino-dark`| The **Cupertino** variants emulate the style used by macOS. |
| `cosmic`| `cosmic-light`| `cosmic-dark`| The **Cosmic** variants emulate the style used by [Cosmic Desktop](https://github.com/pop-os/cosmic). |
| `qt`   | | | The **Qt** style uses [Qt](https://en.wikipedia.org/wiki/Qt_(software)) to render widgets. This style requires Qt to be installed on your system. |
| `native` | | | This is an alias to one of the other styles depending on the platform. It is `cupertino` on macOS, `fluent` on Windows, `material` on Android, `qt` on linux if Qt is available, or `fluent` otherwise. |


By default, the styles automatically adapt to the system's dark or light color setting. Select a `-light` or `-dark` variant to override the system setting and always show either dark or light colors.

The widget style is determined at your project's compile time. The method to select a style depends on how you use Slint.

If no style is selected, `native` is the default.


## Selecting a Widget Style with Rust:

You can select the style before starting your compilation by setting the `SLINT_STYLE` environment variable to the name of your chosen style.

When using the `slint_build` API, call the [`slint_build::compile_with_config()`](https://docs.rs/slint-build/newest/slint_build/fn.compile_with_config.html) function.

When using the `slint_interpeter` API, call the [`slint_interpreter::ComponentCompiler::set_style()`](https://docs.rs/slint-interpreter/newest/slint_interpreter/struct.ComponentCompiler.html#method.set_style) function.

## Selecting a Widget Style when using C++:

Define a `SLINT_STYLE` CMake cache variable to contain the style name as a string. This can be done, for instance, on the command line:

```sh
cmake -DSLINT_STYLE="material" /path/to/source
```

## Selecting a Widget Style when using Node.js:

You can select the style by setting the `style` property in [`LoadFileOptions`](slint-node:interfaces/LoadFileOptions) passed to [`loadFile`](slint-node:functions/loadFile):

```js
import * as slint from "slint-ui";
let ui = slint.loadFile("main.slint", { style: "fluent" });
let main = new ui.Main();
main.greeting = "Hello friends";
```


## Previewing Designs With `slint-viewer`

Select the style either by setting the `SLINT_STYLE` environment variable, or by passing the style name with the `--style` argument:

slint-viewer --style material /path/to/design.slint

## Previewing Designs With The Slint Visual Studio Code Extension

To select the style, first open the Visual Studio Code settings editor:

-   On Windows/Linux - File > Preferences > Settings
-   On macOS - Code > Preferences > Settings

Then enter the style name in Extensions > Slint > Preview:Style

## Previewing Designs With The Generic LSP Process

Choose the style by setting the `SLINT_STYLE` environment variable before launching the process.
Alternatively, if your IDE integration allows for command line parameters, you can specify the style using `--style`.
