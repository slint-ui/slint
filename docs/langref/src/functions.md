# Functions

Declare helper functions with the `function` keyword.
Functions are private by default, but can be made public with the `public` annotation.

```slint,no-preview
export component Example {
    in property <int> min;
    in property <int> max;
    public function inbound(x: int) -> int {
        return Math.min(root.max, Math.max(root.min, x));
    }
}
```
