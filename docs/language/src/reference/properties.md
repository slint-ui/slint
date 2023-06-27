# Properties

All elements have properties. Built-in elements come with common properties such
as color or dimensional properties. You can assign values or entire
[expressions](expressions.md) to them:

```slint,no-preview
export component Example inherits Window {
    // Simple expression: ends with a semi colon
    width: 42px;
    // or a code block (no semicolon needed)
    height: { 42px }
}
```

The default value of a property is the default value of the type.
For example a boolean property defaults to `false`, an `int` property to zero, etc.

In addition to the existing properties, define extra properties by specifying the
name, the type, and optionally a default value:

```slint,no-preview
export component Example {
    // declare a property of type int with the name `my-property`
    property<int> my-property;

    // declare a property with a default value
    property<int> my-second-property: 42;
}
```

Annotate custom the properties with a qualifier that specifies how the
property can be read and written:

-   **`private`** (the default): The property can only be accessed from within the component.
-   **`in`**: The property is an input. It can be set and modified by the user of this component,
    for example through bindings or by assignment in callbacks.
    The component can provide a default binding, but it can't overwrite it by
    assignment
-   **`out`**: An output property that can only be set by the component. It's read-only for the
    users of the components.
-   **`in-out`**: The property can be read and modified by everyone.

```slint,no-preview
export component Button {
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

All properties declared at the top level of a component that aren't `private` are accessible from the outside when using a component as an element, or via the
language bindings from the business logic.

## Bindings

The binding expression is automatically re-evaluated when properties accessed in the expression change.

In the following example, the text of the button automatically changes when
the user presses the button. Incrementing the `counter` property automatically
invalidates the expression bound to `text` and triggers a re-evaluation.

```slint
import { Button } from "std-widgets.slint";
export component Example inherits Window {
    preferred-width: 50px;
    preferred-height: 50px;
    Button {
        property <int> counter: 3;
        clicked => { self.counter += 3 }
        text: self.counter * 2;
    }
}
```

The re-evaluation happens lazily when the property is queried.

Internally, a dependency is registered for any property accessed while evaluating a binding.
When a property changes, the dependencies are notified and all dependent bindings
are marked as dirty.

Callbacks in native code by default don't depend on any properties unless they query a property in the native code.

## Two-way Bindings

Create two-way bindings between properties with the `<=>` syntax. These properties will be linked
together and always contain the same value.

The right hand side of the `<=>` must be a reference to a property of the same type.
The property type is optional with two-way bindings, it will be inferred if not specified.

```slint,no-preview
export component Example  {
    in property<brush> rect-color <=> r.background;
    // It's allowed to omit the type to have it automatically inferred
    in property rect-color2 <=> r.background;
    r:= Rectangle {
        width: parent.width;
        height: parent.height;
        background: blue;
    }
}
```

## Relative Lengths

Sometimes it's convenient to express the relationships of length properties in terms of relative percentages.
For example the following inner blue rectangle has half the size of the outer green window:

```slint
export component Example inherits Window {
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

-   The property is `width` or `height`
-   A binding expression evaluates to a percentage.

If these conditions are met, then it's not necessary to specify the parent property, instead you can simply
use the percentage. The earlier example then looks like this:

```slint
export component Example inherits Window {
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
