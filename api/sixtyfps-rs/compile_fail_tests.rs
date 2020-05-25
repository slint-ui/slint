/**
Test that the tokenizer properly reject tokens with spaces.

This should worlk:

```
mod x {
    use sixtyfps::*;
    sixtyfps!{ Hello := Rectangle { } }
}
```

But his not:

```compile_fail
mod x {
    use sixtyfps::*;
    sixtyfps!{ Hello : = Rectangle { } }
}
```

*/
#[cfg(doctest)]
const basic: u32 = 0;
