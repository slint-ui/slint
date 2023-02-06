## Two-way Bindings

Using the `<=>` syntax, one can create two way binding between properties. These properties are now linked
together and will always contain the same value.

The right hand side of the `<=>` must be a reference to a property of the same type.
When defining a new property using a two way binding to an existing property, one can
omit the type of the new property. Slint will infer this type automatically.

```slint,no-preview
export component Example  {
    in property<brush> rect-color <=> r.background;
    // it is allowed to omit the type to have it automatically inferred
    in property rect-color2 <=> r.background;
    r:= Rectangle {
        width: parent.width;
        height: parent.height;
        background: blue;
    }
}
```
