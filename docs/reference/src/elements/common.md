## Common properties

### Geometry

These properties are valid on all visible items:

-   **`width`** and **`height`** (_in_ _length_): The size of the element. When set, this overrides the default size.
-   **`x`** and **`y`** (_in_ _length_): The position of the element relative to its parent.
-   **`z`** (_in_ _float_): Allows to specify a different order to stack the items with its siblings.
    The value must be a compile time constant. (default value: 0)
-   **`absolute-position`** (_out_ _Point_): The position of the element within the contained window.

### Layout

These properties are valid on all visible items and can be used to specify constraints when used in layouts:

-   **`col`**, **`row`**, **`colspan`**, **`rowspan`** (_in_ _int_): See [`GridLayout`](#gridlayout).
-   **`horizontal-stretch`** and **`vertical-stretch`** (_in-out_ _float_): Specify how much relative space these elements are stretching in a layout. When 0, this means that the elements won't be stretched unless all elements are 0. Builtin widgets have a value of either 0 or 1.
-   **`max-width`** and **`max-height`** (_in_ _length_): The maximum size of an element
-   **`min-width`** and **`min-height`** (_in_ _length_): The minimum size of an element
-   **`preferred-width`** and **`preferred-height`** (_in_ _length_): The preferred size of an element

### Miscellaneous

-   **`cache-rendering-hint`** (_in_ _bool_): When set to `true`, this provides a hint to the renderer to cache the contents of the element and all the children into an intermediate cached layer. For complex sub-trees that rarely change this may speed up the rendering, at the expense of increased memory consumption. Not all rendering backends support this, so this is merely a hint. (default value: `false`)
-   **`dialog-button-role`** (_in_ _enum [`DialogButtonRole`](../language/builtins/enums.md#dialogbuttonrole)_): Specify that this is a button in a `Dialog`.
-   **`opacity`** (_in_ _float_): A value between 0 and 1 (or a percentage) that is used to draw
    the element and its children with transparency.
    0 is fully transparent (invisible), and 1 is fully opaque.
    The opacity is applied to the tree of child elements as if they
    were first drawn into an intermediate layer, and then the whole layer is rendered with this opacity.
    (default value: 1)
-   **`visible`** (_in_ _bool_): When set to `false`, the element and all his children won't be drawn and not react to mouse input (default value: `true`)

The following example demonstrates the `opacity` property with children. An opacity is applied to the red rectangle. Since the green rectangle is a child of the red one, you can see the gradient underneath it, but you can't see the red rectangle through the green one.

```slint,no-preview
export component Example inherits Window {
    width: 100px;
    height: 100px;
    background: @radial-gradient(circle, black, white, black, white);
    Rectangle {
        opacity: 0.5;
        background: red;
        border-color: #822;
        border-width: 5px;
        width: 50px; height: 50px;
        x: 10px; y: 10px;
        Rectangle {
            background: green;
            border-color: #050;
            border-width: 5px;
            width: 50px; height: 50px;
            x: 25px; y: 25px;
        }
    }
}
```

### Accessibility

Use the following `accessible-` properties to make your items interact well with software like screen readers, braille terminals and other software to make your application accessible.
`accessible-role` must be set in order to be able to set any other accessible property or callback.

-   **`accessible-role`** (_in_ _enum [`AccessibleRole`](../language/builtins/enums.md#accessiblerole)_): The role of the element. This property is mandatory to be able to use any other accessible properties. It should be set to a constant value. (default value: `none` for most elements, but `text` for the Text element)
-   **`accessible-checkable`** (_in_ _bool_): Whether the element is can be checked or not.
-   **`accessible-checked`** (_in_ _bool_): Whether the element is checked or not. This maps to the "checked" state of checkboxes, radio buttons, and other widgets.
-   **`accessible-description`** (_in_ _string_): The description for the current element.
-   **`accessible-label`** (_in_ _string_): The label for an interactive element. (default value: empty for most elements, or the value of the `text` property for Text elements)
-   **`accessible-value-maximum`** (_in_ _float_): The maximum value of the item. This is used for example by spin boxes.
-   **`accessible-value-minimum`** (_in_ _float_): The minimum value of the item.
-   **`accessible-value-step`** (_in_ _float_) The smallest increment or decrement by which the current value can change. This corresponds to the step by which a handle on a slider can be dragged.
-   **`accessible-value`** (_in_ _string_): The current value of the item.
-   **`accessible-placeholder-text`** (_in_ _string_): A placeholder text to use when the item's value is empty. Applies to text elements.
-   **`accessible-selectable`** (_in_ _bool_): Whether the element can be selected or not.
-   **`accessible-selected`** (_in_ _bool_): Whether the element is selected or not. This maps to the "is-selected" state of listview items.

You can also use the following callbacks that are going to be called by the accessibility framework:

-  **`accessible-action-default()`**: Invoked when the default action for this widget is requested (eg: pressed for a button).
-  **`accessible-action-set-value(string)`**: Invoked when the user wants to change the accessible value.
-  **`accessible-action-increment()`**: Invoked when the user requests to increment the value.
-  **`accessible-action-decrement()`**: Invoked when the user requests to decrement the value.

### Drop Shadows

To achieve the graphical effect of a visually elevated shape that shows a shadow effect underneath the frame of
an element, it's possible to set the following `drop-shadow` properties:

-   **`drop-shadow-blur`** (_in_ _length_): The radius of the shadow that also describes the level of blur applied to the shadow. Negative values are ignored and zero means no blur. (default value: 0)
-   **`drop-shadow-color`** (_in_ _color_): The base color of the shadow to use. Typically that color is the starting color of a gradient that fades into transparency.
-   **`drop-shadow-offset-x`** and **`drop-shadow-offset-y`** (_in_ _length_): The horizontal and vertical distance of the shadow from the element's frame. A negative value places the shadow left / above of the element.

The `drop-shadow` effect is supported for `Rectangle` elements.