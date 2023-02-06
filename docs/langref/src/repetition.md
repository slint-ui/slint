# Repetition

The `for`-`in` syntax will add an element multiple times.

The syntax look like this: `for name[index] in model : id := Element { ... }`

The _model_ can be of the following type:

-   an integer, in which case the element will be repeated that amount of time
-   an array type or a model declared natively, in which case the element will be instantiated for each element in the array or model.

The _name_ will be available for lookup within the element and is going to be like a pseudo-property set to the
value of the model. The _index_ is optional and will be set to the index of this element in the model.
The _id_ is also optional.

## Examples

```slint
export component Example inherits Window {
    preferred-width: 300px;
    preferred-height: 100px;
    for my-color[index] in [ #e11, #1a2, #23d ]: Rectangle {
        height: 100px;
        width: 60px;
        x: self.width * index;
        background: my-color;
    }
}
```

```slint
export component Example inherits Window {
    preferred-width: 50px;
    preferred-height: 50px;
    in property <[{foo: string, col: color}]> model: [
        {foo: "abc", col: #f00 },
        {foo: "def", col: #00f },
    ];
    VerticalLayout {
        for data in root.model: my-repeated-text := Text {
            color: data.col;
            text: data.foo;
        }
    }
}
```
