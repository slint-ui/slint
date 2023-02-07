## Two-way Bindings

Create two-way bindings between properties with the `<=>` syntax. These properties will be linked
together and always contain the same value.

The right hand side of the `<=>` must be a reference to a property of the same type.
The property type is optional with two-way bindings, it will be inferred if not specified.

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
