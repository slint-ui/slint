## Colors and Brushes

Color literals follow the syntax of CSS:

```slint,no-preview
export component Example inherits Window {
    background: blue;
    property<color> c1: #ffaaff;
    property<brush> b2: Colors.red;
}
```

In addition to plain colors, many elements have properties that are of type `brush` instead of `color`.
A brush is a type that can be either a color or gradient. The brush is then used to fill an element or
draw the outline.

CSS Color names are only in scope in expressions of type `color` or `brush`. Otherwise, you can access
colors from the `Colors` namespace.
