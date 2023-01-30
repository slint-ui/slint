## Two-way Bindings

Using the `<=>` syntax, one can create two ways binding between properties. These properties are now linked
together.
The right hand side of the `<=>` must be a reference to a property of the same type.
The type can be omitted in a property declaration to have the type automatically inferred.

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
