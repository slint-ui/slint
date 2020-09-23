/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*! Module handling mouse events
*/
#![warn(missing_docs)]

use crate::component::ComponentRefPin;
use crate::graphics::Point;
use crate::item_tree::{ItemVisitorResult, VisitChildrenResult};
use euclid::default::Vector2D;
use sixtyfps_corelib_macros::*;
use std::convert::TryFrom;

/// The type of a MouseEvent
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseEventType {
    /// The mouse was pressed
    MousePressed,
    /// The mouse was relased
    MouseReleased,
    /// The mouse position has changed
    MouseMoved,
    /// The mouse exited the item or component
    MouseExit,
}

/// Structur representing a mouse event
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    /// The position of the cursor
    pub pos: Point,
    /// The action performed (pressed/released/moced)
    pub what: MouseEventType,
}

/// This value is returned by the input handler of a component
/// to notify the run-time about how the event was handled and
/// what the next steps are.
#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum InputEventResult {
    /// The event was accepted. This may result in additional events, for example
    /// accepting a mouse move will result in a MouseExit event later.
    EventAccepted,
    /// The event was ignored.
    EventIgnored,
    /* /// Same as grab, but continue forwarding the event to children.
    /// If a child grab the mouse, the grabber will be stored in the item itself.
    /// Only item that have grabbed storage can return this.
    /// The new_grabber is a reference to a usize to store thenext grabber
    TentativeGrab {
        new_grabber: &'a Cell<usize>,
    },
    /// While we have a TentaztiveGrab
    Forward {
        to: usize,
    },*/
    /// All further mouse event need to be sent to this item or component
    GrabMouse,
}

impl Default for InputEventResult {
    fn default() -> Self {
        Self::EventIgnored
    }
}

/// A key code is a symbolic name for a key on a keyboard. Depending on the
/// key mappings, different keys may produce different key codes.
/// Key codes are typically produced when pressing or releasing a key.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, MappedKeyCode)]
#[allow(missing_docs)]
pub enum KeyCode {
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    Key0,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Escape,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    Snapshot,
    Scroll,
    Pause,
    Insert,
    Home,
    Delete,
    End,
    PageDown,
    PageUp,
    Left,
    Up,
    Right,
    Down,
    Back,
    Return,
    Space,
    Compose,
    Caret,
    Numlock,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    AbntC1,
    AbntC2,
    Add,
    Apostrophe,
    Apps,
    At,
    Ax,
    Backslash,
    Calculator,
    Capital,
    Colon,
    Comma,
    Convert,
    Decimal,
    Divide,
    Equals,
    Grave,
    Kana,
    Kanji,
    LAlt,
    LBracket,
    LControl,
    LShift,
    LWin,
    Mail,
    MediaSelect,
    MediaStop,
    Minus,
    Multiply,
    Mute,
    MyComputer,
    NavigateForward,
    NavigateBackward,
    NextTrack,
    NoConvert,
    NumpadComma,
    NumpadEnter,
    NumpadEquals,
    OEM102,
    Period,
    PlayPause,
    Power,
    PrevTrack,
    RAlt,
    RBracket,
    RControl,
    RShift,
    RWin,
    Semicolon,
    Slash,
    Sleep,
    Stop,
    Subtract,
    Sysrq,
    Tab,
    Underline,
    Unlabeled,
    VolumeDown,
    VolumeUp,
    Wake,
    WebBack,
    WebFavorites,
    WebForward,
    WebHome,
    WebRefresh,
    WebSearch,
    WebStop,
    Yen,
    Copy,
    Paste,
    Cut,
}

impl TryFrom<char> for KeyCode {
    type Error = ();

    fn try_from(value: char) -> Result<Self, Self::Error> {
        Ok(match value {
            'a' => Self::A,
            'b' => Self::B,
            'c' => Self::C,
            'd' => Self::D,
            'e' => Self::E,
            'f' => Self::F,
            'g' => Self::G,
            'h' => Self::H,
            'i' => Self::I,
            'j' => Self::J,
            'k' => Self::K,
            'l' => Self::L,
            'm' => Self::M,
            'n' => Self::N,
            'o' => Self::O,
            'p' => Self::P,
            'q' => Self::Q,
            'r' => Self::R,
            's' => Self::S,
            't' => Self::T,
            'u' => Self::U,
            'v' => Self::V,
            'w' => Self::W,
            'x' => Self::X,
            'y' => Self::Y,
            'z' => Self::Z,
            _ => return Err(()),
        })
    }
}

