## Structs

One can define named structures using the `struct` keyword,

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

`{ identifier1: type2, identifier1: type2 }` declares an anonymous structure.

Struct literals like
`{ identifier1: expression1, identifier2: expression2  }` can initialize anonymous
structures. You may use a trailing `,` after `expression2` or in the anonymous structure
definition above after `type2`.

```slint,no-preview
export component Example {
    // The trailing `;` is optional here:
    in-out property<{name: string, score: int}> player: { name: "Foo", score: 100 };
    in-out property<{a: int, }> foo: { a: 3 };
}
```
