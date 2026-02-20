// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*! Module handling mouse events
*/
#![warn(missing_docs)]

use crate::item_tree::ItemTreeRc;
use crate::item_tree::{ItemRc, ItemWeak, VisitChildrenResult};
use crate::items::{DropEvent, ItemRef, MouseCursor, TextCursorDirection};
pub use crate::items::{FocusReason, KeyEvent, KeyboardModifiers, PointerEventButton};
use crate::lengths::{ItemTransform, LogicalPoint, LogicalVector};
use crate::timers::Timer;
use crate::window::{WindowAdapter, WindowInner};
use crate::{Coord, Property, SharedString};
use alloc::rc::Rc;
use alloc::vec::Vec;
use const_field_offset::FieldOffsets;
use core::cell::Cell;
use core::pin::Pin;
use core::time::Duration;

/// A mouse or touch event
///
/// The only difference with [`crate::platform::WindowEvent`] is that it uses untyped `Point`
/// TODO: merge with platform::WindowEvent
#[repr(C)]
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
pub enum MouseEvent {
    /// The mouse or finger was pressed
    /// `position` is the position of the mouse when the event happens.
    /// `button` describes the button that is pressed when the event happens.
    /// `click_count` represents the current number of clicks.
    Pressed { position: LogicalPoint, button: PointerEventButton, click_count: u8, is_touch: bool },
    /// The mouse or finger was released
    /// `position` is the position of the mouse when the event happens.
    /// `button` describes the button that is pressed when the event happens.
    /// `click_count` represents the current number of clicks.
    Released { position: LogicalPoint, button: PointerEventButton, click_count: u8, is_touch: bool },
    /// The position of the pointer has changed
    Moved { position: LogicalPoint, is_touch: bool },
    /// Wheel was operated.
    /// `pos` is the position of the mouse when the event happens.
    /// `delta_x` is the amount of pixels to scroll in horizontal direction,
    /// `delta_y` is the amount of pixels to scroll in vertical direction.
    Wheel { position: LogicalPoint, delta_x: Coord, delta_y: Coord },
    /// The mouse is being dragged over this item.
    /// [`InputEventResult::EventIgnored`] means that the item does not handle the drag operation
    /// and [`InputEventResult::EventAccepted`] means that the item can accept it.
    DragMove(DropEvent),
    /// The mouse is released while dragging over this item.
    Drop(DropEvent),
    /// A platform-recognized pinch gesture (macOS/iOS trackpad, Qt).
    /// `delta` is the incremental scale change; PinchGestureHandler accumulates it.
    PinchGesture { position: LogicalPoint, delta: f32, phase: TouchPhase },
    /// A platform-recognized rotation gesture (macOS/iOS trackpad, Qt).
    /// `delta` is the incremental rotation in degrees using the platform's sign convention
    /// (positive = counterclockwise on macOS). PinchGestureHandler negates this internally
    /// so that its `rotation` property uses positive = clockwise (CSS convention).
    RotationGesture { position: LogicalPoint, delta: f32, phase: TouchPhase },
    /// A platform-recognized double-tap gesture ("smart magnify" on macOS trackpad).
    DoubleTapGesture { position: LogicalPoint },
    /// The mouse exited the item or component
    Exit,
}

impl MouseEvent {
    /// The flag for when event generated from touch
    pub fn is_touch(&self) -> Option<bool> {
        match self {
            MouseEvent::Pressed { is_touch, .. } => Some(*is_touch),
            MouseEvent::Released { is_touch, .. } => Some(*is_touch),
            MouseEvent::Moved { is_touch, .. } => Some(*is_touch),
            MouseEvent::Wheel { .. } => None,
            MouseEvent::PinchGesture { .. } => None,
            MouseEvent::RotationGesture { .. } => None,
            MouseEvent::DoubleTapGesture { .. } => None,
            MouseEvent::DragMove(..) | MouseEvent::Drop(..) => None,
            MouseEvent::Exit => None,
        }
    }

    /// The position of the cursor for this event, if any
    pub fn position(&self) -> Option<LogicalPoint> {
        match self {
            MouseEvent::Pressed { position, .. } => Some(*position),
            MouseEvent::Released { position, .. } => Some(*position),
            MouseEvent::Moved { position, .. } => Some(*position),
            MouseEvent::Wheel { position, .. } => Some(*position),
            MouseEvent::PinchGesture { position, .. } => Some(*position),
            MouseEvent::RotationGesture { position, .. } => Some(*position),
            MouseEvent::DoubleTapGesture { position } => Some(*position),
            MouseEvent::DragMove(e) | MouseEvent::Drop(e) => {
                Some(crate::lengths::logical_point_from_api(e.position))
            }
            MouseEvent::Exit => None,
        }
    }

    /// Translate the position by the given value
    pub fn translate(&mut self, vec: LogicalVector) {
        let pos = match self {
            MouseEvent::Pressed { position, .. } => Some(position),
            MouseEvent::Released { position, .. } => Some(position),
            MouseEvent::Moved { position, .. } => Some(position),
            MouseEvent::Wheel { position, .. } => Some(position),
            MouseEvent::PinchGesture { position, .. } => Some(position),
            MouseEvent::RotationGesture { position, .. } => Some(position),
            MouseEvent::DoubleTapGesture { position } => Some(position),
            MouseEvent::DragMove(e) | MouseEvent::Drop(e) => {
                e.position = crate::api::LogicalPosition::from_euclid(
                    crate::lengths::logical_point_from_api(e.position) + vec,
                );
                None
            }
            MouseEvent::Exit => None,
        };
        if let Some(pos) = pos {
            *pos += vec;
        }
    }

