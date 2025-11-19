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
Test that invalid rust-attr result in warnings.

This should work (with a warning):
```
use slint::*;
slint!{
    export { Foo } from "tests/invalid_rust_attr.slint";
    export component Hello inherits Window { }
}
```

Test that there is indeed a warning:

```compile_fail
#![deny(deprecated)]
use slint::*;
slint!{
    export { Foo } from "tests/invalid_rust_attr.slint";
    export component Hello inherits Window { }
}
```
*/
const INVALID_RUST_ATTR: () = ();

#[cfg(doctest)]
#[doc = include_str!("README.md")]
const CHECK_README_EXAMPLES: () = ();
