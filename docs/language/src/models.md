## Arrays and Models

Arrays are declared by wrapping `[` and `]` square brackets around the type of the array elements.

Array literals as well as properties holding arrays act as models in`for` expressions.

```slint,no-preview
export component Example {
    in-out property<[int]> list-of-int: [1,2,3];
    in-out property<[{a: int, b: string}]> list-of-structs: [{ a: 1, b: "hello" }, {a: 2, b: "world"}];
}
```

Arrays define the following operations:

-   **`array.length`**: One can query the length of an array and model using the builtin `.length` property.
-   **`array[index]`**: The index operator retrieves individual elements of an array.

Out of bound access into an array will return default-constructed values.

```slint,no-preview
export component Example {
    in-out property<[int]> list-of-int: [1,2,3];

    out property <int> list-len: list-of-int.length;
    out property <int> first-int: list-of-int[0];
}

```
