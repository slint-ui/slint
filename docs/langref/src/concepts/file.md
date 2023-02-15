# The `.slint` File

`.slint` is the file extension typically used for files containing a user
interface description written in the Slint language.

Each `.slint` file defines one or several components. These components declare
a tree of elements. Components form the basis of composition in Slint. Use them
to build your own re-usable set of UI controls. You can use each declared
component under its name as an element n another component later.

Below is an example of components and elements:

```slint

component MyButton inherits Text {
    color: black;
    // ...
}

export component MyApp inherits Window {
    preferred-width: 200px;
    preferred-height: 100px;
    Rectangle {
        width: 200px;
        height: 100px;
        background: green;
    }
    MyButton {
        x:0;y:0;
        text: "hello";
    }
    MyButton {
        y:0;
        x: 50px;
        text: "world";
    }
}

```

Both `MyButton` and `MyApp` are components. `Window` and `Rectangle` are built-in elements
used by `MyApp`. `MyApp` also re-uses the `MyButton` component as two separate elements.

Elements have properties, which you can assign values to. Here the first `MyButton`
element has a property `text` that we assign a string constant "hello" to. You
can assign expressions using other properties as well. Slint will re-evaluate
properties when any of the properties they depend on change, which makes the
user-interface reactive.

You can name elements using the `:=` syntax:

```slint
component MyButton inherits Text {
    // ...
}

export component MyApp inherits Window {
    preferred-width: 200px;
    preferred-height: 100px;

    hello := MyButton {
        x:0;y:0;
        text: "hello";
    }
    world := MyButton {
        y:0;
        text: "world";
        x: 50px;
    }
}
```

Names have to be valid [identifiers](../identifiers.md).

Some elements are also accessible under pre-defined names:

-   `root` refers to the outermost element of a component.
-   `self` refers to the current element.
-   `parent` refers to the parent element of the current element.

Users can not redefine these names to refer to something else.
