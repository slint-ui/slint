<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Builtin Elements



## `Dialog`

Dialog is like a window, but it has buttons that are automatically laid out.

A Dialog should have one main element as child, that isn't a button.
The dialog can have any number of `StandardButton` widgets or other buttons
with the `dialog-button-role` property.
The buttons will be placed in an order that depends on the target platform at run-time.

The `kind` property of the `StandardButton`s and the `dialog-button-role` properties need to be set to a constant value, it can't be an arbitrary variable expression.
There can't be several `StandardButton`s of the same kind.

A callback `<kind>_clicked` is automatically added for each `StandardButton` which doesn't have an explicit
callback handler, so it can be handled from the native code: For example if there is a button of kind `cancel`,
a `cancel_clicked` callback will be added.
Each of these automatically-generated callbacks is an alias for the `clicked` callback of the associated `StandardButton`.

### Properties

-   **`icon`** (_in_ _image_): The window icon shown in the title bar or the task bar on window managers supporting it.
-   **`title`** (_in_ _string_): The window title that is shown in the title bar.

### Example

```slint
import { StandardButton, Button } from "std-widgets.slint";
export component Example inherits Dialog {
    Text {
      text: "This is a dialog box";
    }
    StandardButton { kind: ok; }
    StandardButton { kind: cancel; }
    Button {
      text: "More Info";
      dialog-button-role: action;
    }
}
```













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
-   **`swiped()`**: Invoked after the swipe gesture was recognised and the pointer was released.
-   **`cancelled()`**: Invoked when the swipe is cancelled programatically or if the window loses focus.

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

## `TextInput`

The `TextInput` is a lower-level item that shows text and allows entering text.

When not part of a layout, its width or height defaults to 100% of the parent element when not specified.

### Properties

