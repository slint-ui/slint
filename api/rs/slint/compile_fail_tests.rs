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

#[cfg(doctest)]
#[doc = include_str!("README.md")]
const CHECK_README_EXAMPLES: () = ();
