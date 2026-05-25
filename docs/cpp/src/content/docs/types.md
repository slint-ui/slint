---
title: Type Mappings
description: How .slint property types map to C++ types, plus the generated classes for structures and enums.
---

The types used for properties in `.slint` design markup each translate to specific types in C++.
The follow table summarizes the entire mapping:

| `.slint` Type | C++ Type | Note |
| --- | --- | --- |
| `int` | `int` | |
| `float` | `float` | |
| `bool` | `bool` | |
| `string` | `slint::SharedString` | A reference-counted string type that uses UTF-8 encoding and can be easily converted to a `std::string_view` or a `const char *`. |
| `color` | `slint::Color` | |
| `brush` | `slint::Brush` | |
| `image` | `slint::Image` | |
| `physical_length` | `float` | The unit are physical pixels. |
| `length` | `float` | At run-time, logical lengths are automatically translated to physical pixels using the device pixel ratio. |
| `duration` | `std::int64_t` | At run-time, durations are always represented as signed 64-bit integers with millisecond precision. |
| `angle` | `float` | The angle in degrees. |
| `relative-font-size` | `float` | Relative font size factor that is multiplied with the `Window.default-font-size` and can be converted to a `length`. |
| structure | A `class` of the same name | The order of the data member are in the same as in the slint declaration. |
| anonymous object | A `std::tuple` | The fields are in alphabetical order. |
| enum | An `enum class` | The values are always converted to CamelCase. The order of the values is the same as in the declaration. |
| `data-transfer` | `slint::DataTransfer` | Data associated with a drag-drop transfer. |
| `styled-text` | `slint::StyledText` | Styled text parsed from markdown or plain text. Use `StyledText::from_markdown()` or `StyledText::from_plain_text()` to create. |
| `Point` | `slint::LogicalPosition` | A struct with `x` and `y` fields, representing logical coordinates. |

## Structures

The Slint compiler generates a `class` with all data members in
the same order for any user-defined, exported `struct` in the `.slint`
code.

For example, this `struct` in a `.slint` file

```slint no-test
export struct MyStruct {
    foo: int,
    bar: string,
}
```

will generate the following type in C++:

```cpp
class MyStruct {
public:
    int foo;
    slint::SharedString bar;
};
```

## Enums

The Slint compiler generates an `enum class` with all values in the same order and converted to camel case
for any user-defined, exported `enum` in the `.slint` code.

For example, this `enum` in a `.slint` file

```slint no-test
export enum MyEnum { alpha, beta-gamma, omicron }
```

will generate the following type in C++:

```cpp
enum class MyEnum { Alpha, BetaGamma, Omicron };
```
