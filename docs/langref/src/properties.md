# Properties

All elements have properties. Built-in elements come with common properties such
as color or dimensional properties. You can assign values or entire
[expressions](#expressions) to them:

```slint,no-preview
export component Example inherits Window {
    // Simple expression: ends with a semi colon
    width: 42px;
    // or a code block (no semicolon needed)
    height: { 42px }
}
```

In addition to the existing properties, you may define extra properties. In
additon to the property name you must also specify the type of the property
and optionally a default value:

```slint,no-preview
export component Example {
    // declare a property of type int with the name `my-property`
    property<int> my-property;

    // declare a property with a default value
    property<int> my-second-property: 42;
}
```

You can annotate custom the properties with a qualifier that specifies how the
property can be read and written:

-   **`private`** (the default): The property can only be accessed from within the component.
-   **`in`**: The property is an input. It can be set and modified by the user of this component,
    for example through bindings or by assignment in callbacks.
    The component can provide a default binding, but it cannot overwrite it by
    assignment
-   **`out`**: An output property that can only be set by the component. It is read-only for the
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

All properties declared at the top level of a component that are not `private` can
get accessed from the outside when using a component as an element, or via the
language bindings from the business logic.

Note: In the legacy syntax, the default was `in-out`.