/// KeyboardModifiers wraps a u32 that reserves a single bit for each
/// possible modifier key on a keyboard, such as Shift, Control, etc.
///
/// On macOS, the command key is mapped to the logo modifier.
///
/// On Windows, the windows key is mapped to the logo modifier.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct KeyboardModifiers(u32);
/// KeyboardModifier wraps a u32 that has a single bit set to represent
/// a modifier key such as shift on a keyboard. Convenience constants such as
/// [`NO_MODIFIER`], [`SHIFT_MODIFIER`], [`CONTROL_MODIFIER`], [`ALT_MODIFIER`]
/// and [`LOGO_MODIFIER`] are provided.
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct KeyboardModifier(u32);
/// Convenience constant that indicates no modifier key being pressed on a keyboard.
pub const NO_MODIFIER: KeyboardModifier = KeyboardModifier(0);
/// Convenience constant that indicates the shift key being pressed on a keyboard.
pub const SHIFT_MODIFIER: KeyboardModifier =
    KeyboardModifier(winit::event::ModifiersState::SHIFT.bits());
/// Convenience constant that indicates the control key being pressed on a keyboard.
pub const CONTROL_MODIFIER: KeyboardModifier =
    KeyboardModifier(winit::event::ModifiersState::CTRL.bits());
/// Convenience constant that indicates the control key being pressed on a keyboard.
pub const ALT_MODIFIER: KeyboardModifier =
    KeyboardModifier(winit::event::ModifiersState::ALT.bits());
/// Convenience constant that on macOS indicates the command key and on Windows the
/// windows key being pressed on a keyboard.
pub const LOGO_MODIFIER: KeyboardModifier =
    KeyboardModifier(winit::event::ModifiersState::LOGO.bits());

/// Convenience constant that is used to detect copy & paste related shortcuts, where
/// on macOS the modifier is the command key (aka LOGO_MODIFIER) and on Linux and Windows
/// it is control.
pub const COPY_PASTE_MODIFIER: KeyboardModifier =
    if cfg!(target_os = "macos") { LOGO_MODIFIER } else { CONTROL_MODIFIER };

impl KeyboardModifiers {
    /// Returns true if this set of keyboard modifiers includes the given modifier; false otherwise.
    ///
    /// Arguments:
    /// * `modifier`: The keyboard modifier to test for, usually one of the provided convenience
    ///               constants such as [`SHIFT_MODIFIER`].
    pub fn test(&self, modifier: KeyboardModifier) -> bool {
        self.0 & modifier.0 != 0
    }

    /// Returns true if this set of keyboard modifiers consists of exactly the one specified
    /// modifier; false otherwise.
    ///
    /// Arguments:
    /// * `modifier`: The only modifier that is allowed to be in this modifier set, in order
    //                for this function to return true;
    pub fn test_exclusive(&self, modifier: KeyboardModifier) -> bool {
        self.0 == modifier.0
    }

    /// Returns true if the shift key is part of this set of keyboard modifiers.
    pub fn shift(&self) -> bool {
        self.test(SHIFT_MODIFIER)
    }

    /// Returns true if the control key is part of this set of keyboard modifiers.
    pub fn control(&self) -> bool {
        self.test(CONTROL_MODIFIER)
    }

    /// Returns true if the alt key is part of this set of keyboard modifiers.
    pub fn alt(&self) -> bool {
        self.test(ALT_MODIFIER)
    }

    /// Returns true if on macOS the command key and on Windows the Windows key is part of this
    /// set of keyboard modifiers.
    pub fn logo(&self) -> bool {
        self.test(LOGO_MODIFIER)
    }
}

impl Default for KeyboardModifiers {
    fn default() -> Self {
        Self(NO_MODIFIER.0)
    }
}

impl From<winit::event::ModifiersState> for KeyboardModifiers {
    fn from(state: winit::event::ModifiersState) -> Self {
        Self(state.bits())
    }
}

impl From<KeyboardModifier> for KeyboardModifiers {
    fn from(modifier: KeyboardModifier) -> Self {
        Self(modifier.0)
    }
}

/// Represents a key event sent by the windowing system.
#[derive(Debug, Clone, PartialEq)]
#[repr(C)]
pub enum KeyEvent {
    /// A key on a keyboard was pressed.
    KeyPressed {
        /// The key code of the pressed key.
        code: KeyCode,
        /// The keyboard modifiers active at the time of the key press event.
        modifiers: KeyboardModifiers,
    },
    /// A key on a keyboard was released.
    KeyReleased {
        /// The key code of the released key.
        code: KeyCode,
        /// The keyboard modifiers active at the time of the key release event.
        modifiers: KeyboardModifiers,
    },
    /// A key on a keyboard was released that results in
    /// a character that's suitable for text input.
    CharacterInput {
        /// The u32 is a unicode scalar value that is safe to convert to char.
        unicode_scalar: u32,
        /// The keyboard modifiers active at the time of the char input event.
        modifiers: KeyboardModifiers,
    },
}

impl TryFrom<(&winit::event::KeyboardInput, KeyboardModifiers)> for KeyEvent {
    type Error = ();