-   **`color`** (_in_ _brush_): The color of the text (default value: depends on the style)
-   **`font-family`** (_in_ _string_): The name of the font family selected for rendering the text.
-   **`font-size`** (_in_ _length_): The font size of the text.
-   **`font-weight`** (_in_ _int_): The weight of the font. The values range from 100 (lightest) to 900 (thickest). 400 is the normal weight.
-   **`font-italic`** (_in_ _bool_): Whether or not the font face should be drawn italicized or not. (default value: false)
-   **`font-metrics`** (_out_ _struct [`FontMetrics`](structs.md#fontmetrics)_): The design metrics of the font scaled to the font pixel size used by the element.
-   **`has-focus`** (_out_ _bool_): `TextInput` sets this to `true` when it's focused. Only then it receives [`KeyEvent`](structs.md#keyevent)s.
-   **`horizontal-alignment`** (_in_ _enum [`TextHorizontalAlignment`](enums.md#texthorizontalalignment)_): The horizontal alignment of the text.
-   **`input-type`** (_in_ _enum [`InputType`](enums.md#inputtype)_): Use this to configure `TextInput` for editing special input, such as password fields. (default value: `text`)
-   **`letter-spacing`** (_in_ _length_): The letter spacing allows changing the spacing between the glyphs. A positive value increases the spacing and a negative value decreases the distance. (default value: 0)
-   **`read-only`** (_in_ _bool_): When set to `true`, text editing via keyboard and mouse is disabled but selecting text is still enabled as well as editing text programatically. (default value: `false`)
-   **`selection-background-color`** (_in_ _color_): The background color of the selection.
-   **`selection-foreground-color`** (_in_ _color_): The foreground color of the selection.
-   **`single-line`** (_in_ _bool_): When set to `true`, the text is always rendered as a single line, regardless of new line separators in the text. (default value: `true`)
-   **`text-cursor-width`** (_in_ _length_): The width of the text cursor. (default value: provided at run-time by the selected widget style)
-   **`text`** (_in-out_ _string_): The text rendered and editable by the user.
-   **`vertical-alignment`** (_in_ _enum [`TextVerticalAlignment`](enums.md#textverticalalignment)_): The vertical alignment of the text.
-   **`wrap`** (_in_ _enum [`TextWrap`](enums.md#textwrap)_): The way the text input wraps. Only makes sense when `single-line` is false. (default value: no-wrap)

### Functions

-   **`focus()`** Call this function to focus the text input and make it receive future keyboard events.
-   **`clear-focus()`** Call this function to remove keyboard focus from this `TextInput` if it currently has the focus. See also [](../concepts/focus.md).
-   **`set-selection-offsets(int, int)`** Selects the text between two UTF-8 offsets.
-   **`select-all()`** Selects all text.
-   **`clear-selection()`** Clears the selection.
-   **`copy()`** Copies the selected text to the clipboard.
-   **`cut()`** Copies the selected text to the clipboard and removes it from the editable area.
-   **`paste()`** Pastes the text content of the clipboard at the cursor position.

### Callbacks

-   **`accepted()`**: Invoked when enter key is pressed.
-   **`cursor-position-changed(Point)`**: The cursor was moved to the new (x, y) position
    described by the [_`Point`_](structs.md#point) argument.
-   **`edited()`**: Invoked when the text has changed because the user modified it.

### Example

```slint
export component Example inherits Window {
    width: 270px;
    height: 100px;

    TextInput {
        text: "Replace me with a name";
    }
}
```

## `Text`

The `Text` element is responsible for rendering text. Besides the `text` property, that specifies which text to render,
it also allows configuring different visual aspects through the `font-family`, `font-size`, `font-weight`, `color`, and
`stroke` properties.

The `Text` element can break long text into multiple lines of text. A line feed character (`\n`) in the string of the `text`
property will trigger a manual line break. For automatic line breaking you need to set the `wrap` property to a value other than
`no-wrap`, and it's important to specify a `width` and `height` for the `Text` element, in order to know where to break. It's
recommended to place the `Text` element in a layout and let it set the `width` and `height` based on the available screen space
and the text itself.

### Properties

-   **`color`** (_in_ _brush_): The color of the text. (default value: depends on the style)
-   **`font-family`** (_in_ _string_): The name of the font family selected for rendering the text.
-   **`font-size`** (_in_ _length_): The font size of the text.
-   **`font-weight`** (_in_ _int_): The weight of the font. The values range from 100 (lightest) to 900 (thickest). 400 is the normal weight.
-   **`font-italic`** (_in_ _bool_): Whether or not the font face should be drawn italicized or not. (default value: false)
-   **`font-metrics`** (_out_ _struct [`FontMetrics`](structs.md#fontmetrics)_): The design metrics of the font scaled to the font pixel size used by the element.
-   **`horizontal-alignment`** (_in_ _enum [`TextHorizontalAlignment`](enums.md#texthorizontalalignment)_): The horizontal alignment of the text.
-   **`letter-spacing`** (_in_ _length_): The letter spacing allows changing the spacing between the glyphs. A positive value increases the spacing and a negative value decreases the distance. (default value: 0)
-   **`overflow`** (_in_ _enum [`TextOverflow`](enums.md#textoverflow)_): What happens when the text overflows (default value: clip).
-   **`text`** (_in_ _[string](../syntax/types.md#strings)_): The text rendered.
-   **`vertical-alignment`** (_in_ _enum [`TextVerticalAlignment`](enums.md#textverticalalignment)_): The vertical alignment of the text.
-   **`wrap`** (_in_ _enum [`TextWrap`](enums.md#textwrap)_): The way the text wraps (default value: `no-wrap`).
-   **`stroke`** (_in_ _brush_): The brush used for the text outline (default value: `transparent`).
-   **`stroke-width`** (_in_ _length_): The width of the text outline. If the width is zero, then a hairline stroke (1 physical pixel) will be rendered.
-   **`stroke-style`** (_in_ _enum [`TextStrokeStyle`](enums.md#textstrokestyle)_): The style/alignment of the text outline (default value: `outside`).
-   **`rotation-angle`** (_in_ _angle_), **`rotation-origin-x`** (_in_ _length_), **`rotation-origin-y`** (_in_ _length_):
    Rotates the text by the given angle around the specified origin point. The default origin point is the center of the element.
    When these properties are set, the `Text` can't have children.

### Example

This example shows the text "Hello World" in red, using the default font:

```slint
export component Example inherits Window {
    width: 270px;
    height: 100px;

    Text {
        x:0;y:0;
        text: "Hello World";
        color: red;
    }
}
```

This example breaks a longer paragraph of text into multiple lines, by setting a `wrap`
policy and assigning a limited `width` and enough `height` for the text to flow down:

```slint
export component Example inherits Window {
    width: 270px;
    height: 300px;

    Text {
        x:0;
        text: "This paragraph breaks into multiple lines of text";
        wrap: word-wrap;
        width: 150px;
        height: 100%;
    }
}
```

## `Timer`
<!-- FIXME: Timer is not really an element so it doesn't really belong in the `Builtin Elements` section. -->

Use the Timer pseudo-element to schedule a callback at a given interval.
The timer is only running when the `running` property is set to `true`. To stop or start the timer, set that property to `true` or `false`.
It can be also set to a binding expression.
When already running, the timer will be restarted if the `interval` property is changed.

:::{note}
The default value for `running` is `true`, so if you don't specify it, it will be running.
:::

:::{note}
Timer is not an actual element visible in the tree, therefore it doesn't have the common properties such as `x`, `y`, `width`, `height`, etc. It also doesn't take room in a layout and cannot have any children or be inherited from.
:::

### Properties

 -  **`interval`** (_in_ _duration_): The interval between timer ticks. This property is mandatory.
 -  **`running`** (_in_ _bool_): `true` if the timer is running. (default value: `true`)

### Callbacks

 -  **`triggered()`**: Invoked every time the timer ticks (every `interval`).

### Example

This example shows a timer that counts down from 10 to 0 every second:

```slint
import { Button } from "std-widgets.slint";
export component Example inherits Window {
    property <int> value: 10;
    timer := Timer {
        interval: 1s;
        running: true;
        triggered() => {
            value -= 1;
            if (value == 0) {
                self.running = false;
            }
        }
    }
    HorizontalLayout {
        Text { text: value; }
        Button {
            text: "Reset";
            clicked() => { value = 10; timer.running = true; }
        }
    }
}
```


## `TouchArea`

Use `TouchArea` to control what happens when the region it covers is touched or interacted with
using the mouse.

When not part of a layout, its width or height default to 100% of the parent element.

### Properties

-   **`has-hover`** (_out_ _bool_): `TouchArea` sets this to `true` when the mouse is over it.
-   **`mouse-cursor`** (_in_ _enum [`MouseCursor`](enums.md#mousecursor)_): The mouse cursor type when the mouse is hovering the `TouchArea`.
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
    The [_`PointerEvent`_](structs.md#pointerevent) argument contains information such which button was pressed
    and any active keyboard modifiers.
    In the [_`PointerEventKind::Move`_](structs.md#pointereventkind) case the `buttons` field will always
    be set to `PointerEventButton::Other`, independent of whether any button is pressed or not.
-   **`scroll-event(PointerScrollEvent) -> EventResult`**: Invoked when the mouse wheel was rotated or another scroll gesture was made.
    The [_`PointerScrollEvent`_](structs.md#pointerscrollevent) argument contains information about how much to scroll in what direction.
    The returned [`EventResult`](enums.md#eventresult) indicates whether to accept or ignore the event. Ignored events are
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

## `VerticalLayout` and `HorizontalLayout`

These layouts place their children next to each other vertically or horizontally.
The size of elements can either be fixed with the `width` or `height` property, or if they aren't set
they will be computed by the layout respecting the minimum and maximum sizes and the stretch factor.

### Properties

-   **`spacing`** (_in_ _length_): The distance between the elements in the layout.
-   **`padding`** (_in_ _length_): the padding within the layout.
-   **`padding-left`**, **`padding-right`**, **`padding-top`** and **`padding-bottom`** (_in_ _length_): Set these properties to override the padding on specific sides.
-   **`alignment`** (_in_ _enum [`LayoutAlignment`](enums.md#layoutalignment)_): Set the alignment. Matches the CSS flex box.

### Example

```slint
export component Foo inherits Window {
    width: 200px;
    height: 100px;
    HorizontalLayout {
        spacing: 5px;
        Rectangle { background: red; width: 10px; }
        Rectangle { background: blue; min-width: 10px; }
        Rectangle { background: yellow; horizontal-stretch: 1; }
        Rectangle { background: green; horizontal-stretch: 2; }
    }
}
```

## `Window`

`Window` is the root of the tree of elements that are visible on the screen.

The `Window` geometry will be restricted by its layout constraints: Setting the `width` will result in a fixed width,
and the window manager will respect the `min-width` and `max-width` so the window can't be resized bigger
or smaller. The initial width can be controlled with the `preferred-width` property. The same applies to the `Window`s height.

### Properties

-   **`always-on-top`** (_in_ _bool_): Whether the window should be placed above all other windows on window managers supporting it.
-   **`background`** (_in_ _brush_): The background brush of the `Window`. (default value: depends on the style)
-   **`default-font-family`** (_in_ _string_): The font family to use as default in text elements inside this window, that don't have their `font-family` property set.
-   **`default-font-size`** (_in-out_ _length_): The font size to use as default in text elements inside this window, that don't have their `font-size` property set. The value of this property also forms the basis for relative font sizes.
-   **`default-font-weight`** (_in_ _int_): The font weight to use as default in text elements inside this window, that don't have their `font-weight` property set. The values range from 100 (lightest) to 900 (thickest). 400 is the normal weight.
-   **`icon`** (_in_ _image_): The window icon shown in the title bar or the task bar on window managers supporting it.
-   **`no-frame`** (_in_ _bool_): Whether the window should be borderless/frameless or not.
-   **`resize-border-width`** (_in_ _length_): Size of the resize border in borderless/frameless windows (winit only for now).
-   **`title`** (_in_ _string_): The window title that is shown in the title bar.