    /// Transform the position by the given item transform.
    pub fn transform(&mut self, transform: ItemTransform) {
        let pos = match self {
            MouseEvent::Pressed { position, .. } => Some(position),
            MouseEvent::Released { position, .. } => Some(position),
            MouseEvent::Moved { position, .. } => Some(position),
            MouseEvent::Wheel { position, .. } => Some(position),
            MouseEvent::PinchGesture { position, .. } => Some(position),
            MouseEvent::RotationGesture { position, .. } => Some(position),
            MouseEvent::DoubleTapGesture { position } => Some(position),
            MouseEvent::DragMove(e) | MouseEvent::Drop(e) => {
                e.position = crate::api::LogicalPosition::from_euclid(
                    transform
                        .transform_point(crate::lengths::logical_point_from_api(e.position).cast())
                        .cast(),
                );
                None
            }
            MouseEvent::Exit => None,
        };
        if let Some(pos) = pos {
            *pos = transform.transform_point(pos.cast()).cast();
        }
    }

    /// Set the click count of the pressed or released event
    fn set_click_count(&mut self, count: u8) {
        match self {
            MouseEvent::Pressed { click_count, .. } | MouseEvent::Released { click_count, .. } => {
                *click_count = count
            }
            _ => (),
        }
    }
}

/// Phase of a touch or gesture event.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TouchPhase {
    /// The gesture began (e.g., first finger touched or platform gesture started).
    Started,
    /// The gesture is ongoing (e.g., fingers moved or platform gesture updated).
    Moved,
    /// The gesture completed normally.
    Ended,
    /// The gesture was cancelled (e.g., interrupted by the system).
    Cancelled,
}

/// This value is returned by the `input_event` function of an Item
/// to notify the run-time about how the event was handled and
/// what the next steps are.
/// See [`crate::items::ItemVTable::input_event`].
#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub enum InputEventResult {
    /// The event was accepted. This may result in additional events, for example
    /// accepting a mouse move will result in a MouseExit event later.
    EventAccepted,
    /// The event was ignored.
    #[default]
    EventIgnored,
    /// All further mouse events need to be sent to this item or component
    GrabMouse,
    /// Will start a drag operation. Can only be returned from a [`crate::items::DragArea`] item.
    StartDrag,
}

/// This value is returned by the `input_event_filter_before_children` function, which
/// can specify how to further process the event.
/// See [`crate::items::ItemVTable::input_event_filter_before_children`].
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub enum InputEventFilterResult {
    /// The event is going to be forwarded to children, then the [`crate::items::ItemVTable::input_event`]
    /// function is called
    #[default]
    ForwardEvent,
    /// The event will be forwarded to the children, but the [`crate::items::ItemVTable::input_event`] is not
    /// going to be called for this item
    ForwardAndIgnore,
    /// Just like `ForwardEvent`, but even in the case that children grabs the mouse, this function
    /// will still be called for further events
    ForwardAndInterceptGrab,
    /// The event will not be forwarded to children, if a child already had the grab, the
    /// grab will be cancelled with a [`MouseEvent::Exit`] event
    Intercept,
    /// The event will be forwarded to the children with a delay (in milliseconds), unless it is
    /// being intercepted.
    /// This is what happens when the flickable wants to delay the event.
    /// This should only be used for Press event, and the event will be sent after the delay, or
    /// if a release event is seen before that delay
    //(Can't use core::time::Duration because it is not repr(c))
    DelayForwarding(u64),
}

/// This module contains the constant character code used to represent the keys.
#[allow(missing_docs, non_upper_case_globals)]
pub mod key_codes {
    macro_rules! declare_consts_for_special_keys {
       ($($char:literal # $name:ident # $($shifted:expr)? $(=> $($_qt:ident)|* # $($_winit:ident $(($_pos:ident))?)|*    # $($_xkb:ident)|* )? ;)*) => {
            $(pub const $name : char = $char;)*

            #[allow(missing_docs)]
            #[derive(Debug, Copy, Clone, PartialEq)]
            #[non_exhaustive]
            /// The `Key` enum is used to map a specific key by name e.g. `Key::Control` to an
            /// internal used unicode representation. The enum is convertible to [`std::char`] and [`slint::SharedString`](`crate::SharedString`).
            /// Use this with [`slint::platform::WindowEvent`](`crate::platform::WindowEvent`) to supply key events to Slint's platform abstraction.
            ///
            /// # Example
            ///
            /// Send an tab key press event to a window
            ///
            /// ```
            /// use slint::platform::{WindowEvent, Key};
            /// fn send_tab_pressed(window: &slint::Window) {
            ///     window.dispatch_event(WindowEvent::KeyPressed { text: Key::Tab.into() });
            /// }
            /// ```
            pub enum Key {
                $($name,)*
            }

            impl From<Key> for char {
                fn from(k: Key) -> Self {
                    match k {
                        $(Key::$name => $name,)*
                    }
                }
            }

            impl From<Key> for crate::SharedString {
                fn from(k: Key) -> Self {
                    char::from(k).into()
                }
            }
        };
    }

    i_slint_common::for_each_keys!(declare_consts_for_special_keys);
}

/// Internal struct to maintain the pressed/released state of the keys that
/// map to keyboard modifiers.
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct InternalKeyboardModifierState {
    left_alt: bool,
    right_alt: bool,
    altgr: bool,
    left_control: bool,
    right_control: bool,
    left_meta: bool,
    right_meta: bool,
    left_shift: bool,
    right_shift: bool,
}

impl InternalKeyboardModifierState {
    /// Updates a flag of the modifiers if the key of the given text is pressed.
    /// Returns an updated modifier if detected; None otherwise;
    pub(crate) fn state_update(mut self, pressed: bool, text: &SharedString) -> Option<Self> {
        if let Some(key_code) = text.chars().next() {
            match key_code {
                key_codes::Alt => self.left_alt = pressed,
                key_codes::AltGr => self.altgr = pressed,
                key_codes::Control => self.left_control = pressed,
                key_codes::ControlR => self.right_control = pressed,
                key_codes::Shift => self.left_shift = pressed,
                key_codes::ShiftR => self.right_shift = pressed,
                key_codes::Meta => self.left_meta = pressed,
                key_codes::MetaR => self.right_meta = pressed,
                _ => return None,
            };

            // Encoded keyboard modifiers must appear as individual key events. This could
            // be relaxed by implementing a string split, but right now WindowEvent::KeyPressed
            // holds only a single char.
            debug_assert_eq!(key_code.len_utf8(), text.len());
        }

        // Special cases:
        #[cfg(target_os = "windows")]
        {
            if self.altgr {
                // Windows sends Ctrl followed by AltGr on AltGr. Disable the Ctrl again!
                self.left_control = false;
                self.right_control = false;
            } else if self.control() && self.alt() {
                // Windows treats Ctrl-Alt as AltGr
                self.left_control = false;
                self.right_control = false;
                self.left_alt = false;
                self.right_alt = false;
            }
        }

        Some(self)
    }

