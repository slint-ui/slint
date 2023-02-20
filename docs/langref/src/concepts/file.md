# The `.slint` File

User interfaces are written in the Slint language and saved in files with the `.slint` extension.

Each `.slint` file defines one or several components. These components declare
a tree of elements. Components form the basis of composition in Slint. Use them
to build your own re-usable set of UI controls. You can use each declared
component under its name as an element in another component.

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

Elements have properties, which you can assign values to. Here we assign a string
constant "hello" to the first `MyButton`'s `text` property. You
can also assign entire expressions. Slint will re-evaluate the expressions when any
of the properties they depend on change, which makes the user-interface reactive.

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

These names are reserved any you can't re-define them.
