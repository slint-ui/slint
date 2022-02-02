# Type Mappings

The types used for properties in `.slint` design markup each translate to specific types in C++.
The follow table summarizes the entire mapping:

| `.slint` Type | C++ Type | Note |
| --- | --- | --- |
| `int` | `int` | |
| `float` | `float` | |
| `bool` | `bool` | |
| `string` | [`sixtyfps::SharedString`](api/structsixtyfps_1_1_shared_string.html) | A reference-counted string type that uses UTF-8 encoding and can be easily converted to a std::string_view or a const char *. |
| `color` | [`sixtyfps::Color`](api/classsixtyfps_1_1_color.html) | |
| `brush` | [`sixtyfps::Brush`](api/classsixtyfps_1_1_brush.html) | |
| `image` | [`sixtyfps::Image`](api/structsixtyfps_1_1_image.html) | |
| `physical_length` | `float` | The unit are physical pixels. |
| `length` | `float` | At run-time, logical lengths are automatically translated to physical pixels using the device pixel ratio. |
| `duration` | `std::int64_t` | At run-time, durations are always represented as signed 64-bit integers with millisecond precision. |
| `angle` | `float` | The value in degrees. |
| structure | A `class` of the same name | The order of the data member are in the lexicographic order of their name |

## Structures

For user-defined structures in the .slint code, a `class` of the same name is generated with data member
in lexicographic order.

For example, if you have this structure in the .slint file

```slint,ignore
export struct MyStruct := {
    foo: int,
    bar: string,
}
```

It would result in the following type being generated:

```cpp
class MyStruct {
public:
    sixtyfps::SharedString bar;
    int foo;
};
```
