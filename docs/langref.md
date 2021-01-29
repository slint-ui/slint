# The `.60` language reference

This page is work in progress as the language is not yet set in stones.
`TODO` indicates things that are not yet implemented.

## `.60` files

The basic idea is that the `.60` files contains one or several components.
These components contain a tree of elements. Each declared component can be
given a name and re-used under that name as an an element later.

By default, the SixtyFPS comes with some [builtin elements](builtin_elements.md) and [widgets](widgets.md).

Below is an example of components and elements:

```60

MyButton := Text {
    color: black;
    // ...
}

export MyApp := Window {
    width: 200px;
    height: 100px;
    Rectangle {
        width: 200px;
        height: 100px;
        color: green;
    }
    MyButton {
        text: "hello";
    }
    MyButton {
        x: 50px;
        text: "world";
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

### Container Components

When creating components, it may sometimes be useful to influence where child elements
are placed when they are used. For example, imagine a component that draws label above
whatever element the user places inside:

```60
MyApp := Window {

    BoxWithLabel {
        Text {
            // ...
        }
    }

    // ...
}
```

Such a `BoxWithLabel` could be implemented using a layout, but by default child elements like
the `Text` element become children of the `BoxWithLabel`, when they would have to be somewhere
else, inside the layout. For this purpose, you can change the default child placement by using
the `@children` expression inside the element hierarchy of a component:

```60
BoxWithLabel := GridLayout {
    Row {
        Text { text: "label text here"; }
    }
    Row {
        @children
    }
}

MyApp := Window {
    BoxWithLabel {
        Rectangle { color: blue; }
        Rectangle { color: yellow; }
    }
}
```

## Comments

C-style comments are supported:
 - line comments: `//` means everything to the end of the line is commented.
 - block comments: `/* .. */`.  Note that the blocks comments can be nested, so `/* this is a /* single */ comment */`

## Identifiers

Identifiers can be composed of letter (`a-zA-Z`), of numbers (`0-9`), or of the underscore (`_`) or the dash (`-`).
They cannot start with a number or a dash (but they can start with underscore)
The dashes are normalized to underscore. Which means that these two identifiers are the same: `foo-bar` and `foo_bar`.

## Properties

