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
#[derive(Debug, Clone, Copy)]
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

/// Represents a key event sent by the windowing system.
#[derive(Debug, Clone, PartialEq)]
#[repr(C)]
pub enum KeyEvent {
    /// A key on a keyboard was pressed.
    KeyPressed(KeyCode),
    /// A key on a keyboard was released.
    KeyReleased(KeyCode),
    /// A key on a keyboard was released that results in
    /// a character that's suitable for text input. The provided
    /// u32 is a unicode scalar value that is safe to convert to
    /// char.
    CharacterInput(u32),
}

impl TryFrom<&winit::event::KeyboardInput> for KeyEvent {
    type Error = ();

    fn try_from(input: &winit::event::KeyboardInput) -> Result<Self, Self::Error> {
        let key_code = match input.virtual_keycode {
            Some(code) => code.into(),
            None => return Err(()),
        };
        Ok(match input.state {
            winit::event::ElementState::Pressed => KeyEvent::KeyPressed(key_code),
            winit::event::ElementState::Released => KeyEvent::KeyReleased(key_code),
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
pub fn process_key_event(component: ComponentRefPin, event: &KeyEvent) {
    crate::item_tree::visit_items(
        component,
        crate::item_tree::TraversalOrder::BackToFront,
        |_, item, _| match item.as_ref().key_event(event) {
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
