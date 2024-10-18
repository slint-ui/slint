## `TouchArea`

Use `TouchArea` to control what happens when the region it covers is touched or interacted with
using the mouse.

When not part of a layout, its width or height default to 100% of the parent element.

### Properties

-   **`has-hover`** (_out_ _bool_): `TouchArea` sets this to `true` when the mouse is over it.
-   **`mouse-cursor`** (_in_ _enum [`MouseCursor`](../language/builtins/enums.md#mousecursor)_): The mouse cursor type when the mouse is hovering the `TouchArea`.
-   **`mouse-x`**, **`mouse-y`** (_out_ _length_): Set by the `TouchArea` to the position of the mouse within it.
-   **`pressed-x`**, **`pressed-y`** (_out_ _length_): Set by the `TouchArea` to the position of the mouse at the moment it was last pressed.
-   **`pressed`** (_out_ _bool_): Set to `true` by the `TouchArea` when the mouse is pressed over it.

### Callbacks

-   **`clicked()`**: Invoked when clicked: A finger or the left mouse button is pressed, then released on this element.
-   **`double-clicked()`**: Invoked when double-clicked. The left mouse button is pressed and released twice on this element in a short
    period of time, or the same is done with a finger. The `clicked()` callbacks will be triggered before the `double-clicked()` callback is triggered.
-   **`moved()`**: The mouse or finger has been moved. This will only be called if the mouse is also pressed or the finger continues to touch
    the display. See also **pointer-event(PointerEvent)**.
-   **`pointer-event(PointerEvent)`**: Invoked when a button was pressed or released, a finger touched, or the pointer moved.
    The [_`PointerEvent`_](language/builtins/structs.md#pointerevent) argument contains information such which button was pressed
    and any active keyboard modifiers.
    In the [_`PointerEventKind::Move`_](../language/builtins/structs.md#pointereventkind) case the `buttons` field will always
    be set to `PointerEventButton::Other`, independent of whether any button is pressed or not.
-   **`scroll-event(PointerScrollEvent) -> EventResult`**: Invoked when the mouse wheel was rotated or another scroll gesture was made.
    The [_`PointerScrollEvent`_](../language/builtins/structs.md#pointerscrollevent) argument contains information about how much to scroll in what direction.
    The returned [`EventResult`](../language/builtins/enums.md#eventresult) indicates whether to accept or ignore the event. Ignored events are
    forwarded to the parent element.

### Example

```slint
export component Example inherits Window {
    width: 200px;
    height: 100px;
    area := TouchArea {
        width: parent.width;
        height: parent.height;
        clicked => {
            rect2.background = #ff0;
        }
    }
    Rectangle {
        x:0;
        width: parent.width / 2;
        height: parent.height;
        background: area.pressed ? blue: red;
    }
    rect2 := Rectangle {
        x: parent.width / 2;
        width: parent.width / 2;
        height: parent.height;
    }
}
```