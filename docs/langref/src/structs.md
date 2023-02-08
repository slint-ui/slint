## Structs

Define named structures using the `struct` keyword:

```slint,no-preview
export struct Player  {
    name: string,
    score: int,
}

export component Example {
    in-out property<Player> player: { name: "Foo", score: 100 };
}
```

### Anonymous Structures

Declare anonymous structures using `{ identifier1: type2, identifier1: type2 }`
syntax, and initialize them using
`{ identifier1: expression1, identifier2: expression2  }`.

You may have a trailing `,` after the last expression or type.

```slint,no-preview
export component Example {
    in-out property<{name: string, score: int}> player: { name: "Foo", score: 100 };
    in-out property<{a: int, }> foo: { a: 3 };
}
```
