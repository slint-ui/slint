# The `.slint` language reference

The Slint design markup language is used to describe graphical user interfaces:

 * Place and compose a tree of visual elements in a window using a textual representation.
 * Configure the appearance of elements via properties. For example a `Text` element has font and text
   properties, while a `Rectangle` element offers a background color.
 * Assign binding expressions to properties to automatically compute values that depend on other properties.
 * Group binding expressions together with named states and conditions.
 * Declare animations on properties and states to make the user interface feel alive.
 * Build your own re-usable components and share them in `.slint` module files.
 * Define data structures and models and access them from programming languages.
 * Build highly customized user interfaces with the [builtin elements](builtin_elements.md) provided.

Slint also comes with a catalog of high-level [widgets](widgets.md), that are written in the `.slint`
language.

## `.slint` files

The basic idea is that the `.slint` files contains one or several components.
These components contain a tree of elements. Each declared component can be
given a name and re-used under that name as an element later.

Below is an example of components and elements:

```slint

MyButton := Text {
    color: black;
    // ...
}

export MyApp := Window {
    preferred-width: 200px;
    preferred-height: 100px;
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

```slint
MyButton := Text {
    // ...
}

MyApp := Window {
    preferred-width: 200px;
    preferred-height: 100px;

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

```slint,ignore
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

```slint
BoxWithLabel := GridLayout {
    Row {
        Text { text: "label text here"; }
    }
    Row {
        @children
    }
}

MyApp := Window {
    preferred-height: 100px;
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

```slint,no-preview
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

```slint,no-preview
Example := Rectangle {
    // declare a property of type int with the name `my-property`
    property<int> my-property;

    // declare a property with a default value
    property<int> my-second-property: 42;
}
```

You can annotate the properties with a qualifier that specifies how the property can be read and written:

 * **`private`**: The property can only be accessed from within the component.
 * **`in`**: The property is an input. It can be set and modified by the user of this component,
   for example through bindings or by assignment in callbacks.
   The component can provide a default binding, but it cannot overwrite it by
   assignment
 * **`out`**: An output property that can only be set by the component. It is read-only for the
   users of the components.
 * **`in-out`** (the default): The property can be read and modified by everyone.

```slint,no-preview
Button := Rectangle {
    // This is meant to be set by the user of the component.
    in property <string> text;
    // This property is meant to be read by the user of the component.
    out property <bool> pressed;
    // This property is meant to both be changed by the user and the component itself.
    in-out property <bool> checked;

    // This property is internal to this component.
    private property <bool> has-mouse;
}
```

### Bindings

The expression on the right of a binding is automatically re-evaluated when the expression changes.

In the following example, the text of the button is automatically changed when the button is pressed, because
changing the `counter`  property automatically changes the text.

```slint
import { Button } from "std-widgets.slint";
Example := Window {
    preferred-width: 50px;
    preferred-height: 50px;
    Button {
        property <int> counter: 3;
        clicked => { counter += 3 }
        text: counter * 2;
    }
}
```

The re-evaluation happens when the property is queried. Internally, a dependency will be registered
for any property accessed while evaluating this binding. When the dependent properties are changed,
all the dependent bindings are marked dirty. Callbacks in native code by default do not depend on
any properties unless they query a property in the native code.

### Two-way Bindings

Using the `<=>` syntax, one can create two ways binding between properties. These properties are now linked
together.
The right hand side of the `<=>` must be a reference to a property of the same type.
The type can be omitted in a property declaration to have the type automatically inferred.

```slint,no-preview
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
| `relative-font-size` | Relative font size factor that is multiplied with the `Window.default-font-size` and can be converted to a `length`. |

Please see the language specific API references how these types are mapped to the APIs of the different programming languages.

### Structs

Anonymous structs type can be declared with curly braces: `{ identifier1: type2, identifier1: type2, }`
The trailing semicolon is optional.
They can be initialized with a struct literal: `{ identifier1: expression1, identifier2: expression2  }`

```slint,no-preview
Example := Window {
    property<{name: string, score: int}> player: { name: "Foo", score: 100 };
    property<{a: int, }> foo: { a: 3 };
}
```

### Custom named structures

It is possible to define a named struct using the `struct` keyword,

```slint,no-preview
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

```slint,no-preview
Example := Window {
    property<[int]> list-of-int: [1,2,3];
    property<[{a: int, b: string}]> list-of-structs: [{ a: 1, b: "hello" }, {a: 2, b: "world"}];
}
```

* **`length`**: One can query the length of an array and model using the builtin `.length` property.
* **`array[index]`**: Individual elements of an array can be retrieved using the `array[index]` syntax.

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

```slint,no-preview
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
For example the following inner blue rectangle has half the size of the outer green window:

```slint
Example := Window {
    preferred-width: 100px;
    preferred-height: 100px;

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

```slint
Example := Window {
    preferred-width: 100px;
    preferred-height: 100px;

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

```slint,no-preview
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

```slint,no-preview
Example := Rectangle {
    // declares a callback
    callback hello(int, string);
    hello(aa, bb) => { /* ... */ }
}
```

And return value.

```slint,no-preview
Example := Rectangle {
    // declares a callback with a return value
    callback hello(int, int) -> int;
    hello(aa, bb) => { aa + bb }
}
```

### Callback aliases

It is possible to declare callback aliases in a similar way to two-way bindings:

```slint,no-preview
Example := Rectangle {
    callback clicked <=> area.clicked;
    area := TouchArea {}
}
```

## Functions

You can declare helper functions with the function keyword.
Functions are private by default, but can be made public with the `public` annotation.

```slint,no-preview
Example := Rectangle {
    property <int> min;
    property <int> max;
    public function inbound(x: int) -> int {
        return Math.min(root.max, Math.max(root.min, x));
    }
}
```

## Expressions

Expressions are a powerful way to declare relationships and connections in your user interface. They
are typically used to combine basic arithmetic with access to properties of other elements. When
these properties change, the expression is automatically re-evaluated and a new value is assigned
to the property the expression is associated with:

```slint,no-preview
Example := Rectangle {
    // declare a property of type int
    property<int> my-property;

    // This accesses the property
    width: root.my-property * 20px;

}
```

If something changes `my-property`, the width will be updated automatically.

Arithmetic in expression with numbers works like in most programming language with the operators `*`, `+`, `-`, `/`:

```slint,no-preview
Example := Rectangle {
    property <int> p: 1 * 2 + 3 * 4; // same as (1 * 2) + (3 * 4)
}
```

`+` can also be applied with strings to mean concatenation.

There are also the operators `&&` and `||` for logical *and* and *or* between booleans. Comparisons of values of the same types can be done with
`==`, `!=`, `>`, `<`, `=>` and `<=`.

You can access properties by addressing the associated element, followed by a `.` and the property name:

```slint,no-preview
Example := Rectangle {
    foo := Rectangle {
        x: 42px;
    }
    x: foo.x;
}
```

The ternary operator `... ? ... : ...`  is also supported, like in C or JavaScript:

```slint
Example := Window {
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

```slint,no-preview
Example := Text {
    text: "hello";
}
```

### Colors and Brushes

Color literals follow the syntax of CSS:

```slint,no-preview
Example := Window {
    background: blue;
    property<color> c1: #ffaaff;
    property<brush> b2: Colors.red;
}
```

In addition to plain colors, many elements have properties that are of type `brush` instead of `color`.
A brush is a type that can be either a color or gradient. The brush is then used to fill an element or
draw the outline.

CSS Color names are only in scope in expressions of type `color` or `brush`. Otherwise, you can access
colors from the `Colors` namespace.

#### Methods

All colors and brushes have methods that can be called on them:

* **`brighter(factor: float) -> Brush`**

    Returns a new color that is derived from this color but has its brightness increased by the specified factor.
    For example if the factor is 0.5 (or for example 50%) the returned color is 50% brighter. Negative factors
    decrease the brightness.

* **`darker(factor: float) -> Brush`**

    Returns a new color that is derived from this color but has its brightness decreased by the specified factor.
    For example if the factor is .5 (or for example 50%) the returned color is 50% darker. Negative factors
    increase the brightness.

#### Linear Gradients

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

```slint
Example := Window {
    preferred-width: 100px;
    preferred-height: 100px;

    Rectangle {
        background: @linear-gradient(90deg, #3f87a6 0%, #ebf8e1 50%, #f69d3c 100%);
    }
}
```

#### Radial Gradients

Linear gradiants are like real gradiant but the colors is interpolated in a circle instead of
along a line. To describe a readial gradiant, use the `@radial-gradient` macro with the following signature:

**`@radial-gradient(circle, color percentage, color percentage, ...)`**

The first parameter to the macro is always `circle` because only circular radients are supported.
The syntax is otherwise based on the CSS `radial-gradient` function.

Example:

```slint
Example := Window {
    preferred-width: 100px;
    preferred-height: 100px;
    Rectangle {
        background: @radial-gradient(circle, #f00 0%, #0f0 50%, #00f 100%);
    }
}
```

### Images

The `image` type is a reference to an image. It be initialized with the `@image-url("...")` construct.
The URL within the `@image-url` function need to be known at compile time, and it is looked up
relative to the file. In addition, it will also be looked in the include path specified to load
.slint files via import.

It is possible to access the `width` and `height` of an image.

```slint
Example := Window {
    preferred-width: 150px;
    preferred-height: 50px;

    property <image> some_image: @image-url("https://slint-ui.com/logo/slint-logo-full-light.svg");

    Text {
        text: "The image is " + some_image.width + "x" + some_image.height;
    }
}
```

### Arrays/Structs

Arrays are currently only supported in `for` expressions. `[1, 2, 3]` is an array of integers.
All the types in the array have to be of the same type.
It is useful to have arrays of struct. An struct is between curly braces: `{ a: 12, b: "hello"}`.

## Statements

Inside callback handlers, more complicated statements are allowed:

Assignment:

```slint,ignore
clicked => { some-property = 42; }
```

Self-assignment with `+=` `-=` `*=` `/=`

```slint,ignore
clicked => { some-property += 42; }
```

Calling a callback

```slint,ignore
clicked => { root.some-callback(); }
```

Conditional statements

```slint,ignore
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

```slint,ignore
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

```slint
Example := Window {
    preferred-width: 300px;
    preferred-height: 100px;
    for my-color[index] in [ #e11, #1a2, #23d ]: Rectangle {
        height: 100px;
        width: 60px;
        x: width * index;
        background: my-color;
    }
}
```

```slint
Example := Window {
    preferred-width: 50px;
    preferred-height: 50px;
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

```slint
Example := Window {
    preferred-width: 50px;
    preferred-height: 50px;
    if area.pressed : foo := Rectangle { background: blue; }
    if !area.pressed : Rectangle { background: red; }
    area := TouchArea {}
}
```

## Animations

Simple animation that animates a property can be declared with `animate` like this:

```slint
Example := Window {
    preferred-width: 100px;
    preferred-height: 100px;

    background: area.pressed ? blue : red;
    animate background {
        duration: 250ms;
    }

    area := TouchArea {}
}
```

This will animate the color property for 100ms when it changes.

Animation can be configured with the following parameter:

* `delay`: the amount of time to wait before starting the animation
* `duration`: the amount of time it takes for the animation to complete
* `iteration-count`: The number of times a animation should run. A negative value specifies
    infinite reruns. Fractual values are possible.
* `easing`: can be `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out`, `cubic-bezier(a, b, c, d)` as in CSS

It is also possible to animate several properties with the same animation:

```slint,ignore
animate x, y { duration: 100ms; }
```

is the same as

```slint,ignore
animate x { duration: 100ms; }
animate y { duration: 100ms; }
```

## States

The `states` statement allow to declare states like this:

```slint
Example := Window {
    preferred-width: 100px;
    preferred-height: 100px;

    text := Text { text: "hello"; }
    property<bool> pressed;
    property<bool> is-enabled;

    states [
        disabled when !is-enabled : {
            background: gray; // same as root.background: gray;
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

```slint
Example := Window {
    preferred-width: 100px;
    preferred-height: 100px;

    text := Text { text: "hello"; }
    property<bool> pressed;
    property<bool> is-enabled;

    states [
        disabled when !is-enabled : {
            background: gray; // same as root.background: gray;
            text.color: white;
            out {
                animate * { duration: 800ms; }
            }
        }
        down when pressed : {
            background: blue;
            in {
                animate background { duration: 300ms; }
            }
        }
    ]
}
```

## Global Singletons

Declare a global singleton with `global Name := { /* .. properties or callbacks .. */ }` when you want to
make properties and callbacks available throughout the entire project. Access them using `Name.property`.

For example, this can be useful for a common color palette:

```slint,no-preview
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

A global can be declared in another module file, and imported from many files.

Access properties and callbacks from globals in native code by marking them as exported
in the file that exports your main application component. In the above example it is
sufficient to directly export the `Logic` global:

```slint,ignore
export global Logic := {
    property <int> the-value;
    callback magic-operation(int) -> int;
}
// ...
```

It's also possible to export globals from other files:

```slint,ignore
import { Logic as MathLogic } from "math.slint";
export { MathLogic } // known as "MathLogic" when using native APIs to access globals
```

<details data-snippet-language="rust">
<summary>Usage from Rust</summary>

```rust
slint::slint!{
export global Logic := {
    property <int> the-value;
    callback magic-operation(int) -> int;
}

export App := Window {
    // ...
}
}

fn main() {
    let app = App::new();
    app.global::<Logic>().on_magic_operation(|value| {
        eprintln!("magic operation input: {}", value);
        value * 2
    });
    app.global::<Logic>().set_the_value(42);
    // ...
}
```
</details>

<details data-snippet-language="cpp">
<summary>Usage from C++</summary>

```cpp
#include "app.h"

fn main() {
    auto app = App::create();
    app->global<Logic>().on_magic_operation([](int value) -> int {
        return value * 2;
    });
    app->global<Logic>().set_the_value(42);
    // ...
}
```
</details>

It is possible to re-expose a callback or properties from a global using the two way binding syntax.

```slint,no-preview
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


## Modules

Components declared in a .slint file can be shared with components in other .slint files, by means of exporting and importing them.
By default, everything declared in a .slint file is private, but it can be made accessible from the outside using the export
keyword:

```slint,no-preview
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

In the above example, `Button` is usable from other .slint files, but `ButtonHelper` isn't.

It's also possible to change the name just for the purpose of exporting, without affecting its internal use:

```slint,no-preview
Button := Rectangle {
    // ...
}

export { Button as ColorButton }
```

In the above example, ```Button``` is not accessible from the outside, but instead it is available under the name ```ColorButton```.

For convenience, a third way of exporting a component is to declare it exported right away:

```slint,no-preview
export Button := Rectangle {
    // ...
}
```

Similarly, components exported from other files can be accessed by importing them:

```slint,ignore
import { Button } from "./button.slint";

App := Rectangle {
    // ...
    Button {
        // ...
    }
}
```

In the event that two files export a type under the same name, then you have the option
of assigning a different name at import time:

```slint,ignore
import { Button } from "./button.slint";
import { Button as CoolButton } from "../other_theme/button.slint";

App := Rectangle {
    // ...
    CoolButton {} // from other_theme/button.slint
    Button {} // from button.slint
}
```

Elements, globals and structs can be exported and imported.

### Module Syntax

The following syntax is supported for importing types:

```slint,ignore
import { export1 } from "module.slint";
import { export1, export2 } from "module.slint";
import { export1 as alias1 } from "module.slint";
import { export1, export2 as alias2, /* ... */ } from "module.slint";
```

The following syntax is supported for exporting types:

```slint,ignore
// Export declarations
export MyButton := Rectangle { /* ... */ }

// Export lists
MySwitch := Rectangle { /* ... */ }
export { MySwitch };
export { MySwitch as Alias1, MyButton as Alias2 };

// Re-export all types from other module
export * from "other_module.slint";
```

## Focus Handling

Certain elements such as ```TextInput``` accept not only input from the mouse/finger but
also key events originating from (virtual) keyboards. In order for an item to receive
these events, it must have the focus. This is visible through the `has-focus` property.

You can manually activate the focus on an element by calling `focus()`:

```slint
import { Button } from "std-widgets.slint";

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

```slint
import { Button } from "std-widgets.slint";

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

* **`animation-tick() -> duration`**:  This function returns a monotonically increasing time, which can be used for animations.
    Calling this function from a binding will constantly re-evaluate the binding.
    It can be used like so: `x: 1000px + sin(animation-tick() / 1s * 360deg) * 100px;` or `y: 20px * mod(animation-tick(), 2s) / 2s `

```slint
Example := Window {
    preferred-width: 100px;
    preferred-height: 100px;

    Rectangle {
        background: red;
        height: 50px;
        width: parent.width * mod(animation-tick(), 2s) / 2s;
    }

    Rectangle {
        background: blue;
        height: 50px;
        y: 50px;
        width: parent.width * abs(sin(360deg * animation-tick() / 3s));
    }
}
```

## Builtin callbacks

Every element implicitly declares an `init` callback. You can assign a code block to it that will be invoked when the
element is instantiated and after all properties are initialized with the value of their final binding. The order of
invocation is from inside to outside. The following example will print "first", then "second", and then "third":

```slint,no-preview
MyButton := Rectangle {
    property <string> text: "Initial";
    init => {
        // If `text` is queried here, it will have the value "Hello".
        debug("first");
    }
}

MyCheckBox := Rectangle {
    init => { debug("second"); }
}

MyWindow := Window {
    MyButton {
        text: "Hello";
        init => { debug("third"); }
    }
    MyCheckBox {
    }
}
```

Do not use this callback to initialize properties, because this violates the the declarative principle.
Avoid using this callback, unless you need it, for example, in order to notify some native code:

```slint,no-preview
global SystemService := {
    // This callback can be implemented in native code using the Slint API
    callback ensure_service_running();
}

MySystemButton := Rectangle {
    init => {
        SystemService.ensure_service_running();
    }
    // ...
}
```

### `Math` namespace

These functions are available both in the global scope and in the `Math` namespace.

* **`min`**, **`max`**

Return the arguments with the minimum (or maximum) value. All arguments must be of the same numeric type

* **`mod(T, T) -> T`**

Perform a modulo operation, where T is some numeric type.

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

* **`pow(float, float) -> float`**

Return the value of the first value raised to the second

* **`log(float, float) -> float`**

Return the log of the first value with a base of the second value

### `Colors` namespace

These functions are available both in the global scope, and in the `Colors` namespace.

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
You can select a custom font with the `import` statement: `import "./my_custom_font.ttf"` in a .slint file. This
instructions the Slint compiler to include the font and makes the font families globally available for use with
`font-family` properties.

For example:

```slint,ignore
import "./NotoSans-Regular.ttf";

Example := Window {
    default-font-family: "Noto Sans";

    Text {
        text: "Hello World";
    }
}
```
