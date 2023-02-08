## Relative Lengths

Sometimes it's convenient to express the relationships of length properties in terms of relative percentages.
For example the following inner blue rectangle has half the size of the outer green window:

```slint
export component Example inherits Window {
    preferred-width: 100px;
    preferred-height: 100px;

    background: green;
    Rectangle {
        background: blue;
        width: parent.width * 50%;
        height: parent.height * 50%;
    }
}
```

This pattern of expressing the `width` or `height` in percent of the parent's property with the same name is
common. For convenience, a short-hand syntax exists for this scenario:

-   The property is `width` or `height`
-   A binding expression evaluates to a percentage.

If these conditions are met, then it's not necessary to specify the parent property, instead you can simply
use the percentage. The earlier example then looks like this:

```slint
export component Example inherits Window {
    preferred-width: 100px;
    preferred-height: 100px;

    background: green;
    Rectangle {
        background: blue;
        width: 50%;
        height: 50%;
    }
}
```