    pub fn shift(&self) -> bool {
        self.right_shift || self.left_shift
    }
    pub fn alt(&self) -> bool {
        self.right_alt || self.left_alt
    }
    pub fn meta(&self) -> bool {
        self.right_meta || self.left_meta
    }
    pub fn control(&self) -> bool {
        self.right_control || self.left_control
    }
}

impl From<InternalKeyboardModifierState> for KeyboardModifiers {
    fn from(internal_state: InternalKeyboardModifierState) -> Self {
        Self {
            alt: internal_state.alt(),
            control: internal_state.control(),
            meta: internal_state.meta(),
            shift: internal_state.shift(),
        }
    }
}

/// A `KeyboardShortcut` is created by the `@keys(...)` macro and
/// defines which key events match the given shortcuts.
///
/// See [`Self::matches()`] for details
#[derive(Clone, Eq, PartialEq, Default)]
#[repr(C)]
pub struct KeyboardShortcut {
    /// The `key` used to trigger the shortcut
    ///
    /// Note: This is currently converted to lowercase when the shortcut is created!
    key: SharedString,
    /// `KeyboardModifier`s that need to be pressed for the shortcut to fire
    modifiers: KeyboardModifiers,
    /// Whether to ignore shift state when matching the shortcut
    ignore_shift: bool,
    /// Whether to ignore alt state when matching the shortcut
    ignore_alt: bool,
}

/// Re-exported in private_unstable_api to create a KeyboardShortcut struct.
pub fn make_keyboard_shortcut(
    key: SharedString,
    modifiers: KeyboardModifiers,
    ignore_shift: bool,
    ignore_alt: bool,
) -> KeyboardShortcut {
    KeyboardShortcut { key: key.to_lowercase().into(), modifiers, ignore_shift, ignore_alt }
}

#[cfg(feature = "ffi")]
#[allow(unsafe_code)]
pub(crate) mod ffi {
    use super::*;

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_keyboard_shortcut(
        key: &SharedString,
        alt: bool,
        control: bool,
        shift: bool,
        meta: bool,
        ignore_shift: bool,
        ignore_alt: bool,
        out: &mut KeyboardShortcut,
    ) {
        *out = make_keyboard_shortcut(
            key.clone(),
            KeyboardModifiers { alt, control, shift, meta },
            ignore_shift,
            ignore_alt,
        );
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_keyboard_shortcut_debug_string(
        shortcut: &KeyboardShortcut,
        out: &mut SharedString,
    ) {
        *out = crate::format!("{shortcut:?}")
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_keyboard_shortcut_to_platform_string(
        shortcut: &KeyboardShortcut,
        out: &mut SharedString,
    ) {
        *out = shortcut.to_platform_string();
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_keyboard_shortcut_matches(
        shortcut: &KeyboardShortcut,
        key_event: &KeyEvent,
    ) -> bool {
        shortcut.matches(key_event)
    }
}

impl KeyboardShortcut {
    /// Check whether a `KeyboardShortcut` can be triggered by the given `KeyEvent`
    pub fn matches(&self, key_event: &KeyEvent) -> bool {
        // An empty KeyboardShortcut is never triggered, even if the modifiers match.
        if self.key.is_empty() {
            return false;
        }

        // TODO: Should this check the event_type and only match on KeyReleased?
        let mut expected_modifiers = self.modifiers.clone();
        if self.ignore_shift {
            expected_modifiers.shift = key_event.modifiers.shift;
        }
        if self.ignore_alt {
            expected_modifiers.alt = key_event.modifiers.alt;
        }
        // Note: The constructor of KeyboardShortcut ensures that the shortcut's key is already
        // in lowercase, so we can just compare it to the lowercased event text.
        //
        // This improves our handling of CapsLock and Shift, as the event text will be in uppercase
        // if caps lock is active, even if shift is not pressed.
        let event_text = key_event.text.chars().flat_map(|character| character.to_lowercase());

        event_text.eq(self.key.chars()) && key_event.modifiers == expected_modifiers
    }

    /// Convert the keyboard shortcut to a string that looks native on the current platform.
    ///
    /// For example, the shortcut created with @keys(Meta + Control + A)
    /// will be converted like this:
    /// - **macOS**: `⌃⌘A`
    /// - **Windows**: `Super+Ctrl+A`
    /// - **Linux**: `Super+Ctrl+A`
    ///
    /// Note that this functions output is best-effort and may be adjusted/improved at any time,
    /// do not rely on this output to be stable!
    //
    // References for implementation
    // - macOS: <https://developer.apple.com/design/human-interface-guidelines/keyboards>
    // - Windows: <https://learn.microsoft.com/en-us/windows/apps/design/input/keyboard-accelerators>
    // - Linux: <https://developer.gnome.org/hig/guidelines/keyboard.html>
    pub fn to_platform_string(&self) -> SharedString {
        if self.key.is_empty() {
            return SharedString::default();
        }

        let mut parts = alloc::vec![];
        let separator;

        #[cfg(target_os = "macos")]
        {
            separator = "";

            // Slint remaps modifiers on macOS: control → Command, meta → Control
            // From Apple's documentation:
            //
            // List modifier keys in the correct order.
            // If you use more than one modifier key in a custom shortcut, always list them in this order:
            //  Control, Option, Shift, Command
            if self.modifiers.meta {
                parts.push("⌃");
            }
            if !self.ignore_alt && self.modifiers.alt {
                parts.push("⌥");
            }
            if !self.ignore_shift && self.modifiers.shift {
                parts.push("⇧");
            }
            if self.modifiers.control {
                parts.push("⌘");
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            separator = "+";

            // TODO: These should probably be translated, but better to have at least
            // platform-local names than nothing.
            #[cfg(target_os = "windows")]
            let (ctrl_str, alt_str, shift_str, meta_str) = ("Ctrl", "Alt", "Shift", "Win");

            #[cfg(not(target_os = "windows"))]
            let (ctrl_str, alt_str, shift_str, meta_str) = ("Ctrl", "Alt", "Shift", "Super");

            if self.modifiers.meta {
                parts.push(meta_str);
            }
            if self.modifiers.control {
                parts.push(ctrl_str);
            }
            if !self.ignore_alt && self.modifiers.alt {
                parts.push(alt_str);
            }
            if !self.ignore_shift && self.modifiers.shift {
                parts.push(shift_str);
            }
        }
        let key_display = self.format_key_for_display();
        parts.push(&key_display);

        parts.join(separator).into()
    }

    fn format_key_for_display(&self) -> alloc::string::String {
        let key_str = self.key.as_str();
        let first_char = key_str.chars().next();

        if let Some(first_char) = first_char {
            macro_rules! check_special_key {
                ($($char:literal # $name:ident # $($shifted:expr)? $(=> $($qt:ident)|* # $($winit:ident $(($_pos:ident))?)|* # $($xkb:ident)|*)? ;)*) => {
                    match first_char {
                    $($(
                        // Use $qt as a marker - if it exists, generate the check
                        $char => {
                            let _ = stringify!($($qt)|*); // Use $qt to enable this branch
                            return stringify!($name).into();
                        }
                    )?)*
                        _ => ()
                    }
                };
            }
            i_slint_common::for_each_keys!(check_special_key);
        }

        if key_str.chars().count() == 1 {
            return key_str.to_uppercase();
        }

        key_str.into()
    }
}

impl core::fmt::Debug for KeyboardShortcut {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Make sure to keep this in sync with the implemenation in compiler/langtype.rs
        if self.key.is_empty() {
            write!(f, "")
        } else {
            let alt = self
                .ignore_alt
                .then_some("IgnoreAlt+")
                .or(self.modifiers.alt.then_some("Alt+"))
                .unwrap_or_default();
            let ctrl = if self.modifiers.control { "Control+" } else { "" };
            let meta = if self.modifiers.meta { "Meta+" } else { "" };
            let shift = self
                .ignore_shift
                .then_some("IgnoreShift+")
                .or(self.modifiers.shift.then_some("Shift+"))
                .unwrap_or_default();
            let keycode: SharedString = self
                .key
                .chars()
                .flat_map(|character| {
                    let mut escaped = alloc::vec![];
                    if character.is_control() {
                        escaped.extend(character.escape_unicode());
                    } else {
                        escaped.push(character);
                    }
                    escaped
                })
                .collect();
            write!(f, "{meta}{ctrl}{alt}{shift}\"{keycode}\"")
        }
    }
}

/// This enum defines the different kinds of key events that can happen.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum KeyEventType {
    /// A key on a keyboard was pressed.
    #[default]
    KeyPressed = 0,
    /// A key on a keyboard was released.
    KeyReleased = 1,
    /// The input method updates the currently composed text. The KeyEvent's text field is the pre-edit text and
    /// composition_selection specifies the placement of the cursor within the pre-edit text.
    UpdateComposition = 2,
    /// The input method replaces the currently composed text with the final result of the composition.
    CommitComposition = 3,
}

impl KeyEvent {
    /// If a shortcut was pressed, this function returns `Some(StandardShortcut)`.
    /// Otherwise it returns None.
    pub fn shortcut(&self) -> Option<StandardShortcut> {
        if self.modifiers.control && !self.modifiers.shift {
            match self.text.as_str() {
                #[cfg(not(target_arch = "wasm32"))]
                "c" => Some(StandardShortcut::Copy),
                #[cfg(not(target_arch = "wasm32"))]
                "x" => Some(StandardShortcut::Cut),
                #[cfg(not(target_arch = "wasm32"))]
                "v" => Some(StandardShortcut::Paste),
                "a" => Some(StandardShortcut::SelectAll),
                "f" => Some(StandardShortcut::Find),
                "s" => Some(StandardShortcut::Save),
                "p" => Some(StandardShortcut::Print),
                "z" => Some(StandardShortcut::Undo),
                #[cfg(target_os = "windows")]
                "y" => Some(StandardShortcut::Redo),
                "r" => Some(StandardShortcut::Refresh),
                _ => None,
            }
        } else if self.modifiers.control && self.modifiers.shift {
            match self.text.as_str() {
                #[cfg(not(target_os = "windows"))]
                "z" | "Z" => Some(StandardShortcut::Redo),
                _ => None,
            }
        } else {
            None
        }
    }

    /// If a shortcut concerning text editing was pressed, this function
    /// returns `Some(TextShortcut)`. Otherwise it returns None.
    pub fn text_shortcut(&self) -> Option<TextShortcut> {
        let keycode = self.text.chars().next()?;

        let is_apple = crate::is_apple_platform();

        let move_mod = if is_apple {
            self.modifiers.alt && !self.modifiers.control && !self.modifiers.meta
        } else {
            self.modifiers.control && !self.modifiers.alt && !self.modifiers.meta
        };

        if move_mod {
            match keycode {
                key_codes::LeftArrow => {
                    return Some(TextShortcut::Move(TextCursorDirection::BackwardByWord));
                }
                key_codes::RightArrow => {
                    return Some(TextShortcut::Move(TextCursorDirection::ForwardByWord));
                }
                key_codes::UpArrow => {
                    return Some(TextShortcut::Move(TextCursorDirection::StartOfParagraph));
                }
                key_codes::DownArrow => {
                    return Some(TextShortcut::Move(TextCursorDirection::EndOfParagraph));
                }
                key_codes::Backspace => {
                    return Some(TextShortcut::DeleteWordBackward);
                }
                key_codes::Delete => {
                    return Some(TextShortcut::DeleteWordForward);
                }
                _ => (),
            };
        }

        #[cfg(not(target_os = "macos"))]
        {
            if self.modifiers.control && !self.modifiers.alt && !self.modifiers.meta {
                match keycode {
                    key_codes::Home => {
                        return Some(TextShortcut::Move(TextCursorDirection::StartOfText));
                    }
                    key_codes::End => {
                        return Some(TextShortcut::Move(TextCursorDirection::EndOfText));
                    }
                    _ => (),
                };
            }
        }

        if is_apple && self.modifiers.control {
            match keycode {
                key_codes::LeftArrow => {
                    return Some(TextShortcut::Move(TextCursorDirection::StartOfLine));
                }
                key_codes::RightArrow => {
                    return Some(TextShortcut::Move(TextCursorDirection::EndOfLine));
                }
                key_codes::UpArrow => {
                    return Some(TextShortcut::Move(TextCursorDirection::StartOfText));
                }
                key_codes::DownArrow => {
                    return Some(TextShortcut::Move(TextCursorDirection::EndOfText));
                }
                key_codes::Backspace => {
                    return Some(TextShortcut::DeleteToStartOfLine);
                }
                _ => (),
            };
        }

        if let Ok(direction) = TextCursorDirection::try_from(keycode) {
            Some(TextShortcut::Move(direction))
        } else {
            match keycode {
                key_codes::Backspace => Some(TextShortcut::DeleteBackward),
                key_codes::Delete => Some(TextShortcut::DeleteForward),
                _ => None,
            }
        }
    }
}

/// Represents a non context specific shortcut.
pub enum StandardShortcut {
    /// Copy Something
    Copy,
    /// Cut Something
    Cut,
    /// Paste Something
    Paste,
    /// Select All
    SelectAll,
    /// Find/Search Something
    Find,
    /// Save Something
    Save,
    /// Print Something
    Print,
    /// Undo the last action
    Undo,
    /// Redo the last undone action
    Redo,
    /// Refresh
    Refresh,
}

/// Shortcuts that are used when editing text
pub enum TextShortcut {
    /// Move the cursor
    Move(TextCursorDirection),
    /// Delete the Character to the right of the cursor
    DeleteForward,
    /// Delete the Character to the left of the cursor (aka Backspace).
    DeleteBackward,
    /// Delete the word to the right of the cursor
    DeleteWordForward,
    /// Delete the word to the left of the cursor (aka Ctrl + Backspace).
    DeleteWordBackward,
    /// Delete to the left of the cursor until the start of the line
    DeleteToStartOfLine,
}

/// Represents how an item's key_event handler dealt with a key event.
/// An accepted event results in no further event propagation.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum KeyEventResult {
    /// The event was handled.
    EventAccepted,
    /// The event was not handled and should be sent to other items.
    #[default]
    EventIgnored,
}

/// Represents how an item's focus_event handler dealt with a focus event.
/// An accepted event results in no further event propagation.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FocusEventResult {
    /// The event was handled.
    FocusAccepted,
    /// The event was not handled and should be sent to other items.
    #[default]
    FocusIgnored,
}

/// This event is sent to a component and items when they receive or lose
/// the keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum FocusEvent {
    /// This event is sent when an item receives the focus.
    FocusIn(FocusReason),
    /// This event is sent when an item loses the focus.
    FocusOut(FocusReason),
}

/// This state is used to count the clicks separated by [`crate::platform::Platform::click_interval`]
#[derive(Default)]
pub struct ClickState {
    click_count_time_stamp: Cell<Option<crate::animations::Instant>>,
    click_count: Cell<u8>,
    click_position: Cell<LogicalPoint>,
    click_button: Cell<PointerEventButton>,
}

impl ClickState {
    /// Resets the timer and count.
    fn restart(&self, position: LogicalPoint, button: PointerEventButton) {
        self.click_count.set(0);
        self.click_count_time_stamp.set(Some(crate::animations::Instant::now()));
        self.click_position.set(position);
        self.click_button.set(button);
    }

