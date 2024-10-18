## `SwipeGestureHandler`

Use the `SwipeGestureHandler` to handle swipe gesture in some particular direction. Recognition is limited to the element's geometry.

Specify the different swipe directions you'd like to handle by setting the `handle-swipe-left/right/up/down` properties and react to the gesture in the `swiped` callback.

Pointer press events on the recognizer's area are forwarded to the children with a small delay.
If the pointer moves by more than 8 logical pixels in one of the enabled swipe directions, the gesture is recognized, and events are no longer forwarded to the children.

### Properties

-   **`enabled`** (_in_ _bool_): When disabled, the `SwipeGestureHandler` doesn't recognize any gestures.
    (default value: `true`)
-   **`handle-swipe-left`**, **`handle-swipe-right`**, **`handle-swipe-up`**, **`handle-swipe-down`** (_out_ _bool_): Enable handling of swipes in the corresponding direction. (default value: `false`)
-   **`pressed-position`** (_out_ _Point_): The position of the pointer when the swipe started.
-   **`current-position`** (_out_ _Point_): The current pointer position.
-   **`swiping`** (_out_ _bool_): `true` while the gesture is recognized, false otherwise.

### Callbacks

-   **`moved()`**: Invoked when the pointer is moved.
-   **`swiped()`**: Invoked after the swipe gesture was recognized and the pointer was released.
-   **`cancelled()`**: Invoked when the swipe is cancelled programmatically or if the window loses focus.

### Functions

-   **`cancel()`**: Cancel any on-going swipe gesture recognition.

### Example

This example implements swiping between pages of different colors.

```slint
export component Example inherits Window {
    width: 270px;
    height: 100px;

    property <int> current-page: 0;

    sgr := SwipeGestureHandler {
        handle-swipe-right: current-page > 0;
        handle-swipe-left: current-page < 5;
        swiped => {
            if self.current-position.x > self.pressed-position.x + self.width / 4 {
                current-page -= 1;
            } else if self.current-position.x < self.pressed-position.x - self.width / 4 {
                current-page += 1;
            }
        }

        HorizontalLayout {
            property <length> position: - current-page * root.width;
            animate position { duration: 200ms; easing: ease-in-out; }
            property <length> swipe-offset;
            x: position + swipe-offset;
            states [
                swiping when sgr.swiping : {
                    swipe-offset: sgr.current-position.x - sgr.pressed-position.x;
                    out { animate swipe-offset { duration: 200ms; easing: ease-in-out; }  }
                }
            ]

            Rectangle { width: root.width; background: green; }
            Rectangle { width: root.width; background: limegreen; }
            Rectangle { width: root.width; background: yellow; }
            Rectangle { width: root.width; background: orange; }
            Rectangle { width: root.width; background: red; }
            Rectangle { width: root.width; background: violet; }
        }
    }
}
```