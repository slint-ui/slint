# Type Mappings

The types used for properties in `.slint` design markup each translate to specific types in C++.
The follow table summarizes the entire mapping:

```{eval-rst}
===========================  ==================================  =======================================================================================================================================
 :code:`.slint` Type             C++ Type                        Note
===========================  ==================================  =======================================================================================================================================
 :code:`int`                 :code:`int`
 :code:`float`               :code:`float`
 :code:`bool`                :code:`bool`
 :code:`string`              :cpp:class:`slint::SharedString`    A reference-counted string type that uses UTF-8 encoding and can be easily converted to a std::string_view or a :code:`const char *`.
 :code:`color`               :cpp:class:`slint::Color`
 :code:`brush`               :cpp:class:`slint::Brush`
 :code:`image`               :cpp:class:`slint::Image`
 :code:`physical_length`     :code:`float`                       The unit are physical pixels.
 :code:`length`              :code:`float`                       At run-time, logical lengths are automatically translated to physical pixels using the device pixel ratio.
 :code:`duration`            :code:`std::int64_t`                At run-time, durations are always represented as signed 64-bit integers with millisecond precision.
 :code:`angle`               :code:`float`                       The value in degrees.
 :code:`relative-font-size`  :code:`float`                       Relative font size factor that is multiplied with the :code:`Window.default-font-size` and can be converted to a :code:`length`.
 structure                   A :code:`class` of the same name    The order of the data member are in the lexicographic order of their name
===========================  ==================================  =======================================================================================================================================
```
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
    slint::SharedString bar;
    int foo;
};
```
