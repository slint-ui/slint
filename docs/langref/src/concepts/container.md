# Container Components

When creating components, it's sometimes useful to influence where child
elements are placed when used. For example, imagine a component that draws
a label above whatever element the user places inside:

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

One way to implement such a `BoxWithLabel` uses a layout. By default child elements like
the `Text` element become direct children of the `BoxWithLabel`, but we need them to become
children of our layout instead. For this purpose, you can change the default child placement by using
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
