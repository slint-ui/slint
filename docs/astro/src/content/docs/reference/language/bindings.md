---
title: Bindings
description: Assigning expressions to properties.
---

## Form

A binding assigns an expression to a property of the element in whose body it appears.
It has the form of a property name, followed by `:`, an expression, and `;`. {#sls.binding.form}

```slint
export component Example inherits Window {
    Rectangle {
        background: #2a6e3f;
    }
}
```

The name shall refer to a property of the enclosing element. {#sls.binding.target-must-exist}

## Expressions

This revision of the specification covers a single expression form: the color literal. {#sls.expr.forms}

### Color Literals

A color literal consists of `#` followed by 3, 4, 6, or 8 hexadecimal digits:
`#rgb`, `#rgba`, `#rrggbb`, or `#rrggbbaa`.
The digits are case-insensitive.
Any other number of digits is an error. {#sls.expr.color.forms}

The digits specify the red, green, blue, and alpha channels, in this order.
When the alpha channel is absent, the color is fully opaque. {#sls.expr.color.channels}

In the 3- and 4-digit forms, each digit specifies a channel with the digit duplicated:
`#18f` is the same color as `#1188ff`. {#sls.expr.color.short-forms}

A color literal evaluates to a value of type `color`. {#sls.expr.color.type}
