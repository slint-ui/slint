# Container Components

When creating components, it may sometimes be useful to influence where child elements
are placed when they are used. For example, imagine a component that draws a label above
whatever element the user places inside:

```slint,ignore
export component MyApp inherits Window {

    BoxWithLabel {
        Text {
            // ...
        }
    }

    // ...
}
```

Such a `BoxWithLabel` could be implemented using a layout, but by default child elements like
the `Text` element become children of the `BoxWithLabel`, when they would have to be somewhere
else, inside the layout. For this purpose, you can change the default child placement by using
the `@children` expression inside the element hierarchy of a component:

```slint
component BoxWithLabel inherits GridLayout {
    Row {
        Text { text: "label text here"; }
    }
    Row {
        @children
    }
}

export component MyApp inherits Window {
    preferred-height: 100px;
    BoxWithLabel {
        Rectangle { background: blue; }
        Rectangle { background: yellow; }
    }
}
```
