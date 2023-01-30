## Arrays / Model

The type array is using square brackets for example `[int]` is an array of `int`. In the runtime, they are
basically used as models for the `for` expression.

```slint,no-preview
export component Example {
    in-out property<[int]> list-of-int: [1,2,3];
    in-out property<[{a: int, b: string}]> list-of-structs: [{ a: 1, b: "hello" }, {a: 2, b: "world"}];
}
```

-   **`length`**: One can query the length of an array and model using the builtin `.length` property.
-   **`array[index]`**: Individual elements of an array can be retrieved using the `array[index]` syntax.
