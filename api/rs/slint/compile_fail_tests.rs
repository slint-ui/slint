// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

/**
Test that the tokenizer properly rejects tokens with spaces.

This should work:

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
