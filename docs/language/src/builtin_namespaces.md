# Builtin Namespaces

The following namespaces provide access to common constants such as special keys or named colors.

## `Colors`

Use the colors namespace to select colors by their name. For example you can use `Colors.aquamarine` or `Colors.bisque`.
The entire list of names is very long. You can find a complete list in the [CSS Specification](https://www.w3.org/TR/css-color-3/#svg-color).

These functions are available both in the global scope, and in the `Colors` namespace.

-   **`rgb(int, int, int) -> color`**, **`rgba(int, int, int, float) -> color`**

Return the color as in CSS. Like in CSS, these two functions are actually aliases that can take
three or four parameters.

The first 3 parameters can be either number between 0 and 255, or a percentage with a `%` unit.
The fourth value, if present, is an alpha value between 0 and 1.

Unlike in CSS, the commas are mandatory.

## `Key`

Use the constants in the `Key` namespace to handle pressing of keys that don't have a printable character. Check the value of [`KeyEvent`](builtin_structs.md#keyevent)'s `text` property
against the constants below.

-   **`Backspace`**
-   **`Tab`**
-   **`Return`**
-   **`Escape`**
-   **`Backtab`**
-   **`Delete`**
-   **`Shift`**
-   **`Control`**
-   **`Alt`**
-   **`AltGr`**
-   **`CapsLock`**
-   **`ShiftR`**
-   **`ControlR`**
-   **`Meta`**
-   **`MetaR`**
-   **`UpArrow`**
-   **`DownArrow`**
-   **`LeftArrow`**
-   **`RightArrow`**
-   **`F1`**
-   **`F2`**
-   **`F3`**
-   **`F4`**
-   **`F5`**
-   **`F6`**
-   **`F7`**
-   **`F8`**
-   **`F9`**
-   **`F10`**
-   **`F11`**
-   **`F12`**
-   **`F13`**
-   **`F14`**
-   **`F15`**
-   **`F16`**
-   **`F17`**
-   **`F18`**
-   **`F19`**
-   **`F20`**
-   **`F21`**
-   **`F22`**
-   **`F23`**
-   **`F24`**
-   **`Insert`**
-   **`Home`**
-   **`End`**
-   **`PageUp`**
-   **`PageDown`**
-   **`ScrollLock`**
-   **`Pause`**
-   **`SysReq`**
-   **`Stop`**
-   **`Menu`**

## `Math`

These functions are available both in the global scope and in the `Math` namespace.

### `abs(float) -> float`

Return the absolute value.

### `acos(float) -> angle`, `asin(float) -> angle`, `atan(float) -> angle`, `cos(angle) -> float`, `sin(angle) -> float`, `tan(angle) -> float`

The trigonometry function. Note that the should be typed with `deg` or `rad` unit
(for example `cos(90deg)` or `sin(slider.value * 1deg)`).

### `ceil(float) -> int` and `floor(float) -> int`

Return the ceiling or floor

### `log(float, float) -> float`

Return the log of the first value with a base of the second value

### `max(T, T) -> T` and `min(T, T) -> T`

Return the arguments with the minimum (or maximum) value. All arguments must be of the same numeric type

### `mod(T, T) -> T`

Perform a modulo operation, where T is some numeric type.

### `round(float) -> int`

Return the value rounded to the nearest integer

### `sqrt(float) -> float`

Square root

### `pow(float, float) -> float`

Return the value of the first value raised to the second
