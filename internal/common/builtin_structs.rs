// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

//! This module contains all builtin structures exposed in the .slint language.

/// Call a macro with every builtin structures exposed in the .slint language
///
/// ## Example
/// ```rust
/// macro_rules! print_builtin_structs {
///     ($(
///         $(#[$struct_attr:meta])*
///         struct $Name:ident {
///             @name = $inner_name:literal
///             export {
///                 $( $(#[$pub_attr:meta])* $pub_field:ident : $pub_type:ty, )*
///             }
///             private {
///                 $( $(#[$pri_attr:meta])* $pri_field:ident : $pri_type:ty, )*
///             }
///         }
///     )*) => {
///         $(println!("{} => export:[{}] private:[{}]", stringify!($Name), stringify!($($pub_field),*), stringify!($($pri_field),*));)*
///     };
/// }
/// i_slint_common::for_each_builtin_structs!(print_builtin_structs);
/// ```
#[macro_export]
macro_rules! for_each_builtin_structs {
    ($macro:ident) => {
        $macro![
            /// KeyboardModifier provides booleans to indicate possible modifier keys on a keyboard, such as Shift, Control, etc.
            /// This structure is generated as part of `KeyEvent`
            /// On macOS, the command key is mapped to the meta modifier.
            /// On Windows, the windows key is mapped to the meta modifier.
            #[derive(Copy, Eq)]
            struct KeyboardModifiers {
                @name = "slint::private_api::KeyboardModifiers"
                export {
                    /// Indicates the alt key on a keyboard.
                    alt: bool,
                    /// Indicates the control key on a keyboard.
                    control: bool,
                    /// Indicates the shift key on a keyboard.
                    shift: bool,
                    /// Indicates the command key on macos.
                    meta: bool,
                }
                private {
                }
            }

            /// Represents a Pointer event sent by the windowing system.
            /// This structure is generated and passed to the `pointer-event` callback of the `TouchArea` element.
            struct PointerEvent {
                @name = "slint::private_api::PointerEvent"
                export {
                    /// The button that was pressed or released
                    button: PointerEventButton,
                    /// The kind of the event
                    kind: PointerEventKind,
                    /// The keyboard modifiers pressed during the event
                    modifiers: KeyboardModifiers,
                }
                private {
                }
            }

            /// This structure is generated and passed to the key press and release callbacks of the `FocusScope` element.
            struct KeyEvent {
                @name = "slint::private_api::KeyEvent"
                export {
                    /// The unicode representation of the key pressed.
                    text: SharedString,
                    /// The keyboard modifiers active at the time of the key press event.
                    modifiers: KeyboardModifiers,
                }
                private {
                    /// Indicates whether the key was pressed or released
                    event_type: KeyEventType,
                    /// If the event type is KeyEventType::UpdateComposition, then this field specifies
                    /// the start of the selection as byte offsets within the preedit text.
                    preedit_selection_start: usize,
                    /// If the event type is KeyEventType::UpdateComposition, then this field specifies
                    /// the end of the selection as byte offsets within the preedit text.
                    preedit_selection_end: usize,
                }
            }

            /// Represents an item in a StandardListView and a StandardTableView. This is the Rust/C++ type for
            /// the StandardListViewItem type in Slint files, when declaring for example a `property <[StandardListViewItem]> my-list-view-model;`.
            #[non_exhaustive]
            struct StandardListViewItem {
                @name = "slint::StandardListViewItem"
                export {
                    /// The text content of the item
                    text: SharedString,
                }
                private {
                }
            }

            /// This is used to define the column and the column header of a TableView
            #[non_exhaustive]
            struct TableColumn {
                @name = "TableColumn"
                export {
                    /// The title of the column header
                    title: SharedString,
                    /// The minimum column width (logical length)
                    min_width: Coord,
                    /// The horizontal column stretch
                    horizontal_stretch: f32,
                    /// Sorts the column
                    sort_order: SortOrder,
                    /// the actual width of the column (logical length)
                    width: Coord,
                }
                private {
                }
            }

            /// Value of the state property
            /// A state is just the current state, but also has information about the previous state and the moment it changed
            struct StateInfo {
                @name = "slint::private_api::StateInfo"
                export {
                    /// The current state value
                    current_state: i32,
                    /// The previous state
                    previous_state: i32,
                }
                private {
                    /// The instant in which the state changed last
                    change_time: crate::animations::Instant,
                }
            }
        ];
    };
}
