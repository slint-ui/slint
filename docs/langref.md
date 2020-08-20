# The `.60` language reference

This page is work in progress as the language is not yet set in stones.
`TODO` indicates things that are not yet implemented.

## `.60` files

The basic idea is that the `.60` files contains one or several components.
These components contain a tree of elements. Each declared component can be
given a name and re-used under that name as an an element later.

By default, the SixtyFPS comes with some [builtin elements](#builtin-elements).

Below is an example of components and elements:

```60

MyButton := Rectangle {
    // ...
}

export MyApp := Window {
    MyButton {
        text: "hello";
    }
    MyButton {
        text: "world";
    }
    Rectangle {
        color: green;        
    }
}

```

Here, both `MyButton` and `MyApp` are components. `Window` and `Rectangle` are built-in elements
used by `MyApp`. `MyApp` also re-uses the `MyButton` component.

You can assign a name to the elements using the `:=`  syntax in front an element:

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

The outermost element of a component is always accessible under the name `root`.
The current element can be referred as `self`.
The parent element can be referred as `parent`.
These names are reserved and cannot be used as element names.

## Comments

C-style comments are supported:
 - line comments: `//` means everything to the end of the line is commented.
 - block comments: `/* .. */`  (TODO, make possible to nest them)


## Properties

The elements can have properties. Built-in elements come with common properties such
as color or dimensional properties. You can assign values or entire [expressions](#expressions) to them:

```60
Example := Rectangle {
    // Simple expression: ends with a semi colon
    width: 42px;
    // or a code block
    height: { 42px }
}
```

You can also declare your own properties. The properties declared at the top level of a
component are public and can be accessed by the component using it as an element, or using the
language bindings:

```60
Example := Rectangle {
    // declare a property of type int with the name `my_property`
    property<int> my_property;

    // declare a property with a default value
    property<int> my_second_property: 42;
}
```

## Types

 - `int`
 - `float`:
   `int` and `float` are the types for the numbers, they correspond to the equivalent in the target language
    A number can end with '%', so for example `30%` is the same as `0.30`
 - `string`: Represent a utf8 encoded string. Strings are reference counted.
 - `color`: color literal follow more or less the CSS specs
 - `length`: the type for the x, y, width and height coordinate. This is an amount of physical pixels. To convert from
an integer to a length unit, one can simply multiply by `1px`.  Or to convert from a length to a float, one can divide
by `1px`.
 - `logical_length`:  correspond to literal like `1lx`, `1pt`, `1in`, `1mm`, or `1cm`.
It can be converted to and from length provided the binding is run in a context where there
is an access to the pixel ratio.
 - `duration`: is a type for the duration of animation, it is represented by the amount of milisecond. But in the language
they correspond to the number like `1ms` or `1s`
 - `easing`: follow more or less the CSS spec

Please see the language specific API references how these types are mapped to the APIs of the different programming languages.

## Signal

Components may declare signals, that allow it to communicate change of state to the outside. Signals are emitted by "calling" them
and you can re-act to signal emissions by declaring a handler using the `=>` arrow syntax. The built-in `TouchArea`
element comes with a `clicked` signal, that's emitted when the user touches the rectangular area covered by the element, or clicks into
it with the mouse. In the example below, the emission of that signal is forwarded to another custom signal (`hello`) by declaring a
handler and emitting our custom signal:

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

TODO: add parameter to the signal


## Expressions

Expressions are a powerful way to declare relationships and connections in your user interface. They
are typically used to combine basic arithmetic with access to properties of other elements. When
these properties change, the expression is automatically re-evaluated and a new value is assigned
to the property the expression is associated with:

```60
Example := Rectangle {
    // declare a property of type int
    property<int> my_property;

    // This access the property
    width: root.my_property * 20px;

}
```

If someone changes `my_property`, the width will be updated automatically.

Arithmetic in expression works like in most programming language with the operators `*`, `+`, `-`, `/`:

```60
Example := Rectangle {
    x: 1 * 2 + 3 * 4; // same as (1 * 2) + (3 * 4)
}
```

You can access properties by addressing the associated element, followed by a `.` and the property name:

```60
Example := Rectangle {
    foo := Rectangle {
        x: 42;
    }
    x: foo.x;
}
```

Strings can be used with surrounding quote: `"foo"`.
(TODO: escaping, support using stuff like `` `hello {foo}` ``)


```60
Example := Text {
    text: "hello";
}
```

Color literals follow the syntax of CSS:

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

## Statements

Inside signal handlers, more complicated statements are allowed:

Assignment:

```60
clicked => { some_property = 42; }
```

Self-assignement with `+=` `-=` `*=` `/=`

```60
clicked => { some_property += 42; }
```

Calling a signal

```60
clicked => { root.some_signal(); }
```

Conditional expression

```60
clicked => {
    if (condition) {
        foo = 42;
    } else {
        bar = 28;
    }
}
```

Empty expression

```60
clicked => { }
// or
clicked => { ; }
```


## Repetition

The `for` syntax


```60
Example := Rectangle {
    for person[index] in model: Button {
    }
}
```

## Animations

Simple animation that animates a property can be declared with `animate` like so:

```60
Example := Rectangle {
    property<bool> pressed;
    color: pressed ? blue : red;
    animate color {
        duration: 100ms;
    }
}
```

This will aniate the color property for 100ms when it changes.

Animation can be configured with the following parameter:
 * `duration`: the amount of time it takes for the animation to complete
 * `loop_count`: FIXME
 * `easing`: can be `linear`, `ease`, `ease_in`, `ease_out`, `ease_in_out`, `cubic_bezier(a, b, c, d)` as in CSS

It is also possible to animate sevaral properties with the same animation:

```60
animate x, y { duration: 100ms; }
```
is the same as
```60
animate x { duration: 100ms; }
animate y { duration: 100ms; }
```

## States

The `states` statement alow to declare states like so:

```60
Example := Rectangle {
    text := Text { text: "hello" }
    property<bool> pressed;
    property<bool> enabled;

    states [
        disabled when !enabled : {
            color: gray; // same as root.color: gray;
            text.color: white;
        }
        down when pressed : {
            color: blue;
        }
    ]
}
```

In that example, when the `enabled` property is set to false, the `disabled` state will be entered
This will change the color of the Rectangle and of the Text.

### Transitions (TODO)

Complex animation can be declared on state transitions:

```60
Example := Rectangle {
    text := Text { text: "hello" }
    property<bool> pressed;
    property<bool> enabled;

    states [
        disabled when !enabled : {
            color: gray; // same as root.color: gray;
            text.color: white;
        }
        down when pressed : {
            color: blue;
        }
    ]

    transitions [
        to down {
            animate color { duration: 300ms }
        }
        out disabled {
            animate * { duration: 800ms }
        }
    ]
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

In the above example, `Button` is usable from other .60 files, but `ButtonHelper` isn't.

It's also possible to change the name just for the purpose of exporting, without affecting its internal use:

```60
Button := Rectangle {
    // ...
}

export { Button as ColorButton }
```

In the above example, ```Button``` is not accessible from the outside, but instead it is available under the name ```ColorButton```.

For convenience, a third way of exporting a component is to declare it exported right away:

```60
export Button := Rectangle {
    // ...
}
```

Similarly, components exported from other files can be accessed by importing them:

```60
import { Button } from "./button.60";

App := Rectangle {
    // ...
    Button {
        // ...
    }
}
```

In the event that two files export a type under the same then, then you have the option
of assigning a different name at import type:

```60
import { Button } from "./button.60";
import { Button as CoolButton } from "../other_theme/button.60";

App := Rectangle {
    // ...
    CoolButton {} // from cool_button.60
    Button {} // from button.60
}
```

## Builtin elements

### Rendered Items

#### Rectangle

#### Image

#### Path

### TouchArea

### Layouts

#### Window (TODO)

#### GridLayout

#### PathLayout

#### Flickable

...



