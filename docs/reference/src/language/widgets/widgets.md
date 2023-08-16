<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

Slint provides a series of built-in widgets that can be imported from `"std-widgets.slint"`.

The widget appearance depends on the selected style. The following styles are available:

-   `fluent`: The **Fluent** style implements the [Fluent Design System](https://www.microsoft.com/design/fluent/).
-   `material`: The **Material** style implements the [Material Design](https://m3.material.io).
-   `native`: The **Native** style resembles the appearance of the controls that are native to the platform they
    are used on. This specifically includes support for the look and feel of controls on macOS and Windows. This
    style is only available if you have Qt installed on your system.

See [Selecting a Widget Style](../../advanced/style.md#selecting-a-widget-style) for details how to select the style. If no style is selected, `native` is the default. If `native` isn't available, `fluent` is the default.

All widgets support all [properties common to builtin elements](../builtins/elements.md#common-properties).

