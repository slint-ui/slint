// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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
            /// The `KeyboardModifiers` struct provides booleans to indicate possible modifier keys on a keyboard, such as Shift, Control, etc.
            /// It is provided as part of `KeyEvent`'s `modifiers` field.
            ///
            /// Keyboard shortcuts on Apple platforms typically use the Command key (⌘), such as Command+C for "Copy". On other platforms
            /// the same shortcut is typically represented using Control+C. To make it easier to develop cross-platform applications, on macOS,
            /// Slint maps the Command key to the control modifier, and the Control key to the meta modifier.
            ///
            /// On Windows, the Windows key is mapped to the meta modifier.
            #[derive(Copy, Eq)]
            struct KeyboardModifiers {
                @name = "slint::private_api::KeyboardModifiers"
                export {
                    /// Indicates the Alt key on a keyboard.
                    alt: bool,
                    /// Indicates the Control key on a keyboard, except on macOS, where it is the Command key (⌘).
                    control: bool,
                    /// Indicates the Shift key on a keyboard.
                    shift: bool,
                    /// Indicates the Control key on macos, and the Windows key on Windows.
                    meta: bool,
                }
                private {
                }
            }

            /// Represents a Pointer event sent by the windowing system.
            /// This structure is passed to the `pointer-event` callback of the `TouchArea` element.
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

            /// Represents a Pointer scroll (or wheel) event sent by the windowing system.
            /// This structure is passed to the `scroll-event` callback of the `TouchArea` element.
            struct PointerScrollEvent {
                @name = "slint::private_api::PointerScrollEvent"
                export {
                    /// The amount of pixel in the horizontal direction
                    delta_x: Coord,
                    /// The amount of pixel in the vertical direction
                    delta_y: Coord,
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
                    /// This field is set to true for key press events that are repeated,
                    /// i.e. the key is held down. It's always false for key release events.
                    repeat: bool,
                }
                private {
                    /// Indicates whether the key was pressed or released
                    event_type: KeyEventType,
                    /// If the event type is KeyEventType::UpdateComposition or KeyEventType::CommitComposition,
                    /// then this field specifies what part of the current text to replace.
                    /// Relative to the offset of the pre-edit text within the text input element's text.
                    replacement_range: Option<core::ops::Range<i32>>,
                    /// If the event type is KeyEventType::UpdateComposition, this is the new pre-edit text
                    preedit_text: SharedString,
                    /// The selection within the preedit_text
                    preedit_selection: Option<core::ops::Range<i32>>,
                    /// The new cursor position, when None, the cursor is put after the text that was just inserted
                    cursor_position: Option<i32>,
                    anchor_position: Option<i32>,
                }
            }

            /// Represents an item in a StandardListView and a StandardTableView.
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
                @name = "slint::private_api::TableColumn"
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

            /// A structure to hold metrics of a font for a specified pixel size.
            struct FontMetrics {
                @name = "slint::private_api::FontMetrics"
                export {
                    /// The distance between the baseline and the top of the tallest glyph in the font.
                    ascent: Coord,
                    /// The distance between the baseline and the bottom of the tallest glyph in the font.
                    /// This is usually negative.
                    descent: Coord,
                    /// The distance between the baseline and the horizontal midpoint of the tallest glyph in the font,
                    /// or zero if not specified by the font.
                    x_height: Coord,
                    /// The distance between the baseline and the top of a regular upper-case glyph in the font,
                    /// or zero if not specified by the font.
                    cap_height: Coord,
                }
                private {
                }
            }

            /// An item in the menu of a menu bar or context menu
            struct MenuEntry {
                @name = "slint::private_api::MenuEntry"
                export {
                    /// The text of the menu entry
                    title: SharedString,
                    // /// the icon associated with the menu entry
                    // icon: Image,
                    /// an opaque id that can be used to identify the menu entry
                    id: SharedString,
                    // keyboard_shortcut: KeySequence,
                    // /// whether the menu entry is enabled
                    // enabled: bool,
                    /// Sub menu
                    has_sub_menu: bool,
                    /// The menu entry is a separator
                    is_separator: bool,
                }
                private {}
            }
        ];
    };
}
