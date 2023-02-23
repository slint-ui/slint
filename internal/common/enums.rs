// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! This module contains all enums exposed in the .slint language.

// NOTE: when changing the documentation of enums, you need to update
// the markdown file with `cargo xtask enumdocs`

/// Call a macro with every enum exposed in the .slint language
///
/// ## Example
/// ```rust
/// macro_rules! print_enums {
///     ($( $(#[$enum_doc:meta])* enum $Name:ident { $( $(#[$value_doc:meta])* $Value:ident,)* })*) => {
///         $(println!("{} => [{}]", stringify!($Name), stringify!($($Value),*));)*
///     }
/// }
/// i_slint_common::for_each_enums!(print_enums);
/// ```
#[macro_export]
macro_rules! for_each_enums {
    ($macro:ident) => {
        $macro![
            /// This enum describes the different types of alignment of text along the horizontal axis.
            enum TextHorizontalAlignment {
                /// The text will be aligned with the left edge of the containing box.
                Left,
                /// The text will be horizontally centered within the containing box.
                Center,
                /// The text will be aligned to the right of the containing box.
                Right,
            }

            /// This enum describes the different types of alignment of text along the vertical axis.
            enum TextVerticalAlignment {
                /// The text will be aligned to the top of the containing box.
                Top,
                /// The text will be vertically centered within the containing box.
                Center,
                /// The text will be aligned to the bottom of the containing box.
                Bottom,
            }

            /// This enum describes the how the text wrap if it is too wide to fit in the Text width.
            enum TextWrap {
                /// The text will not wrap, but instead will overflow.
                NoWrap,
                /// The text will be wrapped at word boundaries.
                WordWrap,
            }

            /// This enum describes the how the text appear if it is too wide to fit in the Text width.
            enum TextOverflow {
                /// The text will simply be clipped.
                Clip,
                /// The text will be elided with `…`.
                Elide,
            }

            /// This enum describes whether an event was rejected or accepted by an event handler.
            enum EventResult {
                /// The event is rejected by this event handler and may then be handled by the parent item
                Reject,
                /// The event is accepted and won't be processed further
                Accept,
            }

            /// This enum describes the different ways of deciding what the inside of a shape described by a path shall be.
            enum FillRule {
                /// The ["nonzero" fill rule as defined in SVG](https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/fill-rule#nonzero).
                Nonzero,
                /// The ["evenodd" fill rule as defined in SVG](https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/fill-rule#evenodd)
                Evenodd,
            }

            /// This enum is used to add standard buttons to a `Dialog`. The look and positioning
            /// of these `StandardButton`s depends on the environment (OS, UI environment, etc.)
            /// the application runs in.
            enum StandardButtonKind {
                /// A "OK" button that accepts a `Dialog`, closing it
                Ok,
                /// A "Cancel" button that rejects a `Dialog`, closing it
                Cancel,
                /// A "Apply" button that should accept values from a `Dialog` without closing it.
                Apply,
                /// A "Close" button which should close a `Dialog` without looking at values
                Close,
                /// A "Reset" button which should reset the `Dialog` to its start state
                Reset,
                /// A "Help" button
                Help,
                /// A "Yes" button, used to confirm an action
                Yes,
                /// A "No" button, used to negate an action
                No,
                /// A "Abort" button, used to abort an action
                Abort,
                /// A "Retry" button, used to retry a failed action
                Retry,
                /// A "Ignore" button, used to ignore a failed action
                Ignore,
            }

            /// This enum represents the value of the `dialog-button-role` property which can be added to
            /// any element within a `Dialog` to put that item in the button row, and its exact position
            /// depends on the role and the platform.
            enum DialogButtonRole {
                /// This is not a button meant to go into the bottom row
                None,
                /// This is the role of the main button to click to accept the dialog. e.g. "Ok" or "Yes"
                Accept,
                /// This is the role of the main button to click to reject the dialog. e.g. "Cancel" or "No"
                Reject,
                /// This is the role of the "Apply" button
                Apply,
                /// This is the role of the "Reset" button
                Reset,
                /// This is the role of the  "Help" button
                Help,
                /// This is the role of any other button that performs another action.
                Action,
            }

            /// The enum reports what happened to the `PointerEventButton` in the event
            enum PointerEventKind {
                /// The action was cancelled
                Cancel,
                /// The button was pressed
                Down,
                /// The button was released
                Up,
            }

            /// This enum describes the different types of buttons for a pointer event,
            /// typically on a mouse or a pencil.
            #[non_exhaustive]
            enum PointerEventButton {
                /// A button that is none of left, right or middle. For example
                /// this is used for a fourth button on a mouse with many buttons.
                Other,
                /// The left button.
                Left,
                /// The right button.
                Right,
                /// The center button.
                Middle,
            }

            /// This enum represents different types of mouse cursors. It is a subset of the mouse cursors available in CSS.
            /// For details and pictograms see the [MDN Documentation for cursor](https://developer.mozilla.org/en-US/docs/Web/CSS/cursor#values).
            /// Depending on the backend and used OS unidirectional resize cursors may be replaced with bidirectional ones.
            enum MouseCursor {
                /// The systems default cursor.
                Default,
                /// No cursor is displayed.
                None,
                //context_menu,
                /// A cursor indicating help information.
                Help,
                /// A pointing hand indicating a link.
                Pointer,
                /// The program is busy but can still be interacted with.
                Progress,
                /// The program is busy.
                Wait,
                //cell,
                /// A crosshair.
                Crosshair,
                /// A cursor indicating selectable text.
                Text,
                //vertical_text,
                /// An alias or shortcut is being created.
                Alias,
                /// A copy is being created.
                Copy,
                /// Something is to be moved.
                Move,
                /// Something cannot be dropped here.
                NoDrop,
                /// An action is not allowed
                NotAllowed,
                /// Something is grabbable.
                Grab,
                /// Something is being grabbed.
                Grabbing,
                //all_scroll,
                /// Indicating that a column is resizable horizontally.
                ColResize,
                /// Indicating that a row is resizable vertically.
                RowResize,
                /// Unidirectional resize north.
                NResize,
                /// Unidirectional resize east.
                EResize,
                /// Unidirectional resize south.
                SResize,
                /// Unidirectional resize west.
                WResize,
                /// Unidirectional resize north-east.
                NeResize,
                /// Unidirectional resize north-west.
                NwResize,
                /// Unidirectional resize south-east.
                SeResize,
                /// Unidirectional resize south-west.
                SwResize,
                /// Bidirectional resize east-west.
                EwResize,
                /// Bidirectional resize north-south.
                NsResize,
                /// Bidirectional resize north-east-south-west.
                NeswResize,
                /// Bidirectional resize north-west-south-east.
                NwseResize,
                //zoom_in,
                //zoom_out,
            }

            /// This enum defines how the source image shall fit into the image element.
            enum ImageFit {
                /// Scales and stretches the image to fit the width and height of the element.
                Fill,
                /// The source image is scaled to fit into the image element's dimension while preserving the aspect ratio.
                Contain,
                /// The source image is scaled to cover into the image element's dimension while preserving the aspect ratio.
                Cover,
            }

            /// This enum specifies how the source image will be scaled.
            enum ImageRendering {
                /// The image is scaled with a linear interpolation algorithm.
                Smooth,
                /// The image is scaled with the nearest neighbor algorithm.
                Pixelated,
            }

            /// This enum is used to define the type of the input field. Currently this only differentiates between
            /// text and password inputs but in the future it could be expanded to also define what type of virtual keyboard
            /// should be shown, for example.
            enum InputType {
                /// The default value. This will render all characters normally
                Text,
                /// This will render all characters with a character that defaults to "*"
                Password,
            }

            /// Enum representing the alignment property of a BoxLayout or HorizontalLayout.
            /// See [alignment](concepts/layouting.md#alignment)
            enum LayoutAlignment {
                /// Use the minimum size of all elements in a layout, distribute remaining space
                /// based on `*-stretch` among all elements.
                Stretch,
                /// Use minimum size for all elements, distribute remaining space evenly before the
                /// first and after the last element
                Center,
                /// Use minimum size for all elements, put remaining space after the last element
                Start,
                /// Use minimum size for all elements, put remaining space before the first
                /// element
                End,
                /// Use minimum size for all elements, distribute remaining space evenly between
                /// elements.
                SpaceBetween,
                /// Use minimum size for all elements, distribute remaining space evenly before the
                /// first element, after the last element and between elements
                SpaceAround,
            }

            /// PathEvent is a low-level data structure describing the composition of a path. Typically it is
            /// generated at compile time from a higher-level description, such as SVG commands.
            enum PathEvent {
                /// The beginning of the path.
                Begin,
                /// A straight line on the path.
                Line,
                /// A quadratic bezier curve on the path.
                Quadratic,
                /// A cubic bezier curve on the path.
                Cubic,
                /// The end of the path that remains open.
                EndOpen,
                /// The end of a path that is closed.
                EndClosed,
            }

            /// This enum represents the different values for the `accessible-role` property, used to describe the
            /// role of an element in the context of assistive technology such as screen readers.
            enum AccessibleRole {
                /// The element is not accessible.
                None,
                /// The element is a Button or behaves like one.
                Button,
                /// The element is a CheckBox or behaves like one.
                Checkbox,
                /// The element is a ComboBox or behaves like one.
                Combobox,
                /// The element is a Slider or behaves like one.
                Slider,
                /// The element is a SpinBox or behaves like one.
                Spinbox,
                /// The element is a Tab or behaves like one.
                Tab,
                /// The role for a Text element. It is automatically applied.
                Text,
            }

            /// This enum represents the different values of the `sort-order` property.
            /// It is used to sort a `StandardTableView` by a column.
            enum SortOrder {
                /// The column is unsorted.
                Unsorted,

                /// The column is sorted in ascending order.
                Ascending,

                /// The column is sorted in descending order.
                Descending,
            }
        ];
    };
}
