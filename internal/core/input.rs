// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*! Module handling mouse events
*/
#![warn(missing_docs)]

use crate::item_tree::ItemTreeRc;
use crate::item_tree::{ItemRc, ItemWeak, VisitChildrenResult};
use crate::items::{DropEvent, ItemRef, MouseCursor, OperatingSystemType, TextCursorDirection};
pub use crate::items::{FocusReason, KeyEvent, KeyboardModifiers, PointerEventButton};
use crate::lengths::{ItemTransform, LogicalPoint, LogicalVector};
use crate::timers::Timer;
use crate::window::{WindowAdapter, WindowInner};
use crate::{Coord, Property, SharedString};
use alloc::rc::Rc;
use alloc::vec::Vec;
use const_field_offset::FieldOffsets;
use core::cell::Cell;
use core::fmt::Display;
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
    /// `delta` is the incremental scale change; ScaleRotateGestureHandler accumulates it.
    PinchGesture { position: LogicalPoint, delta: f32, phase: TouchPhase },
    /// A platform-recognized rotation gesture (macOS/iOS trackpad, Qt).
    /// `delta` is the incremental rotation in degrees using the Slint convention:
    /// positive = clockwise. Backends must convert from their platform convention
    /// before constructing this event.
    RotationGesture { position: LogicalPoint, delta: f32, phase: TouchPhase },
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
            MouseEvent::PinchGesture { .. } | MouseEvent::RotationGesture { .. } => Some(true),
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

/// A `Keys` is created by the `@keys(...)` macro and
/// defines which key event(s) activate a KeyBinding.
#[derive(Clone, Eq, PartialEq, Default)]
#[repr(C)]
pub struct Keys {
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

/// Re-exported in private_unstable_api to create a Keys struct.
pub fn make_keys(
    key: SharedString,
    modifiers: KeyboardModifiers,
    ignore_shift: bool,
    ignore_alt: bool,
) -> Keys {
    Keys { key: key.to_lowercase().into(), modifiers, ignore_shift, ignore_alt }
}

#[cfg(feature = "ffi")]
#[allow(unsafe_code)]
pub(crate) mod ffi {
    use crate::api::ToSharedString as _;

    use super::*;

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_keys(
        key: &SharedString,
        alt: bool,
        control: bool,
        shift: bool,
        meta: bool,
        ignore_shift: bool,
        ignore_alt: bool,
        out: &mut Keys,
    ) {
        *out = make_keys(
            key.clone(),
            KeyboardModifiers { alt, control, shift, meta },
            ignore_shift,
            ignore_alt,
        );
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_keys_debug_string(shortcut: &Keys, out: &mut SharedString) {
        *out = crate::format!("{shortcut:?}");
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_keys_to_string(shortcut: &Keys, out: &mut SharedString) {
        *out = shortcut.to_shared_string();
    }
}

impl Keys {
    /// Check whether a `Keys` can be triggered by the given `KeyEvent`
    pub(crate) fn matches(&self, key_event: &KeyEvent) -> bool {
        // An empty Keys is never triggered, even if the modifiers match.
        if self.key.is_empty() {
            return false;
        }

        // TODO: Should this check the event_type and only match on KeyReleased?
        let mut expected_modifiers = self.modifiers;
        if self.ignore_shift {
            expected_modifiers.shift = key_event.modifiers.shift;
        }
        if self.ignore_alt {
            expected_modifiers.alt = key_event.modifiers.alt;
        }
        // Note: The shortcut's key is already in lowercase and NFC-normalized
        // (by the compiler and backends respectively), so we only need to
        // lowercase the event text. Backends are expected to NFC-normalize
        // key event text before dispatching.
        //
        // This improves our handling of CapsLock and Shift, as the event text will be in uppercase
        // if caps lock is active, even if shift is not pressed.
        let event_text = key_event.text.chars().flat_map(|character| character.to_lowercase());

        event_text.eq(self.key.chars()) && key_event.modifiers == expected_modifiers
    }

    fn format_key_for_display(&self) -> crate::SharedString {
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
            return key_str.to_uppercase().into();
        }

        key_str.into()
    }
}

impl Display for Keys {
    /// Converts the [`Keys`] to a string that looks native on the current platform.
    ///
    /// For example, the shortcut created with @keys(Meta + Control + A)
    /// will be converted like this:
    /// - **macOS**: `⌃⌘A`
    /// - **Windows**: `Win+Ctrl+A`
    /// - **Linux**: `Super+Ctrl+A`
    ///
    /// Note that this functions output is best-effort and may be adjusted/improved at any time,
    /// do not rely on this output to be stable!
    //
    // References for implementation
    // - macOS: <https://developer.apple.com/design/human-interface-guidelines/keyboards>
    // - Windows: <https://learn.microsoft.com/en-us/windows/apps/design/input/keyboard-accelerators>
    // - Linux: <https://developer.gnome.org/hig/guidelines/keyboard.html>
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.key.is_empty() {
            return Ok(());
        }

