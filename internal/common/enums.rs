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
                left,
                /// The text will be horizontally centered within the containing box.
                center,
                /// The text will be aligned to the right of the containing box.
                right,
            }

            /// This enum describes the different types of alignment of text along the vertical axis.
            enum TextVerticalAlignment {
                /// The text will be aligned to the top of the containing box.
                top,
                /// The text will be vertically centered within the containing box.
                center,
                /// The text will be alignt to the bottom of the containing box.
                bottom,
            }

            /// This enum describes the how the text wrap if it is too wide to fit in the Text width.
            enum TextWrap {
                /// The text will not wrap, but instead will overflow.
                no_wrap,
                /// The text will be wrapped at word boundaries.
                word_wrap,
            }

            /// This enum describes the how the text appear if it is too wide to fit in the Text width.
            enum TextOverflow {
                /// The text will simply be clipped.
                clip,
                /// The text will be elided with `…`.
                elide,
            }

            /// This enum describes whether an event was rejected or accepted by an event handler.
            enum EventResult {
                /// The event is rejected by this event handler and may then be handled by the parent item
                reject,
                /// The event is accepted and won't be processed further
                accept,
            }

            /// This enum describes the different ways of deciding what the inside of a shape described by a path shall be.
            enum FillRule {
                /// The ["nonzero" fill rule as defined in SVG](https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/fill-rule#nonzero).
                nonzero,
                /// The ["evenodd" fill rule as defined in SVG](https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/fill-rule#evenodd)
                evenodd,
            }

            enum StandardButtonKind {
                ok,
                cancel,
                apply,
                close,
                reset,
                help,
                yes,
                no,
                abort,
                retry,
                ignore,
            }

            /// This enum represents the value of the `dialog-button-role` property which can be added to
            /// any element within a `Dialog` to put that item in the button row, and its exact position
            /// depends on the role and the platform.
            enum DialogButtonRole {
                /// This is not a button means to go in the row of button of the dialog
                none,
                /// This is the role of the main button to click to accept the dialog. e.g. "Ok" or "Yes"
                accept,
                /// This is the role of the main button to click to reject the dialog. e.g. "Cancel" or "No"
                reject,
                /// This is the role of the "Apply" button
                apply,
                /// This is the role of the "Reset" button
                reset,
                /// This is the role of the  "Help" button
                help,
                /// This is the role of any other button that performs another action.
                action,
            }

            enum PointerEventKind {
                cancel,
                down,
                up,
            }

            enum PointerEventButton {
                none,
                left,
                right,
                middle,
            }

            /// This enum represents different types of mouse cursors. It is a subset of the mouse cursors available in CSS.
            /// For details and pictograms see the [MDN Documentation for cursor](https://developer.mozilla.org/en-US/docs/Web/CSS/cursor#values).
            /// Depending on the backend and used OS unidirectional resize cursors may be replaced with bidirectional ones.
            enum MouseCursor {
                /// The systems default cursor.
                default,
                /// No cursor is displayed.
                none,
                //context_menu,
                /// A cursor indicating help information.
                help,
                /// A pointing hand indicating a link.
                pointer,
                /// The program is busy but can still be interacted with.
                progress,
                /// The program is busy.
                wait,
                //cell,
                /// A crosshair.
                crosshair,
                /// A cursor indicating selectable text.
                text,
                //vertical_text,
                /// An alias or shortcut is being created.
                alias,
                /// A copy is being created.
                copy,
                /// Something is to be moved.
                r#move,
                /// Something cannot be dropped here.
                no_drop,
                /// An action is not allowed
                not_allowed,
                /// Something is grabbable.
                grab,
                /// Something is being grabbed.
                grabbing,
                //all_scroll,
                /// Indicating that a column is resizable horizontally.
                col_resize,
                /// Indicating that a row is resizable vertically.
                row_resize,
                /// Unidirectional resize north.
                n_resize,
                /// Unidirectional resize east.
                e_resize,
                /// Unidirectional resize south.
                s_resize,
                /// Unidirectional resize west.
                w_resize,
                /// Unidirectional resize north-east.
                ne_resize,
                /// Unidirectional resize north-west.
                nw_resize,
                /// Unidirectional resize south-east.
                se_resize,
                /// Unidirectional resize south-west.
                sw_resize,
                /// Bidirectional resize east-west.
                ew_resize,
                /// Bidirectional resize north-south.
                ns_resize,
                /// Bidirectional resize north-east-south-west.
                nesw_resize,
                /// Bidirectional resize north-west-south-east.
                nwse_resize,
                //zoom_in,
                //zoom_out,
            }

            enum ImageFit {
                fill,
                contain,
                cover,
            }

            enum ImageRendering {
                smooth,
                pixelated,
            }

            /// This enum is used to define the type of the input field. Currently this only differentiates between
            /// text and password inputs but in the future it could be expanded to also define what type of virtual keyboard
            /// should be shown, for example.
            enum InputType {
                /// The default value. This will render all characters normally
                text,
                /// This will render all characters with a character that defaults to "*"
                password,
            }

            /// Enum representing the alignment property of a BoxLayout or HorizontalLayout
            enum LayoutAlignment {
                stretch,
                center,
                start,
                end,
                space_between,
                space_around,
            }

            /// PathEvent is a low-level data structure describing the composition of a path. Typically it is
            /// generated at compile time from a higher-level description, such as SVG commands.
            enum PathEvent {
                /// The beginning of the path.
                begin,
                /// A straight line on the path.
                line,
                /// A quadratic bezier curve on the path.
                quadratic,
                /// A cubic bezier curve on the path.
                cubic,
                /// The end of the path that remains open.
                end_open,
                /// The end of a path that is closed.
                end_closed,
            }

            /// This enum defines the different kinds of key events that can happen.
            enum KeyEventType {
                /// A key on a keyboard was pressed.
                KeyPressed,
                /// A key on a keyboard was released.
                KeyReleased,
            }

            /// This enum represents the different values for the `accessible-role` property, used to describe the
            /// role of an element in the context of assistive technology such as screen readers.
            enum AccessibleRole {
                /// The element is not accessible.
                none,
                /// The element is a Button or behaves like one.
                button,
                /// The element is a CheckBox or behaves like one.
                checkbox,
                /// The element is a ComboBox or behaves like one.
                combobox,
                /// The element is a Slider or behaves like one.
                slider,
                /// The element is a SpinBox or behaves like one.
                spinbox,
                /// A role for anything that is a Tab or behaves like one.
                tab,
                /// A role for static Text items.
                text,
            }
        ];
    };
}

/// add an underscore to a C++ keyword used as an enum
pub fn cpp_escape_keyword(kw: &str) -> &str {
    match kw {
        "default" => "default_",
        other => other,
    }
}
