# `.slint` Files

Each `.slint` file defines one or several components. These components declare
a tree of elements. Each declared component may be used under its
name as an element later.

Components form the basis of composition in Slint. Use them to build your own
re-usable set of UI controls.

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

Here, both `MyButton` and `MyApp` are components. `Window` and `Rectangle` are built-in elements
used by `MyApp`. `MyApp` also re-uses the `MyButton` component.

Assign a name to an element using the `:=` syntax:

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

Names have to be valid [identifiers](identifiers.md).

Some elements are also accessible under pre-defined names:

-   `root` refers to the outermost element of a component.
-   `self` refers to the current element.
-   `parent` refers to the parent element of the current element.

These names are reserved and can't be re-defined by the user.
