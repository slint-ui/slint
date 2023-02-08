## Conversions

Slint supports conversions between different types. Explicit
conversions are required to make the UI description more robust, but implicit
conversions are allowed between some types for convenience.

The following conversions are possible:

-   `int` can be converted implicitly to `float` and vice-versa
-   `int` and `float` can be converted implicitly to `string`
-   `physical-length` and `length` can be converted implicitly to each other only in
    context where the pixel ratio is known.
-   the units type (`length`, `physical-length`, `duration`, ...) cannot be converted to numbers (`float` or `int`)
    but they can be divided by themselves to result in a number. Similarly, a number can be multiplied by one of
    these unit. The idea is that one would multiply by `1px` or divide by `1px` to do such conversions
-   The literal `0` can be converted to any of these types that have associated unit.
-   Struct types convert with another struct type if they have the same property names and their types can be converted.
    The source struct can have either missing properties, or extra properties. But not both.
-   Arrays generally do not convert between each other. Array literals can be converted if the element types are convertible.
-   String can be converted to float by using the `to-float` function. That function returns 0 if the string is not
    a valid number. You can check with `is-float()` if the string contains a valid number

```slint,no-preview
export component Example {
    // ok: int converts to string
    property<{a: string, b: int}> prop1: {a: 12, b: 12 };
    // ok even if a is missing, it will just have the default value
    property<{a: string, b: int}> prop2: { b: 12 };
    // ok even if c is too many, it will be discarded
    property<{a: string, b: int}> prop3: { a: "x", b: 12, c: 42 };
    // ERROR: b is missing and c is extra, this does not compile, because it could be a typo.
    // property<{a: string, b: int}> prop4: { a: "x", c: 42 };

    property<string> xxx: "42.1";
    property<float> xxx1: xxx.to-float(); // 42.1
    property<bool> xxx2: xxx.is-float(); // true
}
```
