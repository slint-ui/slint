## Arrays and Models

Arrays are created by wrapping `[` and `]` around the type of the array elements.

In the runtime, arrays are used as models in the `for` expression.

```slint,no-preview
export component Example {
    in-out property<[int]> list-of-int: [1,2,3];
    in-out property<[{a: int, b: string}]> list-of-structs: [{ a: 1, b: "hello" }, {a: 2, b: "world"}];
}
```

Arrays define some operations by default:

-   **`array.length`**: One can query the length of an array and model using the builtin `.length` property.
-   **`array[index]`**: The index operator retrieves individual elements of an array.

Out of bound access into an array will return default-constructed values or cause
a compile error (if detectable at compile time).

```slint,no-preview
export component Example {
    in-out property<[int]> list-of-int: [1,2,3];

    out property list-len: list-of-int.length;
    out property first-int: list-of-int[0];
}

```
