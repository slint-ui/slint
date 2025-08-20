<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Type Mappings

The types used for properties in `.slint` design markup each translate to specific types in Rust.
The follow table summarizes the entire mapping:

| `.slint` Type | Rust Type | Note |
| --- | --- | --- |
| `angle` | `f32` | The angle in degrees |
| `array` | [`ModelRc`] | Arrays are represented as models, so that their contents can change dynamically. |
| `bool` | `bool` | |
| `brush` | [`Brush`] | |
| `color` | [`Color`] | |
| `duration` | `i64` | At run-time, durations are always represented as signed 64-bit integers with millisecond precision. |
| `float` | `f32` | |
| `image` | [`Image`] | |
| `int` | `i32` | |
| `length` | `f32` | At run-time, logical lengths are automatically translated to physical pixels using the device pixel ratio. |
| `physical_length` | `f32` | The unit are physical pixels. |
| `Point` | [`LogicalPosition`] | A struct with `x` and `y` fields, representing logical coordinates. |
| `relative-font-size` | `f32` | Relative font size factor that is multiplied with the `Window.default-font-size` and can be converted to a `length`. |
| `string` | [`SharedString`] | A reference-counted string type that can be easily converted to a str reference. |
| anonymous object | anonymous tuple | The fields are in alphabetical order. |
| enumeration | `enum` of the same name | The values are converted to CamelCase |
| structure | `struct` of the same name | |

For user defined structures in the .slint, an extra struct is generated.
For example, if the `.slint` contains
```slint,ignore
export struct MyStruct {
    foo: int,
    bar: string,
    names: [string],
}
```

The following struct would be generated:

```rust
#[derive(Default, Clone, Debug, PartialEq)]
struct MyStruct {
    foo: i32,
    bar: slint::SharedString,
    names: slint::ModelRc<slint::SharedString>,
}
```

The `.slint` file allows you to utilize Rust attributes and features for defining structures using the `@rust-attr()` directive.
This enables you to customize the generated code by applying additional traits, derivations, or annotations.
Consider the following structure defined in the `.slint` file with Rust attributes:
```slint,ignore
@rust-attr(derive(serde::Serialize, serde::Deserialize))
struct MyStruct {
    foo: int,
}
```

Based on this structure, the following Rust code would be generated:

```rust
#[derive(serde::Serialize, serde::Deserialize)]
#[derive(Default, Clone, Debug, PartialEq)]
struct MyStruct {
    foo: i32,
}
```