    /// Reset to an invalid state
    pub fn reset(&self) {
        self.click_count.set(0);
        self.click_count_time_stamp.replace(None);
    }

    /// Check if the click is repeated.
    pub fn check_repeat(&self, mouse_event: MouseEvent, click_interval: Duration) -> MouseEvent {
        match mouse_event {
            MouseEvent::Pressed { position, button, is_touch, .. } => {
                let instant_now = crate::animations::Instant::now();

                if let Some(click_count_time_stamp) = self.click_count_time_stamp.get() {
                    if instant_now - click_count_time_stamp < click_interval
                        && button == self.click_button.get()
                        && (position - self.click_position.get()).square_length() < 100 as _
                    {
                        self.click_count.set(self.click_count.get().wrapping_add(1));
                        self.click_count_time_stamp.set(Some(instant_now));
                    } else {
                        self.restart(position, button);
                    }
                } else {
                    self.restart(position, button);
                }

                return MouseEvent::Pressed {
                    position,
                    button,
                    click_count: self.click_count.get(),
                    is_touch,
                };
            }
            MouseEvent::Released { position, button, is_touch, .. } => {
                return MouseEvent::Released {
                    position,
                    button,
                    click_count: self.click_count.get(),
                    is_touch,
                };
            }
            _ => {}
        };

        mouse_event
    }
}

/// The state which a window should hold for the mouse input
#[derive(Default)]
pub struct MouseInputState {
    /// The stack of item which contain the mouse cursor (or grab),
    /// along with the last result from the input function
    item_stack: Vec<(ItemWeak, InputEventFilterResult)>,
    /// Offset to apply to the first item of the stack (used if there is a popup)
    pub(crate) offset: LogicalPoint,
    /// true if the top item of the stack has the mouse grab
    grabbed: bool,
    /// When this is Some, it means we are in the middle of a drag-drop operation and it contains the dragged data.
    /// The `position` field has no signification
    pub(crate) drag_data: Option<DropEvent>,
    delayed: Option<(crate::timers::Timer, MouseEvent)>,
    delayed_exit_items: Vec<ItemWeak>,
    pub(crate) cursor: MouseCursor,
}

impl MouseInputState {
    /// Return the item in the top of the stack
    fn top_item(&self) -> Option<ItemRc> {
        self.item_stack.last().and_then(|x| x.0.upgrade())
    }

