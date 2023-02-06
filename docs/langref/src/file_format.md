# `.slint` Files

Each `.slint` file defines one or several components. These components contain
a tree of elements. Each declared component may be named and re-used under that
name as an element later.

Components form the basis of composition in Slint. They let you build your own
re-usable set of UI elements -- and are what drives the built-in elements
that come with Slint.

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

You can name an elements by using the `:=` syntax:

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

Names have to be valid [identifiers](identifiers.md) and are used to reference
named elements from other elements.

Some Elements have pre-defined names in addition to any you defined yourself:

`root` always refers to the outermost element of a component. `self` is the
current element, while `parent` always refers to the parent element.

These names are reserved and can't be defined by the user.
