<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Types

All properties in Slint have a type. Slint knows these basic types:

| Type                 | Description                                                                                                                                                                                                                                                                                                                                      | Default value |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------- |
| `angle`              | Angle measurement, corresponds to a literal like `90deg`, `1.2rad`, `0.25turn`                                                                                                                                                                                                                                                                   | 0deg          |
| `bool`               | boolean whose value can be either `true` or `false`.                                                                                                                                                                                                                                                                                             | false         |
| `brush`              | A brush is a special type that can be either initialized from a color or a gradient specification. See the [Colors and Brushes Section](#colors-and-brushes) for more information.                                                                                                                                                               | transparent   |
| `color`              | RGB color with an alpha channel, with 8 bit precision for each channel. CSS color names as well as the hexadecimal color encodings are supported, such as `#RRGGBBAA` or `#RGB`.                                                                                                                                                                 | transparent   |
| `duration`           | Type for the duration of animations. A suffix like `ms` (millisecond) or `s` (second) is used to indicate the precision.                                                                                                                                                                                                                         | 0ms           |
| `easing`             | Property animation allow specifying an easing curve. Valid values are `linear` (values are interpolated linearly) and the [four common cubiz-bezier functions known from CSS](https://developer.mozilla.org/en-US/docs/Web/CSS/easing-function#Keywords_for_common_cubic-bezier_easing_functions): `ease`, `ease_in`, `ease_in_out`, `ease_out`. | linear        |
| `float`              | Signed, 32-bit floating point number. Numbers with a `%` suffix are automatically divided by 100, so for example `30%` is the same as `0.30`.                                                                                                                                                                                                    | 0             |
| `image`              | A reference to an image, can be initialized with the `@image-url("...")` construct                                                                                                                                                                                                                                                               | empty image   |
| `int`                | Signed integral number.                                                                                                                                                                                                                                                                                                                          | 0             |
| `length`             | The type used for `x`, `y`, `width` and `height` coordinates. Corresponds to a literal like `1px`, `1pt`, `1in`, `1mm`, or `1cm`. It can be converted to and from length provided the binding is run in a context where there is an access to the device pixel ratio.                                                                            | 0px           |
| `percent`            | Signed, 32-bit floating point number that is interpreted as percentage. Literal number assigned to properties of this type must have a `%` suffix.                                                                                                                                                                                               | 0%            |
| `physical-length`    | This is an amount of physical pixels. To convert from an integer to a length unit, one can simply multiply by `1px`. Or to convert from a length to a float, one can divide by `1phx`.                                                                                                                                                           | 0phx          |
| `relative-font-size` | Relative font size factor that is multiplied with the `Window.default-font-size` and can be converted to a `length`.                                                                                                                                                                                                                             | 0rem          |
| `string`             | UTF-8 encoded, reference counted string.                                                                                                                                                                                                                                                                                                         | `""`          |

Please see the language specific API references how these types are mapped to the APIs of the different programming languages.

## Strings

Any sequence of utf-8 encoded characters surrounded by quotes is a `string`: `"foo"`.

Escape sequences may be embedded into strings to insert characters that would
be hard to insert otherwise:

| Escape          | Result                                                                                          |
| --------------- | ----------------------------------------------------------------------------------------------- |
| `\"`            | `"`                                                                                             |
| `\\`            | `\`                                                                                             |
| `\n`            | new line                                                                                        |
| `\u{x}`         | where `x` is a hexadecimal number, expands to the unicode code point represented by this number |
| `\{expression}` | the result of evaluating the expression                                                         |

Anything else following an unescaped `\` is an error.

```slint,no-preview
export component Example inherits Text {
    text: "hello";
}
```

Note: The `\{...}` syntax is not valid within the `slint!` macro in Rust.

## Colors and Brushes

Color literals follow the syntax of CSS:

```slint,no-preview
export component Example inherits Window {
    background: blue;
    property<color> c1: #ffaaff;
    property<brush> b2: Colors.red;
}
```

In addition to plain colors, many elements have properties that are of type `brush` instead of `color`.
A brush is a type that can be either a color or gradient. The brush is then used to fill an element or
draw the outline.

CSS Color names are only in scope in expressions of type `color` or `brush`. Otherwise, you can access
colors from the `Colors` namespace.

### Methods

All colors and brushes define the following methods:

-   **`brighter(factor: float) -> brush`**

    Returns a new color derived from this color but has its brightness increased by the specified factor.
    For example if the factor is 0.5 (or for example 50%) the returned color is 50% brighter. Negative factors
    decrease the brightness.

-   **`darker(factor: float) -> brush`**

    Returns a new color derived from this color but has its brightness decreased by the specified factor.
    For example if the factor is .5 (or for example 50%) the returned color is 50% darker. Negative factors
    increase the brightness.

-   **`mix(other: brush, factor: float) -> brush`**

    Returns a new color that is a mix of this color and `other`, with a proportion
    factor given by \a factor (which will be clamped to be between `0.0` and `1.0`).

-  **`transparentize(factor: float) -> brush`**

    Returns a new color with the opacity decreased by `factor`.
    The transparency is obtained by multiplying the alpha channel by `(1 - factor)`.


-  **`with_alpha(alpha: float) -> brush`**

    Returns a new color with the alpha value set to `alpha` (between 0 and 1)

### Linear Gradients

Linear gradients describe smooth, colorful surfaces. They're specified using an angle and a series of
color stops. The colors will be linearly interpolated between the stops, aligned to an imaginary line
that is rotated by the specified angle. This is called a linear gradient and is specified using the
`@linear-gradient` macro with the following signature:

**`@linear-gradient(angle, color percentage, color percentage, ...)`**

The first parameter to the macro is an angle (see [Types](types.md)). The gradient line's starting point
will be rotated by the specified value.

Following the initial angle is one or multiple color stops, describe as a space separated pair of a
`color` value and a `percentage`. The color specifies which value the linear color interpolation should
reach at the specified percentage along the axis of the gradient.

The following example shows a rectangle that's filled with a linear gradient that starts with a light blue
color, interpolates to a very light shade in the center and finishes with an orange tone:

```slint
export component Example inherits Window {
    preferred-width: 100px;
    preferred-height: 100px;

    Rectangle {
        background: @linear-gradient(90deg, #3f87a6 0%, #ebf8e1 50%, #f69d3c 100%);
    }
}
```

### Radial Gradients

Linear gradiants are like real gradiant but the colors is interpolated in a circle instead of
along a line. To describe a readial gradiant, use the `@radial-gradient` macro with the following signature:

**`@radial-gradient(circle, color percentage, color percentage, ...)`**

The first parameter to the macro is always `circle` because only circular radients are supported.
The syntax is otherwise based on the CSS `radial-gradient` function.

Example:

```slint
export component Example inherits Window {
    preferred-width: 100px;
    preferred-height: 100px;
    Rectangle {
        background: @radial-gradient(circle, #f00 0%, #0f0 50%, #00f 100%);
    }
}
```

## Images

The `image` type is a reference to an image. It's defined using the `@image-url("...")` construct.
The address within the `@image-url` function must be known at compile time.

Slint looks for images in the following places:

1. The absolute path or the path relative to the current `.slint` file.
2. The include path used by the compiler to look up `.slint` files.

Access an `image`'s dimension using its `width` and `height` properties.

```slint
export component Example inherits Window {
    preferred-width: 150px;
    preferred-height: 50px;

    in property <image> some_image: @image-url("https://slint.dev/logo/slint-logo-full-light.svg");

    Text {
        text: "The image is " + some_image.width + "x" + some_image.height;
    }
}
```

## Structs

Define named structures using the `struct` keyword:

```slint,no-preview
export struct Player  {
    name: string,
    score: int,
}

export component Example {
    in-out property<Player> player: { name: "Foo", score: 100 };
}
```

The default value of a struct, is initialized with all its fields set to their default value.

### Anonymous Structures

Declare anonymous structures using `{ identifier1: type2, identifier1: type2 }`
syntax, and initialize them using
`{ identifier1: expression1, identifier2: expression2  }`.

You may have a trailing `,` after the last expression or type.

```slint,no-preview
export component Example {
    in-out property<{name: string, score: int}> player: { name: "Foo", score: 100 };
    in-out property<{a: int, }> foo: { a: 3 };
}
```

## Enumerations

Define an enumeration with the `enum` keyword:

```slint,no-preview
export enum CardSuit { clubs, diamonds, hearts, spade }

export component Example {
    in-out property<CardSuit> card: spade;
    out property<bool> is-clubs: card == CardSuit.clubs;
}
```

Enum values can be referenced by using the name of the enum and the name of the value
separated by a dot. (eg: `CardSuit.spade`)

The name of the enum can be omitted in bindings of the type of that enum, or if the
return value of a callback is of that enum.

The default value of each enum type is always the first value.

## Arrays and Models

Arrays are declared by wrapping `[` and `]` square brackets around the type of the array elements.

Array literals as well as properties holding arrays act as models in`for` expressions.

```slint,no-preview
export component Example {
    in-out property<[int]> list-of-int: [1,2,3];
    in-out property<[{a: int, b: string}]> list-of-structs: [{ a: 1, b: "hello" }, {a: 2, b: "world"}];
}
```

Arrays define the following operations:

-   **`array.length`**: One can query the length of an array and model using the builtin `.length` property.
-   **`array[index]`**: The index operator retrieves individual elements of an array.

Out of bound access into an array will return default-constructed values.

```slint,no-preview
export component Example {
    in-out property<[int]> list-of-int: [1,2,3];

    out property <int> list-len: list-of-int.length;
    out property <int> first-int: list-of-int[0];
}

```

## Conversions

Slint supports conversions between different types. Explicit
conversions are required to make the UI description more robust, but implicit
conversions are allowed between some types for convenience.

The following conversions are possible:

-   `int` can be converted implicitly to `float` and vice-versa
-   `int` and `float` can be converted implicitly to `string`
-   `physical-length` and `length` can be converted implicitly to each other only in
    context where the pixel ratio is known.
-   the units type (`length`, `physical-length`, `duration`, ...) can't be converted to numbers (`float` or `int`)
    but they can be divided by themselves to result in a number. Similarly, a number can be multiplied by one of
    these unit. The idea is that one would multiply by `1px` or divide by `1px` to do such conversions
-   The literal `0` can be converted to any of these types that have associated unit.
-   Struct types convert with another struct type if they have the same property names and their types can be converted.
    The source struct can have either missing properties, or extra properties. But not both.
-   Arrays generally don't convert between each other. Array literals can be converted if the element types are convertible.
-   String can be converted to float by using the `to-float` function. That function returns 0 if the string isen't
    a valid number. You can check with `is-float()` if the string contains a valid number

```slint,no-preview
export component Example {
    // OK: int converts to string
    property<{a: string, b: int}> prop1: {a: 12, b: 12 };
    // OK: even if a is missing, it will just have the default value ("")
    property<{a: string, b: int}> prop2: { b: 12 };
    // OK: even if c is too many, it will be discarded
    property<{a: string, b: int}> prop3: { a: "x", b: 12, c: 42 };
    // ERROR: b is missing and c is extra, this doesn't compile, because it could be a typo.
    // property<{a: string, b: int}> prop4: { a: "x", c: 42 };

    property<string> xxx: "42.1";
    property<float> xxx1: xxx.to-float(); // 42.1
    property<bool> xxx2: xxx.is-float(); // true
}
```
