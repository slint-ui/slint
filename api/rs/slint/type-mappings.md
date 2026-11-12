# Type Mappings

Each property type in the Slint language maps to a specific Rust type.
The following table summarizes the mapping:

| `.slint` Type | Rust Type | Note |
| --- | --- | --- |
| `angle` | `f32` | The angle in degrees. |
| `array` | [`ModelRc`] | Slint represents arrays as models, so their contents can change dynamically. |
| `bool` | `bool` | |
| `brush` | [`Brush`] | |
| `color` | [`Color`] | |
| `duration` | `i64` | At run-time, durations are always represented as signed 64-bit integers with millisecond precision. |
| `float` | `f32` | |
| `image` | [`Image`] | |
| `int` | `i32` | |
| `length` | `f32` | At run-time, Slint translates logical lengths to physical pixels using the device pixel ratio. |
| `physical-length` | `f32` | The unit is physical pixels. |
| `Point` | [`LogicalPosition`] | A struct with `x` and `y` fields, representing logical coordinates. |
| `relative-font-size` | `f32` | Relative font size factor that is multiplied with the `Window.default-font-size` and can be converted to a `length`. |
| `string` | [`SharedString`] | A reference-counted string type that converts easily to a `&str`. |
| `styled-text` | [`StyledText`] | To create values in Rust, use [`StyledText::from_markdown`](crate::StyledText::from_markdown) or [`StyledText::from_plain_text`](crate::StyledText::from_plain_text). |
| anonymous struct | tuple | The fields are in alphabetical order. |
| enumeration | `enum` of the same name | The values are converted to CamelCase. |
| structure | `struct` of the same name | |

For each user-defined structure in your `.slint` file, Slint generates a Rust struct.
For example, consider this structure:

```slint,ignore
export struct MyStruct {
    foo: int,
    bar: string,
    names: [string],
}
```

Slint generates the following Rust struct:

```rust
#[derive(Default, Clone, Debug, PartialEq)]
struct MyStruct {
    foo: i32,
    bar: slint::SharedString,
    names: slint::ModelRc<slint::SharedString>,
}
```

If you use an anonymous struct, you work with a tuple of its field values, in alphabetical order.
For example, consider this property:

```slint,ignore
export component Example {
    in-out property <{score: int, name: string}> player;
}
```

The generated accessors take and return a `(slint::SharedString, i32)`:

```rust,ignore
example.set_player(("Foo".into(), 100));
let (name, score) = example.get_player();
```

**Note:** `name` comes first, although the property declares `score` first.

To customize a generated struct with traits, derivations, or other Rust attributes, use the `@rust-attr()` directive in your `.slint` file.
For example, consider this structure with Rust attributes:

```slint,ignore
@rust-attr(derive(serde::Serialize, serde::Deserialize))
struct MyStruct {
    foo: int,
}
```

Slint generates the following Rust code for it:

```rust
#[derive(serde::Serialize, serde::Deserialize)]
#[derive(Default, Clone, Debug, PartialEq)]
struct MyStruct {
    foo: i32,
}
```

You can also apply multiple `@rust-attr()` directives to one structure:

```slint,ignore
@rust-attr(derive(serde::Serialize, serde::Deserialize))
@rust-attr(serde(rename_all = "camelCase"))
struct MyStruct {
    foo-bar: int,
}
```