The elements can have properties. Built-in elements come with common properties such
as color or dimensional properties. You can assign values or entire [expressions](#expressions) to them:

```60
Example := Window {
    // Simple expression: ends with a semi colon
    width: 42px;
    // or a code block (no semicolon needed)
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

### Bindings

The expression on the right of a binding is automatically re-evaluated when the expression changes.

In the following example, the text of the button is automaticallty changed when the button is pressed, because
changing the `counter`  property automatically changes the text.

```60
import { Button } from "sixtyfps_widgets.60";
Example := Button {
    property <int> counter: 3;
    clicked => { counter += 3 }
    text: counter * 2;
}
```

### Two-way Bindings

Using the `<=>` syntax, one can create two ways binding between properties. These properties are now linked
together.
The right hand side of the `<=>` must be a reference to a property of the same type.


```60
Example := Window {
    property<color> rect_color <=> r.color;
    r:= Rectangle {
        width: parent.width;
        height: parent.height;
        color: blue;
    }
}
```

## Types

All properties in elements have a type. The following types are supported:

| Type | Description |
| --- | --- |
| `int` | Signed integral number. |
| `float` | Signed, 32-bit floating point number. Numbers with a `%` suffix are automatically divided by 100, so for example `30%` is the same as `0.30`. |
| `string` | UTF-8 encoded, reference counted string. |
| `color` | RGB color with an alpha channel, with 8 bit precision for each channel. CSS color names as well as the hexadecimal color encodings are supported, such as `#RRGGBBAA` or `#RGB`. |
| `length` | The type used for `x`, `y`, `width` and `height` coordinates. This is an amount of physical pixels. To convert from an integer to a length unit, one can simply multiply by `1px`.  Or to convert from a length to a float, one can divide by `1phx`. |
| `logical_length` | Corresponds to a literal like `1px`, `1pt`, `1in`, `1mm`, or `1cm`. It can be converted to and from length provided the binding is run in a context where there is an access to the device pixel ratio. |
| `duration` | Type for the duration of animations. A suffix like `ms` (milisecond) or `s` (second) is used to indicate the precision. |
| `easing` | Property animation allow specifying an easing curve. Valid values are `linear` (values are interpolated linearly) and the [four common cubiz-bezier functions known from CSS](https://developer.mozilla.org/en-US/docs/Web/CSS/easing-function#Keywords_for_common_cubic-bezier_easing_functions):  `ease`, `ease_in`, `ease_in_out`, `ease_out`. |
| `percent` | Signed, 32-bit floating point number that is interpreted as percentage. Literal number assigned to properties of this type must have a `%` suffix. |

Please see the language specific API references how these types are mapped to the APIs of the different programming languages.

### Objects

It is basically an anonymous structures, it can be declared with curly braces: `{ identifier1: type2, identifier1: type2, }`
The trailing semicolon is optional.


```60
Example := Window {
    property<{name: string, score: int}> player: { name: "Foo", score: 100 };
    property<{a: int, }> foo: { a: 3 };
}
```

### Custom named structures

It is possible to define a struct using the struct keyword, and defined as an object type

```60
export struct Player := {
    name: string,
    score: int,
}

Example := Window {
    property<Player> player: { name: "Foo", score: 100 };
}
```

### Arrays / Model

The type array is using square brackets for example  `[int]` is an array of `int`. In the runtime, they are
basically used as models for the `for` expression.

```60
Example := Window {
    property<[int]> list_of_int: [1,2,3];
    property<[{a: int, b: string}]> list_of_object: [{ a: 1, b: "hello" }, {a: 2, b: "world"}];
}
```

### Conversions

 * `int` can be converted implicitly to `float` and vice-versa
 * `int` and `float` can be converted implicitly to `string`
 * `logical_length` and `length` can be converted implictly to eachother only in
   context where the pixel ratio is known.
 * the units type (`length`, `logical_length`, `duration`, ...) cannot be converted to numbers (`float` or `int`)
   but they can be devided with themself to result in a number. Similarily, a number can be multiplied by one of
   these unit. The idea is that one would multiply by `1px` or divide by `1px` to do such conversions
 * Object types convert with another object type if they have the same property names and their types can be converted.
    The source object can have either missing properties, or extra properties. But not both.
 * Array generaly do not convert between eachother. But array literal can be converted if the type does convert.
 * String can be converted to float by using the `to_float` function. That function returns 0 if the string is not
   a valid number. you can check with `is_float` if the string contains a valid number

```60
Example := Window {
    // ok: int converts to string
    property<{a: string, b: int}> prop1: {a: 12, b: 12 };
    // ok even if a is missing, it will just have the default value
    property<{a: string, b: int}> prop2: { b: 12 };
    // ok even if c is too many, it will be discarded
    property<{a: string, b: int}> prop2: { a: "x", b: 12, c: 42 };
    // ERROR: b is missing and c is extra, this does not compile, because it could be a typo.
    // property<{a: string, b: int}> prop2: { a: "x", c: 42 };

    property<string> xxx: "42.1";
    property<float> xxx1: xxx.to_float(); // 42.1
    property<bool> xxx2: xxx.is_float(); // true
}
```

### Relative Lengths

Sometimes it is convenient to express the relationships of length properties in terms of relative percentages.
For example the following inner blue rectangle has half the size of the outer green one:

```60
Example := Rectangle {
    color: green;
    Rectangle {
        color: blue;
        width: parent.width * 50%;
        height: parent.height * 50%;
    }
}
```

This pattern of expressing the `width` or `height` in percent of the parent's property with the same name is
common. For convenience, a short-hand syntax exists for this scenario:

  - The property is `width` or `height`
  - A binding expression evaluates to a percentage.

If these conditions are met, then it is not necessary to specify the parent property, instead you can simply
use the percentage. The earlier example then looks like this:

```60
Example := Rectangle {
    color: green;
    Rectangle {
        color: blue;
        width: 50%;
        height: 50%;
    }
}
```

## Callback

Components may declare callbacks, that allow it to communicate changes of state to the outside. Callbacks are emitted by "calling" them
and you can react to callback emissions by declaring a handler using the `=>` arrow syntax. The built-in `TouchArea`
element comes with a `clicked` callback, that's emitted when the user touches the rectangular area covered by the element, or clicks into
it with the mouse. In the example below, the emission of that callback is forwarded to another custom callback (`hello`) by declaring a
handler and emitting our custom callback:

```60
Example := Rectangle {
    // declare a callback
    callback hello;

    area := TouchArea {
        // sets a handler with `=>`
        clicked => {
            // emit the callback
            root.hello()
        }
    }
}
```

It is also possible to add parameters to the callback.

```60
Example := Rectangle {
    // declares a callback
    callback hello(int, string);
    hello(aa, bb) => { /* ... */ }
}
```


And return value.

```60
Example := Rectangle {
    // declares a callback with a return value
    callback hello(int, int) -> int;
    hello(aa, bb) => { aa + bb }
}
```


## Expressions

Expressions are a powerful way to declare relationships and connections in your user interface. They
are typically used to combine basic arithmetic with access to properties of other elements. When
these properties change, the expression is automatically re-evaluated and a new value is assigned
to the property the expression is associated with:

```60
Example := Rectangle {
    // declare a property of type int
    property<int> my_property;

    // This accesses the property
    width: root.my_property * 20px;

}
```

If something changes `my_property`, the width will be updated automatically.

Arithmetic in expression works like in most programming language with the operators `*`, `+`, `-`, `/`:

```60
Example := Rectangle {
    property <int> p: 1 * 2 + 3 * 4; // same as (1 * 2) + (3 * 4)
}
```

You can access properties by addressing the associated element, followed by a `.` and the property name:

```60
Example := Rectangle {
    foo := Rectangle {
        x: 42px;
    }
    x: foo.x;
}
```

### Strings

Strings can be used with surrounding quotes: `"foo"`.

Some character can be escaped with slashes (`\`)

| Escape | Result |
| --- | --- |
| `\"` | `"` |
| `\\` | `\` |
| `\n` | new line |
| `\u{xxx}` | where `xxx` is an hexadecimal number, this expand to the unicode character represented by this number |
| `\{expression}` | the expression is evaluated and inserted here |

Anything else after a `\` is an error.


(TODO: translations: `tr!"Hello"`)


```60
Example := Text {
    text: "hello";
}
```

### Colors

Color literals follow the syntax of CSS:

```60
Example := Rectangle {
    color: blue;
    property<color> c1: #ffaaff;
}
```

(TODO: currently color name are only limited to a handfull and only supported in color property)

### Arrays/Objects

Arrays are currently only supported in `for` expressions. `[1, 2, 3]` is an array of integers.
All the types in the array have to be of the same type.
It is useful to have arrays of objects. An Object is between curly braces: `{ a: 12, b: "hello"}`.


## Statements

Inside callback handlers, more complicated statements are allowed:

Assignment:

```
clicked => { some_property = 42; }
```

Self-assignement with `+=` `-=` `*=` `/=`

```
clicked => { some_property += 42; }
```

Calling a callback

```
clicked => { root.some_callback(); }
```

Conditional expression

```
clicked => {
    if (condition) {
        foo = 42;
    } else {
        bar = 28;
    }
}
```

Empty expression

```
clicked => { }
// or
clicked => { ; }
```


## Repetition

The `for` syntax


```60
Example := Window {
    height: 100px;
    width: 300px;
    for my_color[index] in [ #e11, #1a2, #23d ]: Rectangle {
        height: 100px;
        width: 60px;
        x: width * index;
        color: my_color;
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

This will animate the color property for 100ms when it changes.

Animation can be configured with the following parameter:
 * `duration`: the amount of time it takes for the animation to complete
 * `loop_count`: FIXME
 * `easing`: can be `linear`, `ease`, `ease_in`, `ease_out`, `ease_in_out`, `cubic_bezier(a, b, c, d)` as in CSS

It is also possible to animate several properties with the same animation:

```
animate x, y { duration: 100ms; }
```
is the same as
```
animate x { duration: 100ms; }
animate y { duration: 100ms; }
```

## States

The `states` statement alow to declare states like so:

```60
Example := Rectangle {
    text := Text { text: "hello"; }
    property<bool> pressed;
    property<bool> is_enabled;

    states [
        disabled when !is_enabled : {
            color: gray; // same as root.color: gray;
            text.color: white;
        }
        down when pressed : {
            color: blue;
        }
    ]
}
```

In that example, when the `is_enabled` property is set to false, the `disabled` state will be entered
This will change the color of the Rectangle and of the Text.

### Transitions

Complex animations can be declared on state transitions:

```60
Example := Rectangle {
    text := Text { text: "hello"; }
    property<bool> pressed;
    property<bool> is_enabled;

    states [
        disabled when !is_enabled : {
            color: gray; // same as root.color: gray;
            text.color: white;
        }
        down when pressed : {
            color: blue;
        }
    ]

    transitions [
        in down : {
            animate color { duration: 300ms; }
        }
        out disabled : {
            animate * { duration: 800ms; }
        }
    ]
}
```

## Global Singletons

You can declare global singleton for properties that are available in the entire project.
The syntax is `global Name := { /* .. properties or callbacks .. */ }`.
Then can be then used using the `Name.property` syntax.

```60
global Palette := {
    property<color> primary: blue;
    property<color> secondary: green;
}

Example := Rectangle {
    color: Palette.primary;
    border-color: Palette.secondary;
    border-width: 2px;
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

In the event that two files export a type under the same name, then you have the option
of assigning a different name at import time:

```60
import { Button } from "./button.60";
import { Button as CoolButton } from "../other_theme/button.60";

App := Rectangle {
    // ...
    CoolButton {} // from cool_button.60
    Button {} // from button.60
}
```

Elements, globals and structs can be exported and imported.

## Focus Handling

Certain elements such as ```TextInput``` accept not only input from the mouse/finger but
also key events originating from (virtual) keyboards. In order for an item to receive
these events, it must have the focus. This is visible through the `has_focus` property.

You can manually activate the focus on an element by calling `focus()`:

```60
import { Button } from "sixtyfps_widgets.60";

App := Window {
    VerticalLayout {
        alignment: start;
        Button {
            text: "press me";
            clicked => { input.focus(); }
        }
        input := TextInput {
            text: "I am a text input field";
        }
    }
}
```

If you have wrapped the `TextInput` in a component, then you can forward such a focus activation
using the `forward-focus` property to refer to the element that should receive it:

```60
LabeledInput := GridLayout {
    forward-focus: input;
    Row {
        Text {
            text: "Input Label:";
        }
        input := TextInput {}
    }
}

App := Window {
    GridLayout {
        Button {
            text: "press me";
            clicked => { label.focus(); }
        }
        label := LabeledInput {
        }
    }
}
``````

If you use the `forward-focus` property on a `Window`, then the specified element will receive
the focus the very first time the window receives the focus - it becomes the initial focus element.

## Builtin functions

 * **`debug(string) -> string`**

The debug function take a string as an argument and prints it

 * **`min`**, **`max`**

Return the arguments with the minimum (or maximum) value. All arguments must be of the same numeric type

 * **`mod(int, int) -> int`**

Perform a modulo operation.

 * **`round(float) -> int`**

Return the value rounded to the nearest integer

 * **`ceil(float) -> int`**, **`floor(float) -> int`**

 Return the ceiling or floor


