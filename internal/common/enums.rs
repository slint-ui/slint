// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! This module contains all enums exposed in the .slint language.

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
// NOTE: the documentation for .slint enum is in builtin_elements.md
#[macro_export]
macro_rules! for_each_enums {
    ($macro:ident) => {
        $macro![
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
            enum DialogButtonRole {
                none,
                accept,
                reject,
                apply,
                reset,
                action,
                help,
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

            enum MouseCursor {
                default,
                none,
                //context_menu,
                help,
                pointer,
                progress,
                wait,
                //cell,
                crosshair,
                text,
                //vertical_text,
                alias,
                copy,
                r#move,
                no_drop,
                not_allowed,
                grab,
                grabbing,
                //all_scroll,
                col_resize,
                row_resize,
                n_resize,
                e_resize,
                s_resize,
                w_resize,
                ne_resize,
                nw_resize,
                se_resize,
                sw_resize,
                ew_resize,
                ns_resize,
                nesw_resize,
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

            enum FillRule {
                nonzero,
                evenodd,
            }

            /// This enum defines the input type in a text input which for now only distinguishes a normal
            /// input from a password input
            enum InputType {
                /// This type is used for a normal text input
                text,
                /// This type is used for password inputs where the characters are represented as *'s
                password,
            }
            enum TextHorizontalAlignment {
                left,
                center,
                right,
            }
            enum TextVerticalAlignment {
                top,
                center,
                bottom,
            }
            enum TextWrap {
                no_wrap,
                word_wrap,
            }
            enum TextOverflow {
                clip,
                elide,
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

            /// What is returned from the event handler
            enum EventResult {
                reject,
                accept,
            }

            /// This enum defines the different kinds of key events that can happen.
            enum KeyEventType {
                /// A key on a keyboard was pressed.
                KeyPressed,
                /// A key on a keyboard was released.
                KeyReleased,
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
