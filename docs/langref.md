# The `.60` language reference

This document describes the `.60` design markup language.

## `.60` files

The basic idea is that the `.60` files contains one or several components.
These components contain a tree of elements. Each declared component can be
given a name and re-used under that name as an element later.

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
        background: green;
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
MyButton := Text {
    // ...
}

MyApp := Window {
    hello := MyButton {
        text: "hello";
    }
    world := MyButton {
        text: "world";
        x: 50px;
    }
}
```

The outermost element of a component is always accessible under the name `root`.
The current element can be referred as `self`.
The parent element can be referred as `parent`.
These names are reserved and cannot be used as element names.

### Container Components

When creating components, it may sometimes be useful to influence where child elements
are placed when they are used. For example, imagine a component that draws a label above
whatever element the user places inside:

```60,ignore
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
        Rectangle { background: blue; }
        Rectangle { background: yellow; }
    }
}
```

## Comments

C-style comments are supported:

* line comments: `//` means everything to the end of the line is commented.
* block comments: `/* .. */`.  Note that the blocks comments can be nested, so `/* this is a /* single */ comment */`

## Identifiers

Identifiers can be composed of letter (`a-zA-Z`), of numbers (`0-9`), or of the underscore (`_`) or the dash (`-`).
They cannot start with a number or a dash (but they can start with underscore)
The underscores are normalized to dashes. Which means that these two identifiers are the same: `foo_bar` and `foo-bar`.

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
    // declare a property of type int with the name `my-property`
    property<int> my-property;

    // declare a property with a default value
    property<int> my-second-property: 42;
}
```

### Bindings

The expression on the right of a binding is automatically re-evaluated when the expression changes.

In the following example, the text of the button is automatically changed when the button is pressed, because
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
The type can be omitted in a property declaration to have the type automatically inferred.

```60
Example := Window {
    property<brush> rect-color <=> r.background;
    // it is allowed to omit the type to have it automatically inferred
    property rect-color2 <=> r.background;
    r:= Rectangle {
        width: parent.width;
        height: parent.height;
        background: blue;
    }
}
```

## Types

All properties in elements have a type. The following types are supported:

| Type | Description |
| --- | --- |
| `int` | Signed integral number. |
| `float` | Signed, 32-bit floating point number. Numbers with a `%` suffix are automatically divided by 100, so for example `30%` is the same as `0.30`. |
| `bool` | boolean whose value can be either `true` or `false`. |
| `string` | UTF-8 encoded, reference counted string. |
| `color` | RGB color with an alpha channel, with 8 bit precision for each channel. CSS color names as well as the hexadecimal color encodings are supported, such as `#RRGGBBAA` or `#RGB`. |
| `brush` | A brush is a special type that can be either initialized from a color or a gradient specification. See the [Colors Section](#colors) for more information. |
| `physical-length` | This is an amount of physical pixels. To convert from an integer to a length unit, one can simply multiply by `1px`.  Or to convert from a length to a float, one can divide by `1phx`. |
| `length` | The type used for `x`, `y`, `width` and `height` coordinates. Corresponds to a literal like `1px`, `1pt`, `1in`, `1mm`, or `1cm`. It can be converted to and from length provided the binding is run in a context where there is an access to the device pixel ratio. |
| `duration` | Type for the duration of animations. A suffix like `ms` (millisecond) or `s` (second) is used to indicate the precision. |
| `angle` | Angle measurement, corresponds to a literal like `90deg`, `1.2rad`, `0.25turn` |
| `easing` | Property animation allow specifying an easing curve. Valid values are `linear` (values are interpolated linearly) and the [four common cubiz-bezier functions known from CSS](https://developer.mozilla.org/en-US/docs/Web/CSS/easing-function#Keywords_for_common_cubic-bezier_easing_functions):  `ease`, `ease_in`, `ease_in_out`, `ease_out`. |
| `percent` | Signed, 32-bit floating point number that is interpreted as percentage. Literal number assigned to properties of this type must have a `%` suffix. |
| `image` | A reference to an image, can be initialized with the `@image-url("...")` construct |

Please see the language specific API references how these types are mapped to the APIs of the different programming languages.

### Structs

Anonymous structs type can be declared with curly braces: `{ identifier1: type2, identifier1: type2, }`
The trailing semicolon is optional.
They can be initialized with a struct literal: `{ identifier1: expression1, identifier2: expression2  }`

```60
Example := Window {
    property<{name: string, score: int}> player: { name: "Foo", score: 100 };
    property<{a: int, }> foo: { a: 3 };
}
```

### Custom named structures

It is possible to define a named struct using the `struct` keyword,

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
    property<[int]> list-of-int: [1,2,3];
    property<[{a: int, b: string}]> list-of-structs: [{ a: 1, b: "hello" }, {a: 2, b: "world"}];
}
```

### Conversions

* `int` can be converted implicitly to `float` and vice-versa
* `int` and `float` can be converted implicitly to `string`
* `physical-length` and `length` can be converted implicitly to each other only in
   context where the pixel ratio is known.
* the units type (`length`, `physical-length`, `duration`, ...) cannot be converted to numbers (`float` or `int`)
  but they can be divided by themselves to result in a number. Similarly, a number can be multiplied by one of
  these unit. The idea is that one would multiply by `1px` or divide by `1px` to do such conversions
* The literal `0` can be converted to any of these types that have associated unit.
* Struct types convert with another struct type if they have the same property names and their types can be converted.
    The source struct can have either missing properties, or extra properties. But not both.
* Array generally do not convert between each other. But array literal can be converted if the type does convert.
* String can be converted to float by using the `to-float` function. That function returns 0 if the string is not
   a valid number. you can check with `is-float` if the string contains a valid number

```60
Example := Window {
    // ok: int converts to string
    property<{a: string, b: int}> prop1: {a: 12, b: 12 };
    // ok even if a is missing, it will just have the default value
    property<{a: string, b: int}> prop2: { b: 12 };
    // ok even if c is too many, it will be discarded
    property<{a: string, b: int}> prop3: { a: "x", b: 12, c: 42 };
    // ERROR: b is missing and c is extra, this does not compile, because it could be a typo.
    // property<{a: string, b: int}> prop4: { a: "x", c: 42 };

    property<string> xxx: "42.1";
    property<float> xxx1: xxx.to-float(); // 42.1
    property<bool> xxx2: xxx.is-float(); // true
}
```

### Relative Lengths

Sometimes it is convenient to express the relationships of length properties in terms of relative percentages.
For example the following inner blue rectangle has half the size of the outer green one:

```60
Example := Rectangle {
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

* The property is `width` or `height`
* A binding expression evaluates to a percentage.

If these conditions are met, then it is not necessary to specify the parent property, instead you can simply
use the percentage. The earlier example then looks like this:

```60
Example := Rectangle {
    background: green;
    Rectangle {
        background: blue;
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

### Callback aliases

It is possible to declare callback aliases in a similar way to two-way bindings:

```60
Example := Rectangle {
    callback clicked <=> area.clicked;
    area := TouchArea {}
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
    property<int> my-property;

    // This accesses the property
    width: root.my-property * 20px;

}
```

If something changes `my-property`, the width will be updated automatically.

Arithmetic in expression with numbers works like in most programming language with the operators `*`, `+`, `-`, `/`:

```60
Example := Rectangle {
    property <int> p: 1 * 2 + 3 * 4; // same as (1 * 2) + (3 * 4)
}
```

`+` can also be applied with strings to mean concatenation.

There are also the operators `&&` and `||` for logical *and* and *or* between booleans. Comparisons of values of the same types can be done with
`==`, `!=`, `>`, `<`, `=>` and `<=`.

You can access properties by addressing the associated element, followed by a `.` and the property name:

```60
Example := Rectangle {
    foo := Rectangle {
        x: 42px;
    }
    x: foo.x;
}
```

The ternary operator `... ? ... : ...`  is also supported, like in C or JavaScript:

```60
Example := Rectangle {
    touch := TouchArea {}
    background: touch.pressed ? #111 : #eee;
    border-width: 1px;
    border-color: !touch.enabled ? #888
        : touch.pressed ? #aaa
        : #555;
}
```


### Strings

Strings can be used with surrounding quotes: `"foo"`.

Some character can be escaped with slashes (`\`)

| Escape | Result |
| --- | --- |
| `\"` | `"` |
| `\\` |`\` |
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
    background: blue;
    property<color> c1: #ffaaff;
}
```

(TODO: currently color name are only limited to a handful and only supported in color property)

In addition to plain colors, many elements have properties that are of type `brush` instead of `color`.
A brush is a type that can be either a color or gradient. The brush is then used to fill an element or
draw the outline.

#### Methods

All colors have methods that can be called on them:

* **`brighter(factor: float) -> Color`**

    Returns a new color that is derived from this color but has its brightness increased by the specified factor.
    For example if the factor is 0.5 (or for example 50%) the returned color is 50% brighter. Negative factors
    decrease the brightness.

* **`darker(factor: float) -> Color`**

    Returns a new color that is derived from this color but has its brightness decreased by the specified factor.
    For example if the factor is .5 (or for example 50%) the returned color is 50% darker. Negative factors
    increase the brightness.

#### Gradients

Gradients allow creating smooth colorful surfaces. They are specified using an angle and a series of
color stops. The colors will be linearly interpolated between the stops, aligned to an imaginary line
that is rotated by the specified angle. This is called a linear gradient and is specified using the
`@linear-gradient` macro with the following signature:

**`@linear-gradient(angle, color percentage, color percentage, ...)`**

The first parameter to the macro is an angle (see [Types](#types)). The gradient line's starting point
will be rotated by the specified value.

Following the initial angle is one or multiple color stops, describe as a space separated pair of a
`color` value and a `percentage`. The color specifies which value the linear color interpolation should
reach at the specified percentage along the axis of the gradient.

The following example shows a rectangle that's filled with a linear gradient that starts with a light blue
color, interpolates to a very light shade in the center and finishes with an orange tone:

```60
Example := Rectangle {
    width: 100px;
    height: 100px;
    background: @linear-gradient(90deg, #3f87a6 0%, #ebf8e1 50%, #f69d3c 100%);
}
```

### Images

The `image` type is a reference to an image. It be initialized with the `@image-url("...")` construct.
The URL within the `@image-url` function need to be known at compile time, and it is looked up
relative to the file. In addition, it will also be looked in the include path specified to load
.60 files via import.

It is possible to access the `width` and `height` of an image.

```60
Example := Text {
    property <image> some_image: @image-url("https://sixtyfps.io/resources/logo_scaled.png");
    text: "The image is " + some_image.width + "x" + some_image.height;
}
```

### Arrays/Structs

Arrays are currently only supported in `for` expressions. `[1, 2, 3]` is an array of integers.
All the types in the array have to be of the same type.
It is useful to have arrays of struct. An struct is between curly braces: `{ a: 12, b: "hello"}`.

## Statements

Inside callback handlers, more complicated statements are allowed:

Assignment:

```ignore
clicked => { some-property = 42; }
```

Self-assignment with `+=` `-=` `*=` `/=`

```ignore
clicked => { some-property += 42; }
```

Calling a callback

```ignore
clicked => { root.some-callback(); }
```

Conditional statements

```ignore
clicked => {
    if (condition) {
        foo = 42;
    } else if (other-condition) {
        bar = 28;
    } else {
        foo = 4;
    }
}
```

Empty expression

```ignore
clicked => { }
// or
clicked => { ; }
```

## Repetition

The `for`-`in` syntax can be used to repeat an element.

The syntax look like this: `for name[index] in model : id := Element { ... }`

The *model* can be of the following type:

* an integer, in which case the element will be repeated that amount of time
* an array type or a model declared natively, in which case the element will be instantiated for each element in the array or model.

The *name* will be available for lookup within the element and is going to be like a pseudo-property set to the
value of the model. The *index* is optional and will be set to the index of this element in the model.
The *id* is also optional.

### Examples

```60
Example := Window {
    height: 100px;
    width: 300px;
    for my-color[index] in [ #e11, #1a2, #23d ]: Rectangle {
        height: 100px;
        width: 60px;
        x: width * index;
        background: my-color;
    }
}
```

```60
Example := Window {
    height: 50px;
    width: 50px;
    property <[{foo: string, col: color}]> model: [
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

## Conditional element

Similar to `for`, the `if` construct can instantiate element only if a given condition is true.
The syntax is `if condition : id := Element { ... }`

```60
Example := Window {
    height: 50px;
    width: 50px;
    if true : foo := Rectangle { background: blue; }
    if false : Rectangle { background: red; }
}
```

## Animations

Simple animation that animates a property can be declared with `animate` like this:

```60
Example := Rectangle {
    property<bool> pressed;
    background: pressed ? blue : red;
    animate background {
        duration: 100ms;
    }
}
```

This will animate the color property for 100ms when it changes.

Animation can be configured with the following parameter:

* `duration`: the amount of time it takes for the animation to complete
* `loop-count`: FIXME
* `easing`: can be `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out`, `cubic-bezier(a, b, c, d)` as in CSS

It is also possible to animate several properties with the same animation:

```ignore
animate x, y { duration: 100ms; }
```
is the same as
```ignore
animate x { duration: 100ms; }
animate y { duration: 100ms; }
```

## States

The `states` statement allow to declare states like this:

```60
Example := Rectangle {
    text := Text { text: "hello"; }
    property<bool> pressed;
    property<bool> is-enabled;

    states [
        disabled when !is-enabled : {
            color: gray; // same as root.color: gray;
            text.color: white;
        }
        down when pressed : {
            background: blue;
        }
    ]
}
```

In that example, when the `is-enabled` property is set to false, the `disabled` state will be entered
This will change the color of the Rectangle and of the Text.

### Transitions

Complex animations can be declared on state transitions:

```60
Example := Rectangle {
    text := Text { text: "hello"; }
    property<bool> pressed;
    property<bool> is-enabled;

    states [
        disabled when !is-enabled : {
            color: gray; // same as root.color: gray;
            text.color: white;
        }
        down when pressed : {
            background: blue;
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
    background: Palette.primary;
    border-color: Palette.secondary;
    border-width: 2px;
}
```

It is possible to re-expose a callback or properties from a global using the two way binding syntax.

```60
global Logic := {
    property <int> the-value;
    callback magic-operation(int) -> int;
}

SomeComponent := Text {
    // use the global in any component
    text: "The magic value is:" + Logic.magic-operation(42);
}

export MainWindow := Window {
    // re-expose the global properties such that the native code
    // can access or modify them
    property the-value <=> Logic.the-value;
    callback magic-operation <=> Logic.magic-operation;

    SomeComponent {}
}
```

A global can be declared in another module file, and imported from many files.

It is also possible to access the properties and callbacks from globals in native code,
such as Rust or C++. In order to access them, it is necessary to mark them as exported
in the file that exports your main application component. In the above example it is
sufficient to directly export the `Logic` global:

```60,ignore
export global Logic := {
    property <int> the-value;
    callback magic-operation(int) -> int;
}
// ...
```

It's also possible to export globals from other files:

```60,ignore
import { Logic as MathLogic } from "math.60";
export { MathLogic } // known as "MathLogic" when using native APIs to access globals
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

```60,ignore
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

```60,ignore
import { Button } from "./button.60";
import { Button as CoolButton } from "../other_theme/button.60";

App := Rectangle {
    // ...
    CoolButton {} // from other_theme/button.60
    Button {} // from button.60
}
```

Elements, globals and structs can be exported and imported.

## Focus Handling

Certain elements such as ```TextInput``` accept not only input from the mouse/finger but
also key events originating from (virtual) keyboards. In order for an item to receive
these events, it must have the focus. This is visible through the `has-focus` property.

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
import { Button } from "sixtyfps_widgets.60";

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
```

If you use the `forward-focus` property on a `Window`, then the specified element will receive
the focus the very first time the window receives the focus - it becomes the initial focus element.

## Builtin functions

* **`debug(string) -> string`**

The debug function take a string as an argument and prints it

* **`min`**, **`max`**

Return the arguments with the minimum (or maximum) value. All arguments must be of the same numeric type

* **`mod(int, int) -> int`**

Perform a modulo operation.

* **`abs(float) -> float`**

Return the absolute value.

* **`round(float) -> int`**

Return the value rounded to the nearest integer

* **`ceil(float) -> int`**, **`floor(float) -> int`**

Return the ceiling or floor

* **`sin(angle) -> float`**, **`cos(angle) -> float`**, **`tan(angle) -> float`**, **`asin(float) -> angle`**, **`acos(float) -> angle`**, **`atan(float) -> angle`**

The trigonometry function. Note that the should be typed with `deg` or `rad` unit
(for example `cos(90deg)` or `sin(slider.value * 1deg)`).

* **`sqrt(float) -> float`**

Square root

* **`rgb(int, int, int) -> color`**,  **`rgba(int, int, int, float) -> color`**

Return the color as in CSS. Like in CSS, these two functions are actually aliases that can take
three or four parameters.

The first 3 parameters can be either number between 0 and 255, or a percentage with a `%` unit.
The fourth value, if present, is an alpha value between 0 and 1.

Unlike in CSS, the commas are mandatory.

## Font Handling

Elements such as `Text` and `TextInput` can render text and allow customizing the appearance of the text through
different properties. The properties prefixed with `font-`, such as `font-family`, `font-size` and `font-weight`
affect the choice of font used for rendering to the screen. If any of these properties is not specified, the `default-font-`
values in the surrounding `Window` element apply, such as `default-font-family`.

The fonts chosen for rendering are automatically picked up from the system. It is also possible to include custom
fonts in your design. A custom font must be a TrueType font (`.ttf`) or a TrueType font collection (`.ttc`).
You can select a custom font with the `import` statement: `import "./my_custom_font.ttf"` in a .60 file. This
instructions the SixtyFPS compiler to include the font and makes the font families globally available for use with
`font-family` properties.

For example:

```60
import "./NotoSans-Regular.ttf";

Example := Window {
    default-font-family: "Noto Sans";

    Text {
        text: "Hello World";
    }
}
```
