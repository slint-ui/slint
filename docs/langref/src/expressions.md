# Expressions

Expressions are a powerful way to declare relationships and connections in your
user interface. They´re typically used to combine basic arithmetic with access
to properties of other elements. When these properties change, the expression
is automatically re-evaluated and a new value is assigned to the property the
expression is associated with:

```slint,no-preview
export component Example {
    // declare a property of type int
    in-out property<int> my-property;

    // This accesses the property
    width: root.my-property * 20px;

}
```

When`my-property` changes, the width changes automatically, too.

Arithmetic in expression with numbers works like in most programming language with the operators `*`, `+`, `-`, `/`:

```slint,no-preview
export component Example {
    in-out property <int> p: 1 * 2 + 3 * 4; // same as (1 * 2) + (3 * 4)
}
```

Concatenate strings with `+`.

The operators `&&` and `||` express logical _and_ and _or_ between
boolean values. The operators `==`, `!=`, `>`, `<`, `=>` and `<=` compare
values of the same type.

Access an element's properties by using its name, followed by a
`.` and the property name:

```slint,no-preview
export component Example {
    foo := Rectangle {
        x: 42px;
    }
    x: foo.x;
}
```

The ternary operator `... ? ... : ...` is also supported, like in C or JavaScript:

```slint
export component Example inherits Window {
    preferred-width: 100px;
    preferred-height: 100px;

    Rectangle {
        touch := TouchArea {}
        background: touch.pressed ? #111 : #eee;
        border-width: 5px;
        border-color: !touch.enabled ? #888
            : touch.pressed ? #aaa
            : #555;
    }
}
```
