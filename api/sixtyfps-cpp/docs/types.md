# Type Mappings

The types used for properties in `.60` design markup each translate to specific types in C++.
The follow table summarizes the entire mapping:

| `.60` Type | C++ Type | Note |
| --- | --- | --- |
| `int` | `int` | |
| `float` | `float` | |
| `string` | [`sixtyfps::SharedString`](api/structsixtyfps_1_1_shared_string.html) | A reference-counted string type that uses UTF-8 encoding and can be easily converted to a std::string_view or a const char *. |
| `color` | [`sixtyfps::Color`](api/classsixtyfps_1_1_color.html) | |
| `length` | `float` | The unit are physical pixels. |
| `logical_length` | `float` | At run-time, logical lengths are automatically translated to physical pixels using the device pixel ratio. |
| `duration` | `std::int64_t` | At run-time, durations are always represented as signed 64-bit integers with milisecond precision. |