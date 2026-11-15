// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains all builtin structures exposed in the .slint language.

/// Maps the optional `$(= $field_default:expr)?` capture of a
/// [`for_each_builtin_structs!`](crate::for_each_builtin_structs) consumer to an `Option`
/// of the default's `stringify!`-ed tokens:
/// `builtin_struct_field_default_tokens!($($field_default)?)`.
/// Note that raw tokens stringify with spaces, such as `- 1.0` or `SortOrder :: Unsorted`.
#[macro_export]
macro_rules! builtin_struct_field_default_tokens {
    () => {
        None
    };
    ($field_default:expr) => {
        Some(stringify!($field_default))
    };
}

/// Call a macro with every builtin structures exposed in the .slint language
///
/// Each struct is declared with `pub struct` if it should be re-exported in a public
/// language-binding module (e.g. `slint::language` in the Rust crate), or plain `struct`
/// to stay private. Consumers can dispatch on `$vis:vis`.
///
/// A field can declare a default value with `= expression` after its type.
/// The expression is limited to number literals, bool literals, and enum values,
/// because the consumers translate it to every target language: Rust and C++ use
/// the expression verbatim, the other consumers apply their own minimal translation
/// (see [`builtin_struct_field_default_tokens!`](crate::builtin_struct_field_default_tokens))
/// and fail their build on anything outside the supported subset.
/// Fields without a default value default to the zero value of their type.
/// The consumers that generate documentation render the declared value themselves,
/// so don't mention it in the field's doc comment.
///
/// ## Example
/// ```rust
/// macro_rules! print_builtin_structs {
///     ($(
///         $(#[$struct_attr:meta])*
///         $vis:vis struct $Name:ident {
///             $( $(#[$field_attr:meta])* $field:ident : $field_type:ty $(= $field_default:tt)?, )*
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
        $macro! {
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

            /// The color roles of a Material 3 color scheme, as reported by the platform.
            /// The fields are the [Material 3 color roles](https://m3.material.io/styles/color/roles).
            struct SystemColorScheme {
                /// The main color, for the most prominent components such as filled buttons.
                primary: Color,
                /// Text and icons on `primary`.
                on_primary: Color,
                /// A standout fill for key components, such as floating action buttons.
                primary_container: Color,
                /// Text and icons on `primary-container`.
                on_primary_container: Color,
                /// A color for components that need less emphasis than `primary`.
                secondary: Color,
                /// Text and icons on `secondary`.
                on_secondary: Color,
                /// A fill for less prominent components, such as tonal buttons.
                secondary_container: Color,
                /// Text and icons on `secondary-container`.
                on_secondary_container: Color,
                /// The color for contrasting accents.
                tertiary: Color,
                /// Text and icons on `tertiary`.
                on_tertiary: Color,
                /// A fill for contrasting components.
                tertiary_container: Color,
                /// Text and icons on `tertiary-container`.
                on_tertiary_container: Color,
                /// The color for errors.
                error: Color,
                /// Text and icons on `error`.
                on_error: Color,
                /// The fill of error components.
                error_container: Color,
                /// Text and icons on `error-container`.
                on_error_container: Color,
                /// The background of the app.
                background: Color,
                /// Text and icons on `background`.
                on_background: Color,
                /// The default background.
                surface: Color,
                /// Text and icons on `surface`.
                on_surface: Color,
                /// An alternative to `surface`.
                surface_variant: Color,
                /// Lower-emphasis text and icons on any surface.
                on_surface_variant: Color,
                /// Important boundaries, such as the outline of a text field.
                outline: Color,
                /// Decorative boundaries, such as dividers.
                outline_variant: Color,
                /// A surface that contrasts with `surface`, such as a snackbar.
                inverse_surface: Color,
                /// Text and icons on `inverse-surface`.
                inverse_on_surface: Color,
                /// Actionable elements on `inverse-surface`.
                inverse_primary: Color,
                /// Like `primary-container`, but the same in the light and the dark scheme.
                primary_fixed: Color,
                /// Text and icons on `primary-fixed` and `primary-fixed-dim`.
                on_primary_fixed: Color,
                /// A stronger `primary-fixed`.
                primary_fixed_dim: Color,
                /// Lower-emphasis text and icons on `primary-fixed` and `primary-fixed-dim`.
                on_primary_fixed_variant: Color,
                /// Like `secondary-container`, but the same in the light and the dark scheme.
                secondary_fixed: Color,
                /// Text and icons on `secondary-fixed` and `secondary-fixed-dim`.
                on_secondary_fixed: Color,
                /// A stronger `secondary-fixed`.
                secondary_fixed_dim: Color,
                /// Lower-emphasis text and icons on `secondary-fixed` and `secondary-fixed-dim`.
                on_secondary_fixed_variant: Color,
                /// Like `tertiary-container`, but the same in the light and the dark scheme.
                tertiary_fixed: Color,
                /// Text and icons on `tertiary-fixed` and `tertiary-fixed-dim`.
                on_tertiary_fixed: Color,
                /// A stronger `tertiary-fixed`.
                tertiary_fixed_dim: Color,
                /// Lower-emphasis text and icons on `tertiary-fixed` and `tertiary-fixed-dim`.
                on_tertiary_fixed_variant: Color,
                /// The dimmest surface.
                surface_dim: Color,
                /// The brightest surface.
                surface_bright: Color,
                /// The container with the lowest emphasis.
                surface_container_lowest: Color,
                /// A container with low emphasis.
                surface_container_low: Color,
                /// The default container, such as for a navigation bar.
                surface_container: Color,
                /// A container with high emphasis.
                surface_container_high: Color,
                /// The container with the highest emphasis.
                surface_container_highest: Color,
            }

            /// The light and dark Material 3 color schemes reported by the platform.
            struct SystemColorSchemes {
                /// Used while `Palette.color-scheme` is light.
                light: SystemColorScheme,
                /// Used while `Palette.color-scheme` is dark.
                dark: SystemColorScheme,
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

            /// This structure holds the hints that a `TextInput` gives to the platform's input method
            /// (e.g. a soft keyboard) about the expected input.
            /// The input method may take these hints into account, but might also ignore them.
            #[non_exhaustive]
            pub struct InputMethodHints {
                /// The auto-capitalization behavior that the input method should apply.
                capitalization: CapitalizationMode = (CapitalizationMode::Sentences),
                /// Hint that the input method may automatically correct spelling mistakes as the user types.
                auto_correct: bool = true,
                /// Hint that the input method may offer auto-completion suggestions for the entered text.
                auto_complete: bool = true,
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
                /// The platform role of the menu entry
                role: MenuItemRole,
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
        }
    };
}
