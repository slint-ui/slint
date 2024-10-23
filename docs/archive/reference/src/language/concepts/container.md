<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Container Components

When creating components, it's sometimes useful to influence where child
elements are placed when used. For example, a component that draws
a label above an element inside:

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

You can implement such a `BoxWithLabel` using a layout. By default child elements like
the `Text` element become direct children of the `BoxWithLabel`, but for this example they need to become
children of the layout instead. To do this can change the default child placement by using
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
