// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/**
Test that the tokenizer properly rejects tokens with spaces.

This should work:

```
mod x {
    use slint::*;
    slint!{ Hello := Rectangle { } }
}
```

But his not:

```compile_fail
mod x {
    use slint::*;
    slint!{ Hello : = Rectangle { } }
}
```

*/
#[cfg(doctest)]
const basic: u32 = 0;

/**
Test that invalid rust-attr are compilation error

This should result in a `compile_error!("Error parsing @rust-attr for struct 'Foo' declared at tests/invalid_rust_attr.slint:4:12"`
```compile_fail
use slint::*;
slint!{
    export { Foo } from "tests/invalid_rust_attr.slint";
    export component Hello inherits Window { }
}
```

But Foo is not used/generated, then we do not detect the error
(Having the test here to test that the previous test would otherwise work, but it would also be ok to detect the error and make it an actual slint compile error)
```
use slint::*;
slint!{
    import { Foo } from "tests/invalid_rust_attr.slint";
    export component Hello inherits Window { }
}
```
*/
const INVALID_RUST_ATTR: () = ();

#[cfg(doctest)]
#[doc = include_str!("README.md")]
const CHECK_README_EXAMPLES: () = ();
