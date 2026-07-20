// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains all builtin structures exposed in the .slint language.

/// Call a macro with every builtin structures exposed in the .slint language
///
/// Each struct is declared with `pub struct` if it should be re-exported in a public
/// language-binding module (e.g. `slint::language` in the Rust crate), or plain `struct`
/// to stay private. Consumers can dispatch on `$vis:vis`.
///
/// ## Example
/// ```rust
/// macro_rules! print_builtin_structs {
///     ($(
///         $(#[$struct_attr:meta])*
///         $vis:vis struct $Name:ident {
///             $( $(#[$field_attr:meta])* $field:ident : $field_type:ty, )*
///         }
///     )*) => {
///         $(println!("{} ({}) => [{}]", stringify!($Name), stringify!($vis), stringify!($($field),*));)*
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
            #[non_exhaustive]
            #[derive(Copy, Eq)]
            pub struct KeyboardModifiers {
                /// Indicates the Alt key on a keyboard.
                alt: bool,
                /// Indicates the Control key on a keyboard, except on macOS, where it is the Command key (⌘).
                control: bool,
                /// Indicates the Shift key on a keyboard.
                shift: bool,
                /// Indicates the Control key on macos, and the Windows key on Windows.
                meta: bool,
            }

            /// Represents a Pointer event sent by the windowing system.
            /// This structure is passed to the `pointer-event` callback of the `TouchArea` element.
            #[non_exhaustive]
            pub struct PointerEvent {
                /// The button that was pressed or released
                button: PointerEventButton,
                /// The kind of the event
                kind: PointerEventKind,
                /// The keyboard modifiers pressed during the event
                modifiers: KeyboardModifiers,
                /// The unique ID of the touch point, indicating the finger ID. 0 means it's not a touch event (e.g., mouse).
                touch_finger_id: i32,
            }

            /// Represents a Pointer scroll (or wheel) event sent by the windowing system.
            /// This structure is passed to the `scroll-event` callback of the `TouchArea` element.
            #[non_exhaustive]
            pub struct PointerScrollEvent {
                /// The amount of pixel in the horizontal direction
                delta_x: Coord,
                /// The amount of pixel in the vertical direction
                delta_y: Coord,
                /// The keyboard modifiers pressed during the event
                modifiers: KeyboardModifiers,
            }

            /// This structure is generated and passed to the key press and release callbacks of the `FocusScope` element.
            #[non_exhaustive]
            pub struct KeyEvent {
                /// The unicode representation of the key pressed.
                text: SharedString,
                /// The keyboard modifiers active at the time of the key press event.
                modifiers: KeyboardModifiers,
                /// This field is set to true for key press events that are repeated,
                /// i.e. the key is held down. It's always false for key release events.
                repeat: bool,
            }

            /// This structure is passed to the callbacks of the `DropArea` element
            #[non_exhaustive]
            pub struct DropEvent {
                /// The payload set on the source `DragArea`.
                data: DataTransfer,

                /// The cursor position in the `DropArea`'s local coordinates.
                position: LogicalPosition,

                /// The action negotiated from current modifier state, clamped to the allowed set;
                /// when no modifier is pressed, the first allowed of move, copy, link.
                /// Updated on every `DragMove`. The target's `can-drop` callback can return this
                /// to honor the user's modifier choice, or override with any other allowed action.
                proposed_action: DragAction,
            }

            /// Represents an item in a StandardListView and a StandardTableView.
            #[non_exhaustive]
            pub struct StandardListViewItem {
                /// The text content of the item
                text: SharedString,
            }

            /// This is used to define the column and the column header of a TableView
            #[non_exhaustive]
            pub struct TableColumn {
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

            /// A structure to hold metrics of a font for a specified pixel size.
            struct FontMetrics {
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

            /// An item in the menu of a menu bar or context menu
            struct MenuEntry {
                /// The text of the menu entry
                title: SharedString,
                /// the icon associated with the menu entry
                icon: Image,
                /// an opaque id that can be used to identify the menu entry
                id: SharedString,
                // keys: KeySequence,
                /// whether the menu entry is enabled
                enabled: bool,
                /// whether the menu entry is checkable
                checkable: bool,
                /// whether the menu entry is checked
                checked: bool,
                /// Sub menu
                has_sub_menu: bool,
                /// The menu entry is a separator
                is_separator: bool,
                /// The shortcut keys
                shortcut: Keys,
            }

            /// A structure representing the four edges of an axis-aligned rectangle
            struct Edges {
                /// The left edge value
                left: Coord,
                /// The top edge value
                top: Coord,
                /// The right edge value
                right: Coord,
                /// The bottom edge value
                bottom: Coord,
            }

            #[non_exhaustive]
            struct ConstraintAdjustment {
                slide: bool,
                flip: bool,
                resize: bool,
            }

            #[non_exhaustive]
            struct PopupAnchor {
                location: PopupAnchorLocation,
                x: Coord,
                y: Coord,
                width: Coord,
                height: Coord,
                gravity: PopupGravity,
                constraint_adjustment_x: ConstraintAdjustment,
                constraint_adjustment_y: ConstraintAdjustment,
            }
        ];
    };
}
