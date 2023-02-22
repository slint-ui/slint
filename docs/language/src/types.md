# Types

All properties in Slint have a type. Slint knows these basic types:

| Type                 | Description                                                                                                                                                                                                                                                                                                                                      |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `int`                | Signed integral number.                                                                                                                                                                                                                                                                                                                          |
| `float`              | Signed, 32-bit floating point number. Numbers with a `%` suffix are automatically divided by 100, so for example `30%` is the same as `0.30`.                                                                                                                                                                                                    |
| `bool`               | boolean whose value can be either `true` or `false`.                                                                                                                                                                                                                                                                                             |
| `string`             | UTF-8 encoded, reference counted string.                                                                                                                                                                                                                                                                                                         |
| `color`              | RGB color with an alpha channel, with 8 bit precision for each channel. CSS color names as well as the hexadecimal color encodings are supported, such as `#RRGGBBAA` or `#RGB`.                                                                                                                                                                 |
| `brush`              | A brush is a special type that can be either initialized from a color or a gradient specification. See the [Colors and Brushes Section](#colors-and-brushes) for more information.                                                                                                                                                               |
| `physical-length`    | This is an amount of physical pixels. To convert from an integer to a length unit, one can simply multiply by `1px`. Or to convert from a length to a float, one can divide by `1phx`.                                                                                                                                                           |
| `length`             | The type used for `x`, `y`, `width` and `height` coordinates. Corresponds to a literal like `1px`, `1pt`, `1in`, `1mm`, or `1cm`. It can be converted to and from length provided the binding is run in a context where there is an access to the device pixel ratio.                                                                            |
| `duration`           | Type for the duration of animations. A suffix like `ms` (millisecond) or `s` (second) is used to indicate the precision.                                                                                                                                                                                                                         |
| `angle`              | Angle measurement, corresponds to a literal like `90deg`, `1.2rad`, `0.25turn`                                                                                                                                                                                                                                                                   |
| `easing`             | Property animation allow specifying an easing curve. Valid values are `linear` (values are interpolated linearly) and the [four common cubiz-bezier functions known from CSS](https://developer.mozilla.org/en-US/docs/Web/CSS/easing-function#Keywords_for_common_cubic-bezier_easing_functions): `ease`, `ease_in`, `ease_in_out`, `ease_out`. |
| `percent`            | Signed, 32-bit floating point number that is interpreted as percentage. Literal number assigned to properties of this type must have a `%` suffix.                                                                                                                                                                                               |
| `image`              | A reference to an image, can be initialized with the `@image-url("...")` construct                                                                                                                                                                                                                                                               |
| `relative-font-size` | Relative font size factor that is multiplied with the `Window.default-font-size` and can be converted to a `length`.                                                                                                                                                                                                                             |

Please see the language specific API references how these types are mapped to the APIs of the different programming languages.

```{include} strings.md

```

```{include} brushes.md

```

```{include} images.md

```

```{include} structs.md

```

```{include} models.md

```

```{include} conversions.md

```
