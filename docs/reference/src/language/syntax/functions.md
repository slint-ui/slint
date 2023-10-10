<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Functions

Functions in Slint are declared using the `function` keyword.

Each function can have parameters which are declared within parentheses, following the format `name : type`.
These parameters can be referenced by their names within the function body.

Functions can also return a value. The return type is specified after `->` in the function signature.
The `return` keyword is used within the function body to return an expression of the declared type.
If a function does not explicitly return a value, the value of the last statement is returned by default.

Functions can be annotated with the `pure` keyword.
This indicates that the function does not cause any side effects.
More details can be found in the [Purity](../concepts/purity.md) chapter.

By default, functions are private and cannot be accessed from external components.
However, their accessibility can be modified using the `public` or `protected` keywords.

- A function annotated with `public` can be accessed by any component.
- A function annotated with `protected` can only be accessed by components that directly inherit from it.

## Example

```slint,no-preview
export component Example {
    in-out property <int> min;
    in-out property <int> max;
    protected function set-bounds(min: int, max: int) {
        root.min = min;
        root.max = max
    }
    public pure function inbound(x: int) -> int {
        return Math.min(root.max, Math.max(root.min, x));
    }
}
```

In the example above, `set-bounds` is a protected function that updates the `min` and `max` properties of the root component.
The `inbound` function is a public, pure function that takes an integer `x` and returns the value constrained within the `min` and `max` bounds.
