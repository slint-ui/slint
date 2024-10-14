<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Legacy Syntax

To maintain compatibility with earlier version of Slint, the pre-1.0 syntax that declared
components and named structs with `:=` remains supported:

```slint,no-preview
export MyApp := Window {
    //...
}
```

This syntax change also effects property lookup rules and default element placement.

In components defined in the new syntax, only properties declared within the
component are in scope. By default parent elements render their children centered
and will apply all layout constraints.

In components defined using old syntax, all properties of bases of `self` and
`root` were in scope in addition to all properties defined inside the component
itself. Elements are placed at position `x: 0` and `y: 0` and their constraints
aren't applied.
