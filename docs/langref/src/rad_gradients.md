### Radial Gradients

Linear gradiants are like real gradiant but the colors is interpolated in a circle instead of
along a line. To describe a readial gradiant, use the `@radial-gradient` macro with the following signature:

**`@radial-gradient(circle, color percentage, color percentage, ...)`**

The first parameter to the macro is always `circle` because only circular radients are supported.
The syntax is otherwise based on the CSS `radial-gradient` function.

Example:

```slint
export component Example inherits Window {
    preferred-width: 100px;
    preferred-height: 100px;
    Rectangle {
        background: @radial-gradient(circle, #f00 0%, #0f0 50%, #00f 100%);
    }
}
```
