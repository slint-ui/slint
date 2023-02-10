# Conditional Element

The `if` construct instantiates an element only if a given condition is true.
The syntax is `if condition : id := Element { ... }`

```slint
export component Example inherits Window {
    preferred-width: 50px;
    preferred-height: 50px;
    if area.pressed : foo := Rectangle { background: blue; }
    if !area.pressed : Rectangle { background: red; }
    area := TouchArea {}
}
```