    /// Returns the item in the top of the stack, if there is a delayed event, this would be the top of the delayed stack
    pub fn top_item_including_delayed(&self) -> Option<ItemRc> {
        self.delayed_exit_items.last().and_then(|x| x.upgrade()).or_else(|| self.top_item())
    }
}

/// Try to handle the mouse grabber. Return None if the event has been handled, otherwise
/// return the event that must be handled
pub(crate) fn handle_mouse_grab(
    mouse_event: &MouseEvent,
    window_adapter: &Rc<dyn WindowAdapter>,
    mouse_input_state: &mut MouseInputState,
) -> Option<MouseEvent> {
    if !mouse_input_state.grabbed || mouse_input_state.item_stack.is_empty() {
        return Some(mouse_event.clone());
    };

    let mut event = mouse_event.clone();
    let mut intercept = false;
    let mut invalid = false;

    event.translate(-mouse_input_state.offset.to_vector());

    mouse_input_state.item_stack.retain(|it| {
        if invalid {
            return false;
        }
        let item = if let Some(item) = it.0.upgrade() {
            item
        } else {
            invalid = true;
            return false;
        };
        if intercept {
            item.borrow().as_ref().input_event(
                &MouseEvent::Exit,
                window_adapter,
                &item,
                &mut mouse_input_state.cursor,
            );
            return false;
        }
        let g = item.geometry();
        event.translate(-g.origin.to_vector());
        if window_adapter.renderer().supports_transformations() {
            if let Some(inverse_transform) = item.inverse_children_transform() {
                event.transform(inverse_transform);
            }
        }

        let interested = matches!(
            it.1,
            InputEventFilterResult::ForwardAndInterceptGrab
                | InputEventFilterResult::DelayForwarding(_)
        );

        if interested
            && item.borrow().as_ref().input_event_filter_before_children(
                &event,
                window_adapter,
                &item,
                &mut mouse_input_state.cursor,
            ) == InputEventFilterResult::Intercept
        {
            intercept = true;
        }
        true
    });
    if invalid {
        return Some(mouse_event.clone());
    }

    let grabber = mouse_input_state.top_item().unwrap();
    let input_result = grabber.borrow().as_ref().input_event(
        &event,
        window_adapter,
        &grabber,
        &mut mouse_input_state.cursor,
    );
    match input_result {
        InputEventResult::GrabMouse => None,
        InputEventResult::StartDrag => {
            mouse_input_state.grabbed = false;
            let drag_area_item = grabber.downcast::<crate::items::DragArea>().unwrap();
            mouse_input_state.drag_data = Some(DropEvent {
                mime_type: drag_area_item.as_pin_ref().mime_type(),
                data: drag_area_item.as_pin_ref().data(),
                position: Default::default(),
            });
            None
        }
        _ => {
            mouse_input_state.grabbed = false;
            // Return a move event so that the new position can be registered properly
            Some(mouse_event.position().map_or(MouseEvent::Exit, |position| MouseEvent::Moved {
                position,
                is_touch: mouse_event.is_touch().unwrap_or(false),
            }))
        }
    }
}

pub(crate) fn send_exit_events(
    old_input_state: &MouseInputState,
    new_input_state: &mut MouseInputState,
    mut pos: Option<LogicalPoint>,
    window_adapter: &Rc<dyn WindowAdapter>,
) {
    // Note that exit events can't actually change the cursor from default so we'll ignore the result
    let cursor = &mut MouseCursor::Default;

    for it in core::mem::take(&mut new_input_state.delayed_exit_items) {
        let Some(item) = it.upgrade() else { continue };
        item.borrow().as_ref().input_event(&MouseEvent::Exit, window_adapter, &item, cursor);
    }

    let mut clipped = false;
    for (idx, it) in old_input_state.item_stack.iter().enumerate() {
        let Some(item) = it.0.upgrade() else { break };
        let g = item.geometry();
        let contains = pos.is_some_and(|p| g.contains(p));
        if let Some(p) = pos.as_mut() {
            *p -= g.origin.to_vector();
            if window_adapter.renderer().supports_transformations() {
                if let Some(inverse_transform) = item.inverse_children_transform() {
                    *p = inverse_transform.transform_point(p.cast()).cast();
                }
            }
        }
        if !contains || clipped {
            if item.borrow().as_ref().clips_children() {
                clipped = true;
            }
            item.borrow().as_ref().input_event(&MouseEvent::Exit, window_adapter, &item, cursor);
        } else if new_input_state.item_stack.get(idx).is_none_or(|(x, _)| *x != it.0) {
            // The item is still under the mouse, but no longer in the item stack. We should also sent the exit event, unless we delay it
            if new_input_state.delayed.is_some() {
                new_input_state.delayed_exit_items.push(it.0.clone());
            } else {
                item.borrow().as_ref().input_event(
                    &MouseEvent::Exit,
                    window_adapter,
                    &item,
                    cursor,
                );
            }
        }
    }
}

/// Process the `mouse_event` on the `component`, the `mouse_grabber_stack` is the previous stack
/// of mouse grabber.
/// Returns a new mouse grabber stack.
pub fn process_mouse_input(
    root: ItemRc,
    mouse_event: &MouseEvent,
    window_adapter: &Rc<dyn WindowAdapter>,
    mouse_input_state: MouseInputState,
) -> MouseInputState {
    let mut result = MouseInputState {
        drag_data: mouse_input_state.drag_data.clone(),
        cursor: mouse_input_state.cursor,
        ..Default::default()
    };
    let r = send_mouse_event_to_item(
        mouse_event,
        root.clone(),
        window_adapter,
        &mut result,
        mouse_input_state.top_item().as_ref(),
        false,
    );
    if mouse_input_state.delayed.is_some()
        && (!r.has_aborted()
            || Option::zip(result.item_stack.last(), mouse_input_state.item_stack.last())
                .is_none_or(|(a, b)| a.0 != b.0))
    {
        // Keep the delayed event
        return mouse_input_state;
    }
    send_exit_events(&mouse_input_state, &mut result, mouse_event.position(), window_adapter);

    if let MouseEvent::Wheel { position, .. } = mouse_event {
        if r.has_aborted() {
            // An accepted wheel event might have moved things. Send a move event at the position to reset the has-hover
            return process_mouse_input(
                root,
                &MouseEvent::Moved { position: *position, is_touch: false },
                window_adapter,
                result,
            );
        }
    }

    result
}

pub(crate) fn process_delayed_event(
    window_adapter: &Rc<dyn WindowAdapter>,
    mut mouse_input_state: MouseInputState,
) -> MouseInputState {
    // the take bellow will also destroy the Timer
    let event = match mouse_input_state.delayed.take() {
        Some(e) => e.1,
        None => return mouse_input_state,
    };

    let top_item = match mouse_input_state.top_item() {
        Some(i) => i,
        None => return MouseInputState::default(),
    };

    let mut actual_visitor =
        |component: &ItemTreeRc, index: u32, _: Pin<ItemRef>| -> VisitChildrenResult {
            send_mouse_event_to_item(
                &event,
                ItemRc::new(component.clone(), index),
                window_adapter,
                &mut mouse_input_state,
                Some(&top_item),
                true,
            )
        };
    vtable::new_vref!(let mut actual_visitor : VRefMut<crate::item_tree::ItemVisitorVTable> for crate::item_tree::ItemVisitor = &mut actual_visitor);
    vtable::VRc::borrow_pin(top_item.item_tree()).as_ref().visit_children_item(
        top_item.index() as isize,
        crate::item_tree::TraversalOrder::FrontToBack,
        actual_visitor,
    );
    mouse_input_state
}

fn send_mouse_event_to_item(
    mouse_event: &MouseEvent,
    item_rc: ItemRc,
    window_adapter: &Rc<dyn WindowAdapter>,
    result: &mut MouseInputState,
    last_top_item: Option<&ItemRc>,
    ignore_delays: bool,
) -> VisitChildrenResult {
    let item = item_rc.borrow();
    let geom = item_rc.geometry();
    // translated in our coordinate
    let mut event_for_children = mouse_event.clone();
    // Unapply the translation to go from 'world' space to local space
    event_for_children.translate(-geom.origin.to_vector());
    if window_adapter.renderer().supports_transformations() {
        // Unapply other transforms.
        if let Some(inverse_transform) = item_rc.inverse_children_transform() {
            event_for_children.transform(inverse_transform);
        }
    }

    let filter_result = if mouse_event.position().is_some_and(|p| geom.contains(p))
        || item.as_ref().clips_children()
    {
        item.as_ref().input_event_filter_before_children(
            &event_for_children,
            window_adapter,
            &item_rc,
            &mut result.cursor,
        )
    } else {
        InputEventFilterResult::ForwardAndIgnore
    };

    let (forward_to_children, ignore) = match filter_result {
        InputEventFilterResult::ForwardEvent => (true, false),
        InputEventFilterResult::ForwardAndIgnore => (true, true),
        InputEventFilterResult::ForwardAndInterceptGrab => (true, false),
        InputEventFilterResult::Intercept => (false, false),
        InputEventFilterResult::DelayForwarding(_) if ignore_delays => (true, false),
        InputEventFilterResult::DelayForwarding(duration) => {
            let timer = Timer::default();
            let w = Rc::downgrade(window_adapter);
            timer.start(
                crate::timers::TimerMode::SingleShot,
                Duration::from_millis(duration),
                move || {
                    if let Some(w) = w.upgrade() {
                        WindowInner::from_pub(w.window()).process_delayed_event();
                    }
                },
            );
            result.delayed = Some((timer, event_for_children));
            result
                .item_stack
                .push((item_rc.downgrade(), InputEventFilterResult::DelayForwarding(duration)));
            return VisitChildrenResult::abort(item_rc.index(), 0);
        }
    };

    result.item_stack.push((item_rc.downgrade(), filter_result));
    if forward_to_children {
        let mut actual_visitor =
            |component: &ItemTreeRc, index: u32, _: Pin<ItemRef>| -> VisitChildrenResult {
                send_mouse_event_to_item(
                    &event_for_children,
                    ItemRc::new(component.clone(), index),
                    window_adapter,
                    result,
                    last_top_item,
                    ignore_delays,
                )
            };
        vtable::new_vref!(let mut actual_visitor : VRefMut<crate::item_tree::ItemVisitorVTable> for crate::item_tree::ItemVisitor = &mut actual_visitor);
        let r = vtable::VRc::borrow_pin(item_rc.item_tree()).as_ref().visit_children_item(
            item_rc.index() as isize,
            crate::item_tree::TraversalOrder::FrontToBack,
            actual_visitor,
        );
        if r.has_aborted() {
            return r;
        }
    };

    let r = if ignore {
        InputEventResult::EventIgnored
    } else {
        let mut event = mouse_event.clone();
        event.translate(-geom.origin.to_vector());
        if last_top_item.is_none_or(|x| *x != item_rc) {
            event.set_click_count(0);
        }
        item.as_ref().input_event(&event, window_adapter, &item_rc, &mut result.cursor)
    };
    match r {
        InputEventResult::EventAccepted => VisitChildrenResult::abort(item_rc.index(), 0),
        InputEventResult::EventIgnored => {
            let _pop = result.item_stack.pop();
            debug_assert_eq!(
                _pop.map(|x| (x.0.upgrade().unwrap().index(), x.1)).unwrap(),
                (item_rc.index(), filter_result)
            );
            VisitChildrenResult::CONTINUE
        }
        InputEventResult::GrabMouse => {
            result.item_stack.last_mut().unwrap().1 =
                InputEventFilterResult::ForwardAndInterceptGrab;
            result.grabbed = true;
            VisitChildrenResult::abort(item_rc.index(), 0)
        }
        InputEventResult::StartDrag => {
            result.item_stack.last_mut().unwrap().1 =
                InputEventFilterResult::ForwardAndInterceptGrab;
            result.grabbed = false;
            let drag_area_item = item_rc.downcast::<crate::items::DragArea>().unwrap();
            result.drag_data = Some(DropEvent {
                mime_type: drag_area_item.as_pin_ref().mime_type(),
                data: drag_area_item.as_pin_ref().data(),
                position: Default::default(),
            });
            VisitChildrenResult::abort(item_rc.index(), 0)
        }
    }
}

/// The TextCursorBlinker takes care of providing a toggled boolean property
/// that can be used to animate a blinking cursor. It's typically stored in the
/// Window using a Weak and set_binding() can be used to set up a binding on a given
/// property that'll keep it up-to-date. That binding keeps a strong reference to the
/// blinker. If the underlying item that uses it goes away, the binding goes away and
/// so does the blinker.
#[derive(FieldOffsets)]
#[repr(C)]
#[pin]
pub(crate) struct TextCursorBlinker {
    cursor_visible: Property<bool>,
    cursor_blink_timer: crate::timers::Timer,
}

impl TextCursorBlinker {
    /// Creates a new instance, wrapped in a Pin<Rc<_>> because the boolean property
    /// the blinker properties uses the property system that requires pinning.
    pub fn new() -> Pin<Rc<Self>> {
        Rc::pin(Self {
            cursor_visible: Property::new(true),
            cursor_blink_timer: Default::default(),
        })
    }

