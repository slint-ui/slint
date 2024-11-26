---
<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
title: Statements
description: Statements
---

Callback handlers may contain complex statements:

Assignment:

```slint no-test
clicked => { some-property = 42; }
```

Self-assignment with `+=` `-=` `*=` `/=`

```slint no-test
clicked => { some-property += 42; }
```

Calling a callback

```slint no-test
clicked => { root.some-callback(); }
```

Conditional statements

```slint no-test
clicked => {
    if (condition) {
        foo = 42;
    } else if (other-condition) {
        bar = 28;
    } else {
        foo = 4;
    }
}
```

Empty expression

```slint no-test
clicked => { }
// or
clicked => { ; }
```
