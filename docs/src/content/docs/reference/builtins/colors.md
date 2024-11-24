---
<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
title: Colors
description: Colors Namespaces
---

Use the colors namespace to select colors by their name. For example you can use `Colors.aquamarine` or `Colors.bisque`.
The entire list of names is very long. You can find a complete list in the [CSS Specification](https://www.w3.org/TR/css-color-3/#svg-color).

These functions are available both in the global scope, and in the `Colors` namespace.
```slint no-test
// Using the Colors namespace
background: Colors.aquamarine;

// Using the functions via global scope.
background: aquamarine;
```

### rgb(int, int, int) -> color 
### rgba(int, int, int, float) -> color

Return the color as in CSS. Like in CSS, these two functions are actually aliases that can take
three or four parameters.

The first 3 parameters can be either number between 0 and 255, or a percentage with a `%` unit.
The fourth value, if present, is an alpha value between 0 and 1.

Unlike in CSS, the commas are mandatory.

 - **`hsv(h: float, s: float, v: float) -> color`**, **`hsv(h: float, s: float, v: float, a: float) -> color`**

Return a color computed from the HSV color space. The hue is between 0 and 360.
The saturation, value, and optional alpha parameter are expected to be within the range of 0 and 1.

