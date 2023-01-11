<!-- Generated with `cargo xtask enumdocs` from internal/commons/enums.rs -->
# Builtin Enums

Enum value can be referenced by using the name of the enum and the name of the value
separated by a dot. (eg: `TextHorizontalAlignment.left`)

The name of the enum can be omitted in bindings of the type of that enum, or if the
return value of a callback is of that enum.

The default value of each enum type is always the first value.
## `TextHorizontalAlignment`

 This enum describes the different types of alignment of text along the horizontal axis.

* **`left`**: The text will be aligned with the left edge of the containing box.
* **`center`**: The text will be horizontally centered within the containing box.
* **`right`**: The text will be aligned to the right of the containing box.

## `TextVerticalAlignment`

 This enum describes the different types of alignment of text along the vertical axis.

* **`top`**: The text will be aligned to the top of the containing box.
* **`center`**: The text will be vertically centered within the containing box.
* **`bottom`**: The text will be aligned to the bottom of the containing box.

## `TextWrap`

 This enum describes the how the text wrap if it is too wide to fit in the Text width.

* **`no-wrap`**: The text will not wrap, but instead will overflow.
* **`word-wrap`**: The text will be wrapped at word boundaries.

## `TextOverflow`

 This enum describes the how the text appear if it is too wide to fit in the Text width.

* **`clip`**: The text will simply be clipped.
* **`elide`**: The text will be elided with `…`.

## `EventResult`

 This enum describes whether an event was rejected or accepted by an event handler.

* **`reject`**: The event is rejected by this event handler and may then be handled by the parent item
* **`accept`**: The event is accepted and won't be processed further

## `FillRule`

 This enum describes the different ways of deciding what the inside of a shape described by a path shall be.

* **`nonzero`**: The ["nonzero" fill rule as defined in SVG](https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/fill-rule#nonzero).
* **`evenodd`**: The ["evenodd" fill rule as defined in SVG](https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/fill-rule#evenodd)

## `StandardButtonKind`


* **`ok`**:
* **`cancel`**:
* **`apply`**:
* **`close`**:
* **`reset`**:
* **`help`**:
* **`yes`**:
* **`no`**:
* **`abort`**:
* **`retry`**:
* **`ignore`**:

## `DialogButtonRole`

 This enum represents the value of the `dialog-button-role` property which can be added to
 any element within a `Dialog` to put that item in the button row, and its exact position
 depends on the role and the platform.

* **`none`**: This is not a button means to go in the row of button of the dialog
* **`accept`**: This is the role of the main button to click to accept the dialog. e.g. "Ok" or "Yes"
* **`reject`**: This is the role of the main button to click to reject the dialog. e.g. "Cancel" or "No"
* **`apply`**: This is the role of the "Apply" button
* **`reset`**: This is the role of the "Reset" button
* **`help`**: This is the role of the  "Help" button
* **`action`**: This is the role of any other button that performs another action.

## `PointerEventKind`


* **`cancel`**:
* **`down`**:
* **`up`**:

## `PointerEventButton`

 This enum describes the different types of buttons for a pointer event,
 typically on a mouse or a pencil.

* **`none`**: A button that is none of left, right or middle. For example
    this is used for a fourth button on a mouse with many buttons.
* **`left`**: The left button.
* **`right`**: The right button.
* **`middle`**: The center button.

## `MouseCursor`

 This enum represents different types of mouse cursors. It is a subset of the mouse cursors available in CSS.
 For details and pictograms see the [MDN Documentation for cursor](https://developer.mozilla.org/en-US/docs/Web/CSS/cursor#values).
 Depending on the backend and used OS unidirectional resize cursors may be replaced with bidirectional ones.

* **`default`**: The systems default cursor.
* **`none`**: No cursor is displayed.
* **`help`**: A cursor indicating help information.
* **`pointer`**: A pointing hand indicating a link.
* **`progress`**: The program is busy but can still be interacted with.
* **`wait`**: The program is busy.
* **`crosshair`**: A crosshair.
* **`text`**: A cursor indicating selectable text.
* **`alias`**: An alias or shortcut is being created.
* **`copy`**: A copy is being created.
* **`move`**: Something is to be moved.
* **`no-drop`**: Something cannot be dropped here.
* **`not-allowed`**: An action is not allowed
* **`grab`**: Something is grabbable.
* **`grabbing`**: Something is being grabbed.
* **`col-resize`**: Indicating that a column is resizable horizontally.
* **`row-resize`**: Indicating that a row is resizable vertically.
* **`n-resize`**: Unidirectional resize north.
* **`e-resize`**: Unidirectional resize east.
* **`s-resize`**: Unidirectional resize south.
* **`w-resize`**: Unidirectional resize west.
* **`ne-resize`**: Unidirectional resize north-east.
* **`nw-resize`**: Unidirectional resize north-west.
* **`se-resize`**: Unidirectional resize south-east.
* **`sw-resize`**: Unidirectional resize south-west.
* **`ew-resize`**: Bidirectional resize east-west.
* **`ns-resize`**: Bidirectional resize north-south.
* **`nesw-resize`**: Bidirectional resize north-east-south-west.
* **`nwse-resize`**: Bidirectional resize north-west-south-east.

## `ImageFit`


* **`fill`**:
* **`contain`**:
* **`cover`**:

## `ImageRendering`


* **`smooth`**:
* **`pixelated`**:

## `InputType`

 This enum is used to define the type of the input field. Currently this only differentiates between
 text and password inputs but in the future it could be expanded to also define what type of virtual keyboard
 should be shown, for example.

* **`text`**: The default value. This will render all characters normally
* **`password`**: This will render all characters with a character that defaults to "*"

## `LayoutAlignment`

 Enum representing the alignment property of a BoxLayout or HorizontalLayout

* **`stretch`**:
* **`center`**:
* **`start`**:
* **`end`**:
* **`space-between`**:
* **`space-around`**:

## `PathEvent`

 PathEvent is a low-level data structure describing the composition of a path. Typically it is
 generated at compile time from a higher-level description, such as SVG commands.

* **`begin`**: The beginning of the path.
* **`line`**: A straight line on the path.
* **`quadratic`**: A quadratic bezier curve on the path.
* **`cubic`**: A cubic bezier curve on the path.
* **`end-open`**: The end of the path that remains open.
* **`end-closed`**: The end of a path that is closed.

## `AccessibleRole`

 This enum represents the different values for the `accessible-role` property, used to describe the
 role of an element in the context of assistive technology such as screen readers.

* **`none`**: The element is not accessible.
* **`button`**: The element is a Button or behaves like one.
* **`checkbox`**: The element is a CheckBox or behaves like one.
* **`combobox`**: The element is a ComboBox or behaves like one.
* **`slider`**: The element is a Slider or behaves like one.
* **`spinbox`**: The element is a SpinBox or behaves like one.
* **`tab`**: The element is a Tab or behaves like one.
* **`text`**: The role for a Text element. It is automatically applied.

## `SortOrder`

 This enum represents the different values of the `sort-order` property.
 It is used to sort a `StandardTableView` by a column.

* **`unsorted`**: The column is unsorted.
* **`ascending`**: The column is sorted in ascending order.
* **`descending`**: The column is sorted in descending order.

