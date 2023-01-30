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