        if crate::is_apple_platform() {
            // Slint remaps modifiers on macOS: control → Command, meta → Control
            // From Apple's documentation:
            //
            // List modifier keys in the correct order.
            // If you use more than one modifier key in a custom shortcut, always list them in this order:
            //  Control, Option, Shift, Command
            if self.modifiers.meta {
                f.write_str("⌃")?;
            }
            if !self.ignore_alt && self.modifiers.alt {
                f.write_str("⌥")?;
            }
            if !self.ignore_shift && self.modifiers.shift {
                f.write_str("⇧")?;
            }
            if self.modifiers.control {
                f.write_str("⌘")?;
            }
        } else {
            let separator = "+";

            // TODO: These should probably be translated, but better to have at least
            // platform-local names than nothing.
            let (ctrl_str, alt_str, shift_str, meta_str) =
                if crate::detect_operating_system() == OperatingSystemType::Windows {
                    ("Ctrl", "Alt", "Shift", "Win")
                } else {
                    ("Ctrl", "Alt", "Shift", "Super")
                };

            if self.modifiers.meta {
                f.write_str(meta_str)?;
                f.write_str(separator)?;
            }
            if self.modifiers.control {
                f.write_str(ctrl_str)?;
                f.write_str(separator)?;
            }
            if !self.ignore_alt && self.modifiers.alt {
                f.write_str(alt_str)?;
                f.write_str(separator)?;
            }
            if !self.ignore_shift && self.modifiers.shift {
                f.write_str(shift_str)?;
                f.write_str(separator)?;
            }
        }
        f.write_str(&self.format_key_for_display())
    }
}

impl core::fmt::Debug for Keys {
    /// Formats the keyboard shortcut so that the output would be accepted by the @keys macro in Slint.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Make sure to keep this in sync with the implemenation in compiler/langtype.rs
        if self.key.is_empty() {
            write!(f, "")
        } else {
            let alt = self
                .ignore_alt
                .then_some("Alt?+")
                .or(self.modifiers.alt.then_some("Alt+"))
                .unwrap_or_default();
            let ctrl = if self.modifiers.control { "Control+" } else { "" };
            let meta = if self.modifiers.meta { "Meta+" } else { "" };
            let shift = self
                .ignore_shift
                .then_some("Shift?+")
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

#[derive(Default)]
/// This struct is used to pass key events to the runtime.
pub struct InternalKeyEvent {
    /// That's the public type with only public fields
    pub key_event: KeyEvent,
    /// Indicates whether the key was pressed or released
    pub event_type: KeyEventType,
    /// If the event type is KeyEventType::UpdateComposition or KeyEventType::CommitComposition,
    /// then this field specifies what part of the current text to replace.
    /// Relative to the offset of the pre-edit text within the text input element's text.
    pub replacement_range: Option<core::ops::Range<i32>>,
    /// If the event type is KeyEventType::UpdateComposition, this is the new pre-edit text
    pub preedit_text: SharedString,
    /// The selection within the preedit_text
    pub preedit_selection: Option<core::ops::Range<i32>>,
    /// The new cursor position, when None, the cursor is put after the text that was just inserted
    pub cursor_position: Option<i32>,
    /// The anchor position, when None, the cursor is put after the text that was just inserted
    pub anchor_position: Option<i32>,
}

impl InternalKeyEvent {
    /// If a shortcut was pressed, this function returns `Some(StandardShortcut)`.
    /// Otherwise it returns None.
    pub fn shortcut(&self) -> Option<StandardShortcut> {
        if self.key_event.modifiers.control && !self.key_event.modifiers.shift {
            match self.key_event.text.as_str() {
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
        } else if self.key_event.modifiers.control && self.key_event.modifiers.shift {
            match self.key_event.text.as_str() {
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
        let ke = &self.key_event;
        let keycode = ke.text.chars().next()?;

        let is_apple = crate::is_apple_platform();

        let move_mod = if is_apple {
            ke.modifiers.alt && !ke.modifiers.control && !ke.modifiers.meta
        } else {
            ke.modifiers.control && !ke.modifiers.alt && !ke.modifiers.meta
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
            if ke.modifiers.control && !ke.modifiers.alt && !ke.modifiers.meta {
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

        if is_apple && ke.modifiers.control {
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
        if window_adapter.renderer().supports_transformations()
            && let Some(inverse_transform) = item.inverse_children_transform()
        {
            event.transform(inverse_transform);
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
            if window_adapter.renderer().supports_transformations()
                && let Some(inverse_transform) = item.inverse_children_transform()
            {
                *p = inverse_transform.transform_point(p.cast()).cast();
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

    if let MouseEvent::Wheel { position, .. } = mouse_event
        && r.has_aborted()
    {
        // An accepted wheel event might have moved things. Send a move event at the position to reset the has-hover
        return process_mouse_input(
            root,
            &MouseEvent::Moved { position: *position, is_touch: false },
            window_adapter,
            result,
        );
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

/// A single active touch point.
#[derive(Clone, Copy, Default)]
struct TouchPoint {
    id: u64,
    position: LogicalPoint,
}

/// Fixed-capacity map of touch IDs to touch points.
///
/// Touchscreens rarely report more than 5 simultaneous contacts, and gesture
/// recognition only uses the first two. A linear-scan array avoids the heap
/// allocation and pointer-chasing overhead of `BTreeMap` for this tiny collection.
const MAX_TRACKED_TOUCHES: usize = 5;

#[derive(Clone)]
struct TouchMap {
    entries: [TouchPoint; MAX_TRACKED_TOUCHES],
    len: usize,
}

impl Default for TouchMap {
    fn default() -> Self {
        Self { entries: [TouchPoint::default(); MAX_TRACKED_TOUCHES], len: 0 }
    }
}

impl TouchMap {
    fn get(&self, id: u64) -> Option<&TouchPoint> {
        self.entries[..self.len].iter().find(|tp| tp.id == id)
    }

    fn get_mut(&mut self, id: u64) -> Option<&mut TouchPoint> {
        self.entries[..self.len].iter_mut().find(|tp| tp.id == id)
    }

    fn insert(&mut self, point: TouchPoint) {
        if let Some(existing) = self.entries[..self.len].iter_mut().find(|tp| tp.id == point.id) {
            *existing = point;
        } else if self.len < MAX_TRACKED_TOUCHES {
            self.entries[self.len] = point;
            self.len += 1;
        }
    }

    fn remove(&mut self, id: u64) {
        if let Some(idx) = self.entries[..self.len].iter().position(|tp| tp.id == id) {
            self.len -= 1;
            self.entries[idx] = self.entries[self.len];
        }
    }

    fn len(&self) -> usize {
        self.len
    }

    /// Returns the first two distinct IDs, or `None` if fewer than 2 entries.
    fn first_two_ids(&self) -> Option<(u64, u64)> {
        if self.len >= 2 { Some((self.entries[0].id, self.entries[1].id)) } else { None }
    }

    /// Returns the first entry, if any.
    fn first(&self) -> Option<&TouchPoint> {
        if self.len > 0 { Some(&self.entries[0]) } else { None }
    }
}

/// Fixed-capacity buffer for [`MouseEvent`]s produced by the touch state machine.
///
/// No branch in [`TouchState::process`] emits more than 3 events (gesture end
/// produces PinchEnded + RotationEnded + Pressed/Exit). Capacity 4 provides a
/// margin without heap allocation.
const MAX_TOUCH_EVENTS: usize = 4;

#[derive(Clone)]
pub(crate) struct TouchEventBuffer {
    events: [Option<MouseEvent>; MAX_TOUCH_EVENTS],
    len: usize,
}

impl TouchEventBuffer {
    fn new() -> Self {
        Self { events: [None, None, None, None], len: 0 }
    }

    fn push(&mut self, event: MouseEvent) {
        debug_assert!(self.len < MAX_TOUCH_EVENTS, "TouchEventBuffer overflow");
        if self.len < MAX_TOUCH_EVENTS {
            self.events[self.len] = Some(event);
            self.len += 1;
        }
    }

    /// Returns an iterator over the buffered events.
    pub(crate) fn into_iter(self) -> impl Iterator<Item = MouseEvent> {
        let len = self.len;
        self.events.into_iter().take(len).flatten()
    }
}

/// State of the multi-touch gesture recognizer.
#[derive(Default, Debug, Clone, Copy)]
enum GestureRecognitionState {
    /// 0-1 fingers; forwarding as mouse events.
    #[default]
    Idle,
    /// 2 fingers down, waiting for movement to exceed threshold.
    TwoFingersDown { finger_ids: (u64, u64), initial_distance: f32, last_angle: euclid::Angle<f32> },
    /// Actively synthesizing PinchGesture/RotationGesture events.
    Pinching {
        finger_ids: (u64, u64),
        initial_distance: f32,
        last_scale: f32,
        last_angle: euclid::Angle<f32>,
    },
}

/// Tracks all active touch points and recognizes pinch/rotation gestures.
///
/// When only one finger is down, touch events are forwarded as mouse events.
/// When two fingers are down and move beyond a threshold, synthesized
/// `PinchGesture` and `RotationGesture` events are emitted — the same events
/// that platform gesture recognition (e.g. macOS trackpad) produces.
pub(crate) struct TouchState {
    active_touches: TouchMap,
    /// The finger forwarded as mouse events during single-touch.
    primary_touch_id: Option<u64>,
    gesture_state: GestureRecognitionState,
}

impl Default for TouchState {
    fn default() -> Self {
        Self {
            active_touches: TouchMap::default(),
            primary_touch_id: None,
            gesture_state: GestureRecognitionState::Idle,
        }
    }
}

impl TouchState {
    /// Minimum movement (in logical pixels) before two fingers are recognized as a pinch.
    const PINCH_THRESHOLD: f32 = 8.0;

    /// Minimum angular change (in degrees) before two fingers are recognized as a rotation.
    const ROTATION_THRESHOLD: f32 = 5.0;

    /// Returns the finger IDs from the current gesture state, if any.
    fn gesture_finger_ids(&self) -> Option<(u64, u64)> {
        match self.gesture_state {
            GestureRecognitionState::TwoFingersDown { finger_ids, .. }
            | GestureRecognitionState::Pinching { finger_ids, .. } => Some(finger_ids),
            GestureRecognitionState::Idle => None,
        }
    }

    /// Returns (distance, angle) between two specific touch points.
    fn geometry_for(&self, (id_a, id_b): (u64, u64)) -> Option<(f32, euclid::Angle<f32>)> {
        let a = self.active_touches.get(id_a)?;
        let b = self.active_touches.get(id_b)?;
        let delta = (b.position - a.position).cast::<f32>();
        Some((delta.length(), delta.angle_from_x_axis()))
    }

    /// Returns the positions of the two gesture fingers, or `None` if not available.
    fn gesture_finger_positions(&self) -> Option<(&TouchPoint, &TouchPoint)> {
        let (id_a, id_b) = self.gesture_finger_ids()?;
        let a = self.active_touches.get(id_a)?;
        let b = self.active_touches.get(id_b)?;
        Some((a, b))
    }

    /// Returns the midpoint between the two gesture fingers, or `None`.
    fn gesture_midpoint(&self) -> Option<LogicalPoint> {
        let (a, b) = self.gesture_finger_positions()?;
        let mid = a.position.cast::<f32>().lerp(b.position.cast::<f32>(), 0.5);
        Some(mid.cast())
    }

    /// Returns (distance, angle) between the two gesture fingers.
    fn gesture_geometry(&self) -> Option<(f32, euclid::Angle<f32>)> {
        let (a, b) = self.gesture_finger_positions()?;
        let delta = (b.position - a.position).cast::<f32>();
        Some((delta.length(), delta.angle_from_x_axis()))
    }

    /// Returns true if the given touch ID is one of the two gesture fingers.
    fn is_gesture_finger(&self, id: u64) -> bool {
        self.gesture_finger_ids().is_some_and(|(a, b)| id == a || id == b)
    }

    /// Run the touch state machine for a single event and return the
    /// [`MouseEvent`]s to dispatch.
    ///
    /// This is intentionally separated from [`crate::window::WindowInner::process_touch_input`]
    /// so that the `RefCell` borrow can be dropped *once* before dispatching,
    /// rather than requiring a manual `drop` at every branch.
    pub(crate) fn process(
        &mut self,
        id: u64,
        position: LogicalPoint,
        phase: TouchPhase,
    ) -> TouchEventBuffer {
        let mut events = TouchEventBuffer::new();
        match phase {
            TouchPhase::Started => self.process_started(id, position, &mut events),
            TouchPhase::Moved => self.process_moved(id, position, &mut events),
            TouchPhase::Ended => self.process_ended(id, position, false, &mut events),
            TouchPhase::Cancelled => self.process_ended(id, position, true, &mut events),
        }
        events
    }

    fn process_started(&mut self, id: u64, position: LogicalPoint, events: &mut TouchEventBuffer) {
        self.active_touches.insert(TouchPoint { id, position });

        let total = self.active_touches.len();
        if total == 1 {
            // First finger: become primary, forward as mouse press.
            self.primary_touch_id = Some(id);
            self.gesture_state = GestureRecognitionState::Idle;
            events.push(MouseEvent::Pressed {
                position,
                button: PointerEventButton::Left,
                click_count: 0,
                is_touch: true,
            });
        } else if total == 2 {
            // Second finger: transition Idle → TwoFingersDown.
            let finger_ids = self.active_touches.first_two_ids().unwrap_or((0, 0));

            // Synthesize a Release for the primary finger to clear any
            // Flickable grab / delay state.
            let primary_pos = self
                .primary_touch_id
                .and_then(|pid| self.active_touches.get(pid))
                .map(|tp| tp.position)
                .unwrap_or(position);

            // Compute initial geometry for threshold detection.
            let (initial_distance, last_angle) =
                self.geometry_for(finger_ids).unwrap_or((0.0, euclid::Angle::zero()));
            self.gesture_state = GestureRecognitionState::TwoFingersDown {
                finger_ids,
                initial_distance,
                last_angle,
            };

            events.push(MouseEvent::Released {
                position: primary_pos,
                button: PointerEventButton::Left,
                click_count: 0,
                is_touch: true,
            });
        }
        // 3+ fingers: tracked in active_touches but ignored for gesture.
    }

    fn process_moved(&mut self, id: u64, position: LogicalPoint, events: &mut TouchEventBuffer) {
        if let Some(tp) = self.active_touches.get_mut(id) {
            tp.position = position;
        }

        let is_gesture_finger = self.is_gesture_finger(id);

        match self.gesture_state {
            GestureRecognitionState::Idle => {
                if self.primary_touch_id == Some(id) {
                    events.push(MouseEvent::Moved { position, is_touch: true });
                }
            }
            GestureRecognitionState::TwoFingersDown {
                finger_ids,
                initial_distance,
                last_angle,
            } if is_gesture_finger => {
                if let Some((dist, angle)) = self.gesture_geometry() {
                    let delta_dist = (dist - initial_distance).abs();
                    let delta_angle = (angle - last_angle).signed().to_degrees().abs();
                    if delta_dist > Self::PINCH_THRESHOLD || delta_angle > Self::ROTATION_THRESHOLD
                    {
                        // Re-snapshot so the first gesture event starts from
                        // the current geometry rather than accumulating the
                        // threshold movement.
                        self.gesture_state = GestureRecognitionState::Pinching {
                            finger_ids,
                            initial_distance: dist,
                            last_scale: 1.0,
                            last_angle: angle,
                        };

                        let midpoint = self.gesture_midpoint().unwrap_or(position);

                        events.push(MouseEvent::PinchGesture {
                            position: midpoint,
                            delta: 0.0,
                            phase: TouchPhase::Started,
                        });
                        events.push(MouseEvent::RotationGesture {
                            position: midpoint,
                            delta: 0.0,
                            phase: TouchPhase::Started,
                        });
                    }
                }
            }
            GestureRecognitionState::Pinching {
                initial_distance, last_scale, last_angle, ..
            } if is_gesture_finger => {
                if let Some((dist, angle)) = self.gesture_geometry() {
                    let midpoint = self.gesture_midpoint().unwrap_or(position);

                    let current_scale =
                        if initial_distance > 0.0 { dist / initial_distance } else { 1.0 };
                    let scale_delta = current_scale - last_scale;

                    // `.signed()` wraps to [-pi, pi] so crossing the ±180°
                    // atan2 boundary doesn't produce a full-revolution jump.
                    let rotation_delta = (angle - last_angle).signed().to_degrees();

                    // Update the mutable state for next frame.
                    if let GestureRecognitionState::Pinching {
                        last_scale: ref mut ls,
                        last_angle: ref mut la,
                        ..
                    } = self.gesture_state
                    {
                        *ls = current_scale;
                        *la = angle;
                    }

                    events.push(MouseEvent::PinchGesture {
                        position: midpoint,
                        delta: scale_delta,
                        phase: TouchPhase::Moved,
                    });
                    events.push(MouseEvent::RotationGesture {
                        position: midpoint,
                        delta: rotation_delta,
                        phase: TouchPhase::Moved,
                    });
                }
            }
            _ => {}
        }
    }

    fn process_ended(
        &mut self,
        id: u64,
        position: LogicalPoint,
        is_cancelled: bool,
        events: &mut TouchEventBuffer,
    ) {
        // Check gesture membership *before* removing from the map.
        let is_gesture_finger = self.is_gesture_finger(id);
        let midpoint = self.gesture_midpoint().unwrap_or(position);
        self.active_touches.remove(id);

        match self.gesture_state {
            GestureRecognitionState::Idle => {
                if self.primary_touch_id == Some(id) {
                    self.primary_touch_id = None;
                    events.push(MouseEvent::Released {
                        position,
                        button: PointerEventButton::Left,
                        click_count: 0,
                        is_touch: true,
                    });
                    events.push(MouseEvent::Exit);
                }
            }
            GestureRecognitionState::TwoFingersDown { .. } if is_gesture_finger => {
                self.gesture_state = GestureRecognitionState::Idle;
                if !is_cancelled {
                    if let Some(remaining) = self.active_touches.first() {
                        let remaining_pos = remaining.position;
                        self.primary_touch_id = Some(remaining.id);
                        events.push(MouseEvent::Pressed {
                            position: remaining_pos,
                            button: PointerEventButton::Left,
                            click_count: 0,
                            is_touch: true,
                        });
                    } else {
                        self.primary_touch_id = None;
                        events.push(MouseEvent::Exit);
                    }
                } else {
                    self.primary_touch_id = None;
                    events.push(MouseEvent::Exit);
                }
            }
            GestureRecognitionState::Pinching { .. } if is_gesture_finger => {
                self.gesture_state = GestureRecognitionState::Idle;

                let gesture_phase =
                    if is_cancelled { TouchPhase::Cancelled } else { TouchPhase::Ended };

                let remaining = if !is_cancelled {
                    self.active_touches.first().map(|tp| (tp.id, tp.position))
                } else {
                    None
                };
                if let Some((rid, _)) = remaining {
                    self.primary_touch_id = Some(rid);
                } else {
                    self.primary_touch_id = None;
                }

                events.push(MouseEvent::PinchGesture {
                    position: midpoint,
                    delta: 0.0,
                    phase: gesture_phase,
                });
                events.push(MouseEvent::RotationGesture {
                    position: midpoint,
                    delta: 0.0,
                    phase: gesture_phase,
                });

                if let Some((_, rpos)) = remaining {
                    events.push(MouseEvent::Pressed {
                        position: rpos,
                        button: PointerEventButton::Left,
                        click_count: 0,
                        is_touch: true,
                    });
                } else {
                    events.push(MouseEvent::Exit);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod touch_tests {
    extern crate alloc;
    use alloc::vec;
    use alloc::vec::Vec;

    use super::*;
    use crate::lengths::LogicalPoint;

    fn pt(x: f32, y: f32) -> LogicalPoint {
        euclid::point2(x, y)
    }

    // -----------------------------------------------------------------------
    // TouchMap tests
    // -----------------------------------------------------------------------

    #[test]
    fn touch_map_insert_and_get() {
        let mut map = TouchMap::default();
        assert_eq!(map.len(), 0);
        map.insert(TouchPoint { id: 1, position: pt(10.0, 20.0) });
        assert_eq!(map.len(), 1);
        assert!(map.get(1).is_some());
        assert!((map.get(1).unwrap().position.x - 10.0).abs() < f32::EPSILON);
        assert!(map.get(2).is_none());
    }

    #[test]
    fn touch_map_update_existing() {
        let mut map = TouchMap::default();
        map.insert(TouchPoint { id: 1, position: pt(10.0, 20.0) });
        map.insert(TouchPoint { id: 1, position: pt(30.0, 40.0) });
        assert_eq!(map.len(), 1);
        assert!((map.get(1).unwrap().position.x - 30.0).abs() < f32::EPSILON);
    }

    #[test]
    fn touch_map_remove() {
        let mut map = TouchMap::default();
        map.insert(TouchPoint { id: 1, position: pt(10.0, 20.0) });
        map.insert(TouchPoint { id: 2, position: pt(30.0, 40.0) });
        assert_eq!(map.len(), 2);
        map.remove(1);
        assert_eq!(map.len(), 1);
        assert!(map.get(1).is_none());
        assert!(map.get(2).is_some());
    }

    #[test]
    fn touch_map_remove_nonexistent() {
        let mut map = TouchMap::default();
        map.insert(TouchPoint { id: 1, position: pt(10.0, 20.0) });
        map.remove(99);
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn touch_map_capacity() {
        let mut map = TouchMap::default();
        for i in 0..MAX_TRACKED_TOUCHES {
            map.insert(TouchPoint { id: i as u64, position: pt(i as f32, 0.0) });
        }
        assert_eq!(map.len(), MAX_TRACKED_TOUCHES);
        // Inserting beyond capacity is silently ignored.
        map.insert(TouchPoint { id: 99, position: pt(99.0, 0.0) });
        assert_eq!(map.len(), MAX_TRACKED_TOUCHES);
        assert!(map.get(99).is_none());
    }

    #[test]
    fn touch_map_first_two_ids() {
        let mut map = TouchMap::default();
        assert!(map.first_two_ids().is_none());
        map.insert(TouchPoint { id: 5, position: pt(0.0, 0.0) });
        assert!(map.first_two_ids().is_none());
        map.insert(TouchPoint { id: 10, position: pt(0.0, 0.0) });
        assert_eq!(map.first_two_ids(), Some((5, 10)));
    }

    #[test]
    fn touch_map_first() {
        let mut map = TouchMap::default();
        assert!(map.first().is_none());
        map.insert(TouchPoint { id: 7, position: pt(1.0, 2.0) });
        let tp = map.first().unwrap();
        assert_eq!(tp.id, 7);
        assert!((tp.position.x - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn touch_map_get_mut() {
        let mut map = TouchMap::default();
        map.insert(TouchPoint { id: 1, position: pt(0.0, 0.0) });
        map.get_mut(1).unwrap().position = pt(5.0, 6.0);
        assert!((map.get(1).unwrap().position.x - 5.0).abs() < f32::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Helper: extract event types for readable assertions
    // -----------------------------------------------------------------------

    #[derive(Debug, PartialEq)]
    enum Ev {
        Pressed(f32, f32),
        Released(f32, f32),
        Moved(f32, f32),
        Exit,
        PinchStarted,
        PinchMoved(f32),
        PinchEnded,
        PinchCancelled,
        RotationStarted,
        RotationMoved(f32),
        RotationEnded,
        RotationCancelled,
    }

    fn classify(events: &TouchEventBuffer) -> Vec<Ev> {
        events
            .clone()
            .into_iter()
            .map(|e| match e {
                MouseEvent::Pressed { position, .. } => Ev::Pressed(position.x, position.y),
                MouseEvent::Released { position, .. } => Ev::Released(position.x, position.y),
                MouseEvent::Moved { position, .. } => Ev::Moved(position.x, position.y),
                MouseEvent::Exit => Ev::Exit,
                MouseEvent::PinchGesture { delta, phase, .. } => match phase {
                    TouchPhase::Started => Ev::PinchStarted,
                    TouchPhase::Moved => Ev::PinchMoved(delta),
                    TouchPhase::Ended => Ev::PinchEnded,
                    TouchPhase::Cancelled => Ev::PinchCancelled,
                },
                MouseEvent::RotationGesture { delta, phase, .. } => match phase {
                    TouchPhase::Started => Ev::RotationStarted,
                    TouchPhase::Moved => Ev::RotationMoved(delta),
                    TouchPhase::Ended => Ev::RotationEnded,
                    TouchPhase::Cancelled => Ev::RotationCancelled,
                },
                _ => panic!("unexpected event: {:?}", e),
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // TouchState: single-finger forwarding
    // -----------------------------------------------------------------------

    #[test]
    fn single_finger_press_move_release() {
        let mut state = TouchState::default();

        let evs = state.process(1, pt(100.0, 200.0), TouchPhase::Started);
        assert_eq!(classify(&evs), vec![Ev::Pressed(100.0, 200.0)]);

        let evs = state.process(1, pt(110.0, 200.0), TouchPhase::Moved);
        assert_eq!(classify(&evs), vec![Ev::Moved(110.0, 200.0)]);

        let evs = state.process(1, pt(110.0, 200.0), TouchPhase::Ended);
        assert_eq!(classify(&evs), vec![Ev::Released(110.0, 200.0), Ev::Exit]);
    }

    #[test]
    fn single_finger_cancel() {
        let mut state = TouchState::default();

        state.process(1, pt(100.0, 200.0), TouchPhase::Started);

        let evs = state.process(1, pt(100.0, 200.0), TouchPhase::Cancelled);
        assert_eq!(classify(&evs), vec![Ev::Released(100.0, 200.0), Ev::Exit]);
    }

    #[test]
    fn non_primary_move_ignored() {
        let mut state = TouchState::default();
        // Touch 1 is primary.
        state.process(1, pt(100.0, 200.0), TouchPhase::Started);

        // Move for a different ID that was never started (edge case).
        let evs = state.process(99, pt(50.0, 50.0), TouchPhase::Moved);
        assert!(classify(&evs).is_empty());
    }

    // -----------------------------------------------------------------------
    // TouchState: two-finger → gesture transition
    // -----------------------------------------------------------------------

    #[test]
    fn two_fingers_synthesize_release_then_gesture() {
        let mut state = TouchState::default();

        // Finger 1 down.
        let evs = state.process(1, pt(100.0, 200.0), TouchPhase::Started);
        assert_eq!(classify(&evs), vec![Ev::Pressed(100.0, 200.0)]);

        // Finger 2 down → synthesized release for finger 1.
        let evs = state.process(2, pt(200.0, 200.0), TouchPhase::Started);
        assert_eq!(classify(&evs), vec![Ev::Released(100.0, 200.0)]);
        assert!(matches!(state.gesture_state, GestureRecognitionState::TwoFingersDown { .. }));

        // Move finger 2 far enough to trigger pinch (> 8px threshold).
        let evs = state.process(2, pt(220.0, 200.0), TouchPhase::Moved);
        assert_eq!(classify(&evs), vec![Ev::PinchStarted, Ev::RotationStarted]);
        assert!(matches!(state.gesture_state, GestureRecognitionState::Pinching { .. }));
    }

    #[test]
    fn two_fingers_below_threshold_no_gesture() {
        let mut state = TouchState::default();

        state.process(1, pt(100.0, 200.0), TouchPhase::Started);
        state.process(2, pt(200.0, 200.0), TouchPhase::Started);

        // Small movement within threshold.
        let evs = state.process(2, pt(202.0, 200.0), TouchPhase::Moved);
        assert!(classify(&evs).is_empty());
        assert!(matches!(state.gesture_state, GestureRecognitionState::TwoFingersDown { .. }));
    }

    #[test]
    fn pinch_produces_scale_deltas() {
        let mut state = TouchState::default();

        // Set up: finger 1 at (0, 0), finger 2 at (100, 0) → distance = 100.
        state.process(1, pt(0.0, 0.0), TouchPhase::Started);
        state.process(2, pt(100.0, 0.0), TouchPhase::Started);

        // Move finger 2 to (120, 0) to exceed threshold and start pinching.
        state.process(2, pt(120.0, 0.0), TouchPhase::Moved);
        assert!(matches!(state.gesture_state, GestureRecognitionState::Pinching { .. }));

        // Now move finger 2 further to (180, 0).
        // New distance = 180, initial distance (re-snapshotted) = 120.
        // Scale = 180/120 = 1.5, delta = 1.5 - 1.0 = 0.5.
        let evs = state.process(2, pt(180.0, 0.0), TouchPhase::Moved);
        let classified = classify(&evs);
        assert_eq!(classified.len(), 2);
        if let Ev::PinchMoved(delta) = classified[0] {
            assert!((delta - 0.5).abs() < 0.01, "expected ~0.5, got {}", delta);
        } else {
            panic!("expected PinchMoved, got {:?}", classified[0]);
        }
    }

    #[test]
    fn rotation_produces_correct_deltas() {
        let mut state = TouchState::default();

        // Finger 1 at origin, finger 2 on the X axis at (100, 0).
        // Initial angle = atan2(0, 100) = 0°.
        state.process(1, pt(0.0, 0.0), TouchPhase::Started);
        state.process(2, pt(100.0, 0.0), TouchPhase::Started);

        // Move finger 2 far enough to trigger gesture.
        state.process(2, pt(120.0, 0.0), TouchPhase::Moved);
        assert!(matches!(state.gesture_state, GestureRecognitionState::Pinching { .. }));

        // Rotate ~45° clockwise: move finger 2 from (120, 0) to roughly
        // (70.7, 70.7) which is at 45° from origin.
        // atan2(70.7, 70.7) ≈ 45°. Delta from re-snapshotted 0° = +45°.
        // Slint convention: positive = clockwise → delta ≈ +45°.
        let evs = state.process(2, pt(70.7, 70.7), TouchPhase::Moved);
        let classified = classify(&evs);
        assert_eq!(classified.len(), 2);
        if let Ev::RotationMoved(delta) = classified[1] {
            assert!((delta - 45.0).abs() < 1.0, "expected ~45.0 (clockwise), got {}", delta);
        } else {
            panic!("expected RotationMoved, got {:?}", classified[1]);
        }
    }

    #[test]
    fn rotation_across_180_degree_boundary() {
        let mut state = TouchState::default();

        // Finger 1 at origin, finger 2 at (-100, -10).
        // angle = atan2(-10, -100) ≈ -174.3°.
        state.process(1, pt(0.0, 0.0), TouchPhase::Started);
        state.process(2, pt(-100.0, -10.0), TouchPhase::Started);

        // Trigger gesture by moving far enough.
        state.process(2, pt(-120.0, -10.0), TouchPhase::Moved);
        assert!(matches!(state.gesture_state, GestureRecognitionState::Pinching { .. }));

        // Rotate across the ±180° boundary: move finger 2 to (-100, 10).
        // New angle = atan2(10, -100) ≈ 174.3°.
        // Raw angular change crosses ±180°, but per-frame delta should be
        // small (~11.4° which is 2 * 5.7°), NOT a ~349° jump.
        let evs = state.process(2, pt(-100.0, 10.0), TouchPhase::Moved);
        let classified = classify(&evs);
        if let Ev::RotationMoved(delta) = classified[1] {
            assert!(
                delta.abs() < 20.0,
                "rotation should be a small delta (~11°), got {} (discontinuity!)",
                delta
            );
        } else {
            panic!("expected RotationMoved, got {:?}", classified[1]);
        }
    }

    // -----------------------------------------------------------------------
    // TouchState: gesture end transitions
    // -----------------------------------------------------------------------

    #[test]
    fn pinch_end_with_remaining_finger() {
        let mut state = TouchState::default();

        state.process(1, pt(0.0, 0.0), TouchPhase::Started);
        state.process(2, pt(100.0, 0.0), TouchPhase::Started);
        // Trigger pinch.
        state.process(2, pt(120.0, 0.0), TouchPhase::Moved);

        // Lift finger 2 → gesture ends, finger 1 gets re-pressed.
        let evs = state.process(2, pt(120.0, 0.0), TouchPhase::Ended);
        let classified = classify(&evs);
        assert_eq!(classified, vec![Ev::PinchEnded, Ev::RotationEnded, Ev::Pressed(0.0, 0.0)]);
        assert!(matches!(state.gesture_state, GestureRecognitionState::Idle));
        assert_eq!(state.primary_touch_id, Some(1));
    }

    #[test]
    fn pinch_cancel_emits_cancelled_and_exit() {
        let mut state = TouchState::default();

        state.process(1, pt(0.0, 0.0), TouchPhase::Started);
        state.process(2, pt(100.0, 0.0), TouchPhase::Started);
        state.process(2, pt(120.0, 0.0), TouchPhase::Moved);

        // Cancel finger 2.
        let evs = state.process(2, pt(120.0, 0.0), TouchPhase::Cancelled);
        let classified = classify(&evs);
        assert_eq!(classified, vec![Ev::PinchCancelled, Ev::RotationCancelled, Ev::Exit]);
        assert!(state.primary_touch_id.is_none());
    }

    #[test]
    fn two_fingers_down_lift_before_threshold_returns_to_idle() {
        let mut state = TouchState::default();

        state.process(1, pt(100.0, 200.0), TouchPhase::Started);
        state.process(2, pt(200.0, 200.0), TouchPhase::Started);
        assert!(matches!(state.gesture_state, GestureRecognitionState::TwoFingersDown { .. }));

        // Lift finger 2 without exceeding movement threshold.
        let evs = state.process(2, pt(200.0, 200.0), TouchPhase::Ended);
        let classified = classify(&evs);
        // Remaining finger 1 gets re-pressed.
        assert_eq!(classified, vec![Ev::Pressed(100.0, 200.0)]);
        assert!(matches!(state.gesture_state, GestureRecognitionState::Idle));
        assert_eq!(state.primary_touch_id, Some(1));
    }

    #[test]
    fn two_fingers_down_cancel_both_emits_exit() {
        let mut state = TouchState::default();

        state.process(1, pt(100.0, 200.0), TouchPhase::Started);
        state.process(2, pt(200.0, 200.0), TouchPhase::Started);

        // Cancel finger 2 (gesture finger, no remaining → Exit).
        let evs = state.process(2, pt(200.0, 200.0), TouchPhase::Cancelled);
        assert_eq!(classify(&evs), vec![Ev::Exit]);

        // Cancel finger 1 (now in Idle, but not primary since cancel cleared it).
        let evs = state.process(1, pt(100.0, 200.0), TouchPhase::Cancelled);
        assert!(classify(&evs).is_empty());
    }

    // -----------------------------------------------------------------------
    // TouchState: 3+ fingers
    // -----------------------------------------------------------------------

    #[test]
    fn third_finger_ignored_for_gesture() {
        let mut state = TouchState::default();

        state.process(1, pt(0.0, 0.0), TouchPhase::Started);
        state.process(2, pt(100.0, 0.0), TouchPhase::Started);

        // Third finger: no additional events.
        let evs = state.process(3, pt(50.0, 50.0), TouchPhase::Started);
        assert!(classify(&evs).is_empty());
        assert_eq!(state.active_touches.len(), 3);
    }

    // -----------------------------------------------------------------------
    // Angle wrapping via Euclid
    // -----------------------------------------------------------------------

    #[test]
    fn euclid_angle_signed_wrapping() {
        use euclid::Angle;
        let wrap = |deg: f32| Angle::degrees(deg).signed().to_degrees();
        assert!(wrap(0.0).abs() < f32::EPSILON);
        assert!((wrap(180.0) - 180.0).abs() < 0.01);
        assert!((wrap(181.0) - (-179.0)).abs() < 0.01);
        assert!((wrap(-181.0) - 179.0).abs() < 0.01);
        assert!(wrap(360.0).abs() < 0.01);
    }

    #[test]
    fn zero_distance_fingers_no_division_by_zero() {
        let mut state = TouchState::default();

        // Two fingers at the exact same position → distance = 0.
        state.process(1, pt(100.0, 100.0), TouchPhase::Started);
        state.process(2, pt(100.0, 100.0), TouchPhase::Started);
        assert!(matches!(state.gesture_state, GestureRecognitionState::TwoFingersDown { .. }));

        // Move one finger far enough to trigger gesture.
        let evs = state.process(2, pt(120.0, 100.0), TouchPhase::Moved);
        assert!(matches!(state.gesture_state, GestureRecognitionState::Pinching { .. }));
        let classified = classify(&evs);
        assert_eq!(classified.len(), 2);
        assert_eq!(classified[0], Ev::PinchStarted);

        // Move further — scale should not be inf/NaN despite initial_distance
        // having been 0 (re-snapshotted to 20.0 at threshold crossing).
        let evs = state.process(2, pt(140.0, 100.0), TouchPhase::Moved);
        let classified = classify(&evs);
        if let Ev::PinchMoved(delta) = classified[0] {
            assert!(delta.is_finite(), "scale delta should be finite, got {}", delta);
        } else {
            panic!("expected PinchMoved, got {:?}", classified[0]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate alloc;

    #[test]
    fn test_to_string() {
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
            let shortcut = make_keys(key.into(), modifiers, ignore_shift, ignore_alt);

            use crate::alloc::string::ToString;
            let result = shortcut.to_string();

            #[cfg(target_os = "macos")]
            assert_eq!(result.as_str(), _expected_macos, "Failed for key: {:?}", key);

            #[cfg(target_os = "windows")]
            assert_eq!(result.as_str(), _expected_windows, "Failed for key: {:?}", key);

            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            assert_eq!(result.as_str(), _expected_linux, "Failed for key: {:?}", key);
        }
    }
}
