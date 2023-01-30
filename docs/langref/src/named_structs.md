## Custom named structures

It is possible to define a named struct using the `struct` keyword,

```slint,no-preview
export struct Player  {
    name: string,
    score: int,
}

export component Example {
    in-out property<Player> player: { name: "Foo", score: 100 };
}
```
