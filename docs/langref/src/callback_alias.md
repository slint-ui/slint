## Callback aliases

It is possible to declare callback aliases in a similar way to two-way bindings:

```slint,no-preview
export component Example inherits Rectangle {
    callback clicked <=> area.clicked;
    area := TouchArea {}
}
```