    /// Sets a binding on the provided property that will ensure that the property value
    /// is true when the cursor should be shown and false if not.
    pub fn set_binding(
        instance: Pin<Rc<TextCursorBlinker>>,
        prop: &Property<bool>,
        cycle_duration: Duration,
    ) {
        instance.as_ref().cursor_visible.set(true);
        // Re-start timer, in case.
        Self::start(&instance, cycle_duration);
        prop.set_binding(move || {
            TextCursorBlinker::FIELD_OFFSETS.cursor_visible.apply_pin(instance.as_ref()).get()
        });
    }

    /// Starts the blinking cursor timer that will toggle the cursor and update all bindings that
    /// were installed on properties with set_binding call.
    pub fn start(self: &Pin<Rc<Self>>, cycle_duration: Duration) {
        if self.cursor_blink_timer.running() {
            self.cursor_blink_timer.restart();
        } else {
            let toggle_cursor = {
                let weak_blinker = pin_weak::rc::PinWeak::downgrade(self.clone());
                move || {
                    if let Some(blinker) = weak_blinker.upgrade() {
                        let visible = TextCursorBlinker::FIELD_OFFSETS
                            .cursor_visible
                            .apply_pin(blinker.as_ref())
                            .get();
                        blinker.cursor_visible.set(!visible);
                    }
                }
            };
            if !cycle_duration.is_zero() {
                self.cursor_blink_timer.start(
                    crate::timers::TimerMode::Repeated,
                    cycle_duration / 2,
                    toggle_cursor,
                );
            }
        }
    }