    fn try_from(
        input: (&winit::event::KeyboardInput, KeyboardModifiers),
    ) -> Result<Self, Self::Error> {
        let key_code = match input.0.virtual_keycode {
            Some(code) => code.into(),
            None => return Err(()),
        };
        Ok(match input.0.state {
            winit::event::ElementState::Pressed => {
                KeyEvent::KeyPressed { code: key_code, modifiers: input.1 }
            }
            winit::event::ElementState::Released => {
                KeyEvent::KeyReleased { code: key_code, modifiers: input.1 }
            }
        })
    }
}

/// Represents how an item's key_event handler dealt with a key event.
/// An accepted event results in no further event propagation.
#[repr(C)]
pub enum KeyEventResult {
    /// The event was handled.
    EventAccepted,
    /// The event was not handled and should be sent to other items.
    EventIgnored,
}

/// Feed the given mouse event into the tree of items that component holds. The
/// event will be delivered to items in front first.
///
/// The returned tuple is identical with the tuple the ItemVTable's input_event returns,
/// indicating the acceptance or potential mouse grabbing as well as how to proceed
/// in the event of recursive item tree traversal.
///
/// Arguments:
/// * `component`: The component to deliver the event to.
/// * `event`: The mouse event to deliver.
pub fn process_ungrabbed_mouse_event(
    component: ComponentRefPin,
    event: MouseEvent,
    window: &crate::eventloop::ComponentWindow,
) -> (InputEventResult, VisitChildrenResult) {
    let offset = Vector2D::new(0., 0.);

    let mut result = InputEventResult::EventIgnored;
    let item_index = crate::item_tree::visit_items(
        component,
        crate::item_tree::TraversalOrder::FrontToBack,
        |_, item, offset| -> ItemVisitorResult<Vector2D<f32>> {
            let geom = item.as_ref().geometry();
            let geom = geom.translate(*offset);

            if geom.contains(event.pos) {
                let mut event2 = event.clone();
                event2.pos -= geom.origin.to_vector();
                match item.as_ref().input_event(event2, window) {
                    InputEventResult::EventAccepted => {
                        result = InputEventResult::EventAccepted;
                        return ItemVisitorResult::Abort;
                    }
                    InputEventResult::EventIgnored => (),
                    InputEventResult::GrabMouse => {
                        result = InputEventResult::GrabMouse;
                        return ItemVisitorResult::Abort;
                    }
                };
            }

            ItemVisitorResult::Continue(geom.origin.to_vector())
        },
        offset,
    );

    (
        result,
        if result == InputEventResult::GrabMouse {
            item_index
        } else {
            VisitChildrenResult::CONTINUE
        },
    )
}
/*
/// The event must be in the component coordinate
/// Returns the new grabber.
pub fn process_grabbed_mouse_event(
    component: ComponentRefPin,
    item: core::pin::Pin<ItemRef>,
    offset: Point,
    event: MouseEvent,
    old_grab: VisitChildrenResult,
) -> (InputEventResult, VisitChildrenResult) {
    let mut event2 = event.clone();
    event2.pos -= offset.to_vector();

    let res = item.as_ref().input_event(event2);
    match res {
        InputEventResult::EventIgnored => {
            // We need then to forward to another event
            process_ungrabbed_mouse_event(component, event)
        }
        InputEventResult::GrabMouse => (res, old_grab),
        InputEventResult::EventAccepted => (res, VisitChildrenResult::CONTINUE),
    }
}*/

/// Process the given key event by sending it to the item tree that belongs to the specified component.
pub fn process_key_event(
    component: ComponentRefPin,
    event: &KeyEvent,
    window: &crate::eventloop::ComponentWindow,
) {
    crate::item_tree::visit_items(
        component,
        crate::item_tree::TraversalOrder::BackToFront,
        |_, item, _| match item.as_ref().key_event(event, window) {
            KeyEventResult::EventAccepted => ItemVisitorResult::Abort,
            KeyEventResult::EventIgnored => ItemVisitorResult::Continue(()),
        },
        (),
    );
}

pub(crate) mod ffi {
    use super::*;

    #[no_mangle]
    pub extern "C" fn sixtyfps_process_ungrabbed_mouse_event(
        component: core::pin::Pin<crate::component::ComponentRef>,
        event: MouseEvent,
        window: &crate::eventloop::ComponentWindow,
        new_mouse_grabber: &mut crate::item_tree::VisitChildrenResult,
    ) -> InputEventResult {
        let (res, grab) = process_ungrabbed_mouse_event(component, event, window);
        *new_mouse_grabber = grab;
        res
    }
    /*
    #[no_mangle]
    pub extern "C" fn sixtyfps_process_grabbed_mouse_event(
        component: ComponentRefPin,
        item: core::pin::Pin<ItemRef>,
        offset: Point,
        event: MouseEvent,
        old_grab: VisitChildrenResult,
    ) -> (InputEventResult, crate::item_tree::VisitChildrenResult) {
        process_grabbed_mouse_event(component, item, offset, event, old_grab)
    }*/
}
