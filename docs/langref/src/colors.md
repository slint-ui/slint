## `Colors` namespace

These functions are available both in the global scope, and in the `Colors` namespace.

* **`rgb(int, int, int) -> color`**,  **`rgba(int, int, int, float) -> color`**

Return the color as in CSS. Like in CSS, these two functions are actually aliases that can take
three or four parameters.

The first 3 parameters can be either number between 0 and 255, or a percentage with a `%` unit.
The fourth value, if present, is an alpha value between 0 and 1.

Unlike in CSS, the commas are mandatory.