    /// Stops the blinking cursor timer. This is usually used for example when the window that contains
    /// text editable elements looses the focus or is hidden.
    pub fn stop(&self) {
        self.cursor_blink_timer.stop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate alloc;

    #[test]
    fn test_to_platform_string() {
        let test_cases = [
            (
                "a",
                KeyboardModifiers { alt: false, control: true, shift: false, meta: false },
                false,
                false,
                "⌘A",
                "Ctrl+A",
                "Ctrl+A",
            ),
            (
                "a",
                KeyboardModifiers { alt: true, control: true, shift: true, meta: true },
                false,
                false,
                "⌃⌥⇧⌘A",
                "Win+Ctrl+Alt+Shift+A",
                "Super+Ctrl+Alt+Shift+A",
            ),
            (
                "\u{001b}",
                KeyboardModifiers { alt: false, control: true, shift: true, meta: false },
                false,
                false,
                "⇧⌘Escape",
                "Ctrl+Shift+Escape",
                "Ctrl+Shift+Escape",
            ),
            (
                "+",
                KeyboardModifiers { alt: false, control: true, shift: false, meta: false },
                true,
                false,
                "⌘+",
                "Ctrl++",
                "Ctrl++",
            ),
            (
                "a",
                KeyboardModifiers { alt: true, control: true, shift: false, meta: false },
                false,
                true,
                "⌘A",
                "Ctrl+A",
                "Ctrl+A",
            ),
            (
                "",
                KeyboardModifiers { alt: false, control: true, shift: false, meta: false },
                false,
                false,
                "",
                "",
                "",
            ),
            (
                "\u{000a}",
                KeyboardModifiers { alt: false, control: false, shift: false, meta: false },
                false,
                false,
                "Return",
                "Return",
                "Return",
            ),
            (
                "\u{0009}",
                KeyboardModifiers { alt: false, control: false, shift: false, meta: false },
                false,
                false,
                "Tab",
                "Tab",
                "Tab",
            ),
            (
                "\u{0020}",
                KeyboardModifiers { alt: false, control: false, shift: false, meta: false },
                false,
                false,
                "Space",
                "Space",
                "Space",
            ),
            (
                "\u{0008}",
                KeyboardModifiers { alt: false, control: false, shift: false, meta: false },
                false,
                false,
                "Backspace",
                "Backspace",
                "Backspace",
            ),
        ];

        for (
            key,
            modifiers,
            ignore_shift,
            ignore_alt,
            _expected_macos,
            _expected_windows,
            _expected_linux,
        ) in test_cases
        {
            let shortcut = make_keyboard_shortcut(key.into(), modifiers, ignore_shift, ignore_alt);

            let result = shortcut.to_platform_string();

            #[cfg(target_os = "macos")]
            assert_eq!(result.as_str(), _expected_macos, "Failed for key: {:?}", key);

            #[cfg(target_os = "windows")]
            assert_eq!(result.as_str(), _expected_windows, "Failed for key: {:?}", key);

            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            assert_eq!(result.as_str(), _expected_linux, "Failed for key: {:?}", key);
        }
    }
}
