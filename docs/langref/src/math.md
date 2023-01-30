## `Math` namespace

These functions are available both in the global scope and in the `Math` namespace.

* **`min`**, **`max`**

Return the arguments with the minimum (or maximum) value. All arguments must be of the same numeric type

* **`mod(T, T) -> T`**

Perform a modulo operation, where T is some numeric type.

* **`abs(float) -> float`**

Return the absolute value.

* **`round(float) -> int`**

Return the value rounded to the nearest integer

* **`ceil(float) -> int`**, **`floor(float) -> int`**

Return the ceiling or floor

* **`sin(angle) -> float`**, **`cos(angle) -> float`**, **`tan(angle) -> float`**, **`asin(float) -> angle`**, **`acos(float) -> angle`**, **`atan(float) -> angle`**

The trigonometry function. Note that the should be typed with `deg` or `rad` unit
(for example `cos(90deg)` or `sin(slider.value * 1deg)`).

* **`sqrt(float) -> float`**

Square root

* **`pow(float, float) -> float`**

Return the value of the first value raised to the second

* **`log(float, float) -> float`**

Return the log of the first value with a base of the second value
