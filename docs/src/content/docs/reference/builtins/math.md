---
<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
title: Math 
description: Math Namespace
---


These functions are available both in the global scope and in the `Math` namespace.

They can also be called directly as member function of numeric types. (For example `angle.sin()` or `foo.mod(10)`).

### `abs(T) -> T`

Return the absolute value, where T is a numeric type.

### `acos(float) -> angle`, `asin(float) -> angle`, `atan(float) -> angle`, `atan2(float, float) -> angle`, `cos(angle) -> float`, `sin(angle) -> float`, `tan(angle) -> float`

The trigonometry function. Note that the should be typed with `deg` or `rad` unit
(for example `cos(90deg)` or `sin(slider.value * 1deg)`).

### `ceil(float) -> int` and `floor(float) -> int`

Return the ceiling or floor

### `clamp(T, T, T) -> T`

Takes a `value`, `minimum` and `maximum` and returns `maximum` if
`value > maximum`, `minimum` if `value < minimum`, or `value` in all other cases.

### `log(float, float) -> float`

Return the log of the first value with a base of the second value

### `max(T, T) -> T` and `min(T, T) -> T`

Return the arguments with the minimum (or maximum) value. All arguments must be of the same numeric type

### `mod(T, T) -> T`

Perform a modulo operation, where T is some numeric type.
Returns the remainder of the euclidean division of the arguments.
This always returns a positive number between 0 and the absolute value of the second value.

### `round(float) -> int`

Return the value rounded to the nearest integer

### `sqrt(float) -> float`

Square root

### `pow(float, float) -> float`

Return the value of the first value raised to the second
