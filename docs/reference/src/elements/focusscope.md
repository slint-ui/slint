## `FocusScope`

The `FocusScope` exposes callbacks to intercept key events. Note that `FocusScope`
will only invoke them when it `has-focus`.

The [`KeyEvent`](structs.md#keyevent) has a text property, which is a character of the key entered.
When a non-printable key is pressed, the character will be either a control character,
or it will be mapped to a private unicode character. The mapping of these non-printable, special characters is available in the [`Key`](namespaces.md#key) namespace

### Properties

-   **`has-focus`** (_out_ _bool_): Is `true` when the element has keyboard
    focus.
-   **`enabled`** (_in_ _bool_): When true, the `FocusScope` will make itself the focused element when clicked. Set this to false if you don't want the click-to-focus
    behavior. Similarly, a disabled `FocusScope` does not accept the focus via tab focus traversal. A parent `FocusScope` will still receive key events from
    child `FocusScope`s that were rejected, even if `enabled` is set to false. (default value: true)

### Functions

-   **`focus()`** Call this function to transfer keyboard focus to this `FocusScope`,
    to receive future [`KeyEvent`](structs.md#keyevent)s.
-   **`clear-focus()`** Call this function to remove keyboard focus from this `FocusScope` if it currently has the focus. See also [](../concepts/focus.md).

### Callbacks

-   **`key-pressed(KeyEvent) -> EventResult`**: Invoked when a key is pressed, the argument is a [`KeyEvent`](structs.md#keyevent) struct. The returned [`EventResult`](enums.md#eventresult) indicates whether to accept or ignore the event. Ignored events are
    forwarded to the parent element.
-   **`key-released(KeyEvent) -> EventResult`**: Invoked when a key is released, the argument is a [`KeyEvent`](structs.md#keyevent) struct. The returned [`EventResult`](enums.md#eventresult) indicates whether to accept or ignore the event. Ignored events are
    forwarded to the parent element.
-   **`focus-changed-event()`**: Invoked when the focus on the `FocusScope` has changed.

### Example

```slint,no-preview
export component Example inherits Window {
    width: 100px;
    height: 100px;
    forward-focus: my-key-handler;
    my-key-handler := FocusScope {
        key-pressed(event) => {
            debug(event.text);
            if (event.modifiers.control) {
                debug("control was pressed during this event");
            }
            if (event.text == Key.Escape) {
                debug("Esc key was pressed")
            }
            accept
        }
    }
}
```