# The `.60` language reference

This page is work in progress as the language is not yet set in stones.

## Comments

C-style comments are supported:
 - line comments: `//` means everything to the end of the line is commented.
 - block comments: `/* .. */`  (FIXME, make possible to embedd them)

## `.60` files

The basic idea is that the .60 files contains one or several "component" (FIXME: name to be
adjusted).
These "components" are consisting of a bunch of elements that form a tree of elements.
Each declared component can be re-used as an element later. There are also a bunch
of builtin elements.

```60

MyButton := Rectangle {
    // ...
}

MyApp := Window {
    MyButton {
        text: "hello";
    }
    MyButton {
        text: "world";
    }
}

```

Here, both `MyButton` and `MyApp` are components.  MyApp is the main component because it is the last one
(FIXME, maybe there should be a keyword or something)

One can give name to the elements using the `:=`  syntax within a component

```60
//...
MyApp := Window {
    hello := MyButton {
        text: "hello";
    }
    world := MyButton {
        text: "world";
    }
}
```

The root element of a component is always called `root`

## Properties

The elements can have properties

```60
Example := Rectangle {
    // Simple expression: ends with a semi colon
    width: 42px;
    // or a code block
    height: { 42px }
}
```

You can declare properties. The properties declared at the top level of the main component
are public.
property are declared like so:

```60
Example := Rectangle {
    // declare a property of type int
    property<int32> my_property;

    // declare a property with a default value
    property<int32> my_second_property: 42;
}
```

The value of properties are an expression (see later).
You can access properties in these expression, and the bindings are automatically
re-evaluated if the property changes.

```60
Example := Rectangle {
    // declare a property of type int
    property<int32> my_property;

    // This access the property
    width: root.my_property * 20px;

}
```

If one change the `my_property`, the width will be updated automatically.


## Types

 - `int32`
 - `float32`
 - `string`
 - `color`
 - `length`
 - `logical_length`
 - `duration`
 - FIXME: more

`int32` and `float32` are the types for the numbers, they correspond to the equivalent in the target language
A number can end with '%', so for example `30%` is the same as `0.30`

`string` are implicitly shared.

`length` is the type for the x, y, width and height coordinate. This is an amount of physical pixels. To convert from
an integer to a length unit, one can simply multiply by `1px`.  Or to convert from a length to a float32, one can divide
by `1px`.
`logical_length` correspond to literal like `1lx`, `1pt`, `1in`, `1mm`, or `1cm`.
It can be converted to and from length provided the binding is run in a context where there
is an access to the pixel ratio.

`duration` is a type for the duration of animation, it is represented by the amount of milisecond. But in the language
they correspond to the number like `1ms` or `1s`


## Expressions

Basic arithmetic expression do what they do in most languages

```60
Example := Rectangle {
    x: 1 * 2 + 3 * 4; // same as (1 * 2) + (3 * 4)
}
```

Access properties with `.`

```60
Example := Rectangle {
    x: foo.x;
    foo := Rectangle {
        x: 42;
    }
}
```

Strings are with quote.
(FIXME what is the escaping, should we support using stuff like "hello {name}"?)

```60
Example := Text {
    text: "hello";
}
```

Color literal use the CSS syntax:

```60
Example := Rectangle {
    color: blue;
    property<color> c1: #ffaaff;
}
```

Array / Object

```
TODO
```


## Signal

FIXME: rename event?

```60
Example := Rectangle {
    // declares a signal
    signal hello;

    area := TouchArea {
        // sets a handler with `=>`
        clicked => {
            // emit the signal
            root.hello()
        }
    }
}
```

## Repetition

The `for` syntax


```60
Example := Rectangle {
    for person[index] in model: Button {
    }
}
```


## Modules

Components declared in a .60 file can be shared with components in other .60 files, by means of exporting and importing them.
By default, everything declared in a .60 file is private, but it can be made accessible from the outside using the export
keyword:

```60
ButtonHelper := Rectangle { 
    // ...
}

Button := Rectangle {
    // ...
    ButtonHelper {
        // ...
    }
}

export { Button }
```

In the above example, ```Button``` is usable from other .60 files, but ```ButtonHelper``` isn't.

It's also possible to change the name just for the purpose of exporting, without affecting its internal use:

```60
Button := Rectangle {
    // ...
}

export { Button as ColorButton }
```

In the above example, ```Button``` is not accessible from the outside, but instead it is available under the name ```ColorButton```.