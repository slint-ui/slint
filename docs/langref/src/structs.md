## Structs

Anonymous structs type can be declared with curly braces: `{ identifier1: type2, identifier1: type2, }`
The trailing semicolon is optional.
They can be initialized with a struct literal: `{ identifier1: expression1, identifier2: expression2  }`

```slint,no-preview
export component Example {
    in-out property<{name: string, score: int}> player: { name: "Foo", score: 100 };
    in-out property<{a: int, }> foo: { a: 3 };
}
```
