// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*! Module handling mouse events
*/
#![warn(missing_docs)]

use crate::graphics::Point;
use crate::item_tree::{ItemVisitorResult, VisitChildrenResult};
use crate::items::{ItemRc, ItemRef, ItemWeak, PointerEventButton, TextCursorDirection};
use crate::window::WindowRc;
use crate::Property;
use crate::{component::ComponentRc, SharedString};
use alloc::rc::Rc;
use alloc::vec::Vec;
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use euclid::default::Vector2D;

/// A Mouse event
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs)]
pub enum MouseEvent {
    /// The mouse was pressed
    MousePressed { pos: Point, button: PointerEventButton },
    /// The mouse was released
    MouseReleased { pos: Point, button: PointerEventButton },
    /// The mouse position has changed
    MouseMoved { pos: Point },
    /// Wheel was operated.
    /// `pos` is the position of the mouse when the event happens.
    /// `delta` is the amount of pixel to scroll.
    MouseWheel { pos: Point, delta: Point },
    /// The mouse exited the item or component
    MouseExit,
}

impl MouseEvent {
    /// The position of the cursor
    pub fn pos(&self) -> Option<Point> {
        match self {
            MouseEvent::MousePressed { pos, .. } => Some(*pos),
            MouseEvent::MouseReleased { pos, .. } => Some(*pos),
            MouseEvent::MouseMoved { pos } => Some(*pos),
            MouseEvent::MouseWheel { pos, .. } => Some(*pos),
            MouseEvent::MouseExit => None,
        }
    }

    /// Translate the position by the given value
    pub fn translate(&mut self, vec: Vector2D<f32>) {
        let pos = match self {
            MouseEvent::MousePressed { pos, .. } => Some(pos),
            MouseEvent::MouseReleased { pos, .. } => Some(pos),
            MouseEvent::MouseMoved { pos } => Some(pos),
            MouseEvent::MouseWheel { pos, .. } => Some(pos),
            MouseEvent::MouseExit => None,
        };
        if let Some(pos) = pos {
            *pos += vec;
        }
    }
}

/// This value is returned by the `input_event` function of an Item
/// to notify the run-time about how the event was handled and
/// what the next steps are.
/// See [`crate::items::ItemVTable::input_event`].
#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum InputEventResult {
    /// The event was accepted. This may result in additional events, for example
    /// accepting a mouse move will result in a MouseExit event later.
    EventAccepted,
    /// The event was ignored.
    EventIgnored,
    /// All further mouse event need to be sent to this item or component
    GrabMouse,
}

impl Default for InputEventResult {
    fn default() -> Self {
        Self::EventIgnored
    }
}

/// This value is returned by the `input_event_filter_before_children` function, which
/// can specify how to further process the event.
/// See [`crate::items::ItemVTable::input_event_filter_before_children`].
#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum InputEventFilterResult {
    /// The event is going to be forwarded to children, then the [`crate::items::ItemVTable::input_event`]
    /// function is called
    ForwardEvent,
    /// The event will be forwarded to the children, but the [`crate::items::ItemVTable::input_event`] is not
    /// going to be called for this item
    ForwardAndIgnore,
    /// Just like `ForwardEvent`, but even in the case the children grabs the mouse, this function
    /// Will still be called for further event.
    ForwardAndInterceptGrab,
    /// The Event will not be forwarded to children, if a children already had the grab, the
    /// grab will be cancelled with a [`MouseEvent::MouseExit`] event
    Intercept,
}

impl Default for InputEventFilterResult {
    fn default() -> Self {
        Self::ForwardEvent
    }
}

/// This module contains the constant character code used to represent the keys
#[allow(missing_docs, non_upper_case_globals)]
pub mod key_codes {
    macro_rules! declare_consts_for_special_keys {
       ($($char:literal # $name:ident # $($_qt:ident)|* # $($_winit:ident)|* ;)*) => {
            $(pub const $name : char = $char;)*
        };
    }

    i_slint_common::for_each_special_keys!(declare_consts_for_special_keys);
}

/// KeyboardModifier provides booleans to indicate possible modifier keys
/// on a keyboard, such as Shift, Control, etc.
///
/// On macOS, the command key is mapped to the meta modifier.
///
/// On Windows, the windows key is mapped to the meta modifier.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct KeyboardModifiers {
    /// Indicates the alt key on a keyboard.
    pub alt: bool,
    /// Indicates the control key on a keyboard.
    pub control: bool,
    /// Indicates the logo key on macOS and the windows key on Windows.
    pub meta: bool,
    /// Indicates the shift key on a keyboard.
    pub shift: bool,
}

#[derive(Debug, Clone, PartialEq, strum::EnumString, strum::Display)]
#[repr(C)]
/// This enum defines the different kinds of key events that can happen.
pub enum KeyEventType {
    /// A key on a keyboard was pressed.
    KeyPressed,
    /// A key on a keyboard was released.
    KeyReleased,
}

impl Default for KeyEventType {
    fn default() -> Self {
        Self::KeyPressed
    }
}

/// Represents a key event sent by the windowing system.
#[derive(Debug, Clone, PartialEq, Default)]
#[repr(C)]
pub struct KeyEvent {
    /// The keyboard modifiers active at the time of the key press event.
    pub modifiers: KeyboardModifiers,
    /// The unicode representation of the key pressed.
    pub text: SharedString,

    // note: this field is not exported in the .slint in the KeyEvent builtin struct
    /// Indicates whether the key was pressed or released
    pub event_type: KeyEventType,
}

impl KeyEvent {
    /// If a shortcut was pressed, this function returns `Some(StandardShortcut)`.
    /// Otherwise it returns None.
    pub fn shortcut(&self) -> Option<StandardShortcut> {
        if self.modifiers.control && !self.modifiers.shift {
            match self.text.as_str() {
                "c" => Some(StandardShortcut::Copy),
                "x" => Some(StandardShortcut::Cut),
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
                "z" => Some(StandardShortcut::Redo),
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

        let move_mod = if cfg!(target_os = "macos") {
            self.modifiers.alt && !self.modifiers.control && !self.modifiers.meta
        } else {
            self.modifiers.control && !self.modifiers.alt && !self.modifiers.meta
        };

        if move_mod {
            match keycode {
                key_codes::LeftArrow => {
                    return Some(TextShortcut::Move(TextCursorDirection::BackwardByWord))
                }
                key_codes::RightArrow => {
                    return Some(TextShortcut::Move(TextCursorDirection::ForwardByWord))
                }
                key_codes::UpArrow => {
                    return Some(TextShortcut::Move(TextCursorDirection::StartOfParagraph))
                }
                key_codes::DownArrow => {
                    return Some(TextShortcut::Move(TextCursorDirection::EndOfParagraph))
                }
                _ => (),
            };
        }

        #[cfg(not(target_os = "macos"))]
        {
            if self.modifiers.control && !self.modifiers.alt && !self.modifiers.meta {
                match keycode {
                    key_codes::Home => {
                        return Some(TextShortcut::Move(TextCursorDirection::StartOfText))
                    }
                    key_codes::End => {
                        return Some(TextShortcut::Move(TextCursorDirection::EndOfText))
                    }
                    _ => (),
                };
            }
        }

        #[cfg(target_os = "macos")]
        {
            if self.modifiers.control {
                match keycode {
                    key_codes::LeftArrow => {
                        return Some(TextShortcut::Move(TextCursorDirection::StartOfLine))
                    }
                    key_codes::RightArrow => {
                        return Some(TextShortcut::Move(TextCursorDirection::EndOfLine))
                    }
                    key_codes::UpArrow => {
                        return Some(TextShortcut::Move(TextCursorDirection::StartOfText))
                    }
                    key_codes::DownArrow => {
                        return Some(TextShortcut::Move(TextCursorDirection::EndOfText))
                    }
                    _ => (),
                };
            }
        }

        match TextCursorDirection::try_from(keycode) {
            Ok(direction) => return Some(TextShortcut::Move(direction)),
            _ => (),
        };

        match keycode {
            key_codes::Backspace => Some(TextShortcut::DeleteBackward),
            key_codes::Delete => Some(TextShortcut::DeleteForward),
            _ => None,
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
    /// Copy Something
    SelectAll,
    /// Select All
    Find,
    /// Find/Search Something
    Save,
    /// Save Something
    Print,
    /// Print Something
    Undo,
    /// Undo the last action
    Redo,
    /// Redo the last undone action
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
}

/// Represents how an item's key_event handler dealt with a key event.
/// An accepted event results in no further event propagation.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyEventResult {
    /// The event was handled.
    EventAccepted,
    /// The event was not handled and should be sent to other items.
    EventIgnored,
}

/// Represents how an item's focus_event handler dealt with a focus event.
/// An accepted event results in no further event propagation.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FocusEventResult {
    /// The event was handled.
    FocusAccepted,
    /// The event was not handled and should be sent to other items.
    FocusIgnored,
}

/// This event is sent to a component and items when they receive or loose
/// the keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub enum FocusEvent {
    /// This event is sent when an item receives the focus.
    FocusIn,
    /// This event is sent when an item looses the focus.
    FocusOut,
    /// This event is sent when the window receives the keyboard focus.
    WindowReceivedFocus,
    /// This event is sent when the window looses the keyboard focus.
    WindowLostFocus,
}

/// The state which a window should hold for the mouse input
#[derive(Default)]
pub struct MouseInputState {
    /// The stack of item which contain the mouse cursor (or grab),
    /// along with the last result from the input function
    item_stack: Vec<(ItemWeak, InputEventFilterResult)>,
    /// true if the top item of the stack has the mouse grab
    grabbed: bool,
}

/// Try to handle the mouse grabber. Return true if the event has handled, or false otherwise
fn handle_mouse_grab(
    mouse_event: &MouseEvent,
    window: &WindowRc,
    mouse_input_state: &mut MouseInputState,
) -> bool {
    if !mouse_input_state.grabbed || mouse_input_state.item_stack.is_empty() {
        return false;
    };

    let mut event = *mouse_event;
    let mut intercept = false;
    let mut invalid = false;

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
            item.borrow().as_ref().input_event(MouseEvent::MouseExit, window, &item);
            return false;
        }
        let g = item.borrow().as_ref().geometry();
        event.translate(-g.origin.to_vector());

        if it.1 == InputEventFilterResult::ForwardAndInterceptGrab
            && item.borrow().as_ref().input_event_filter_before_children(event, window, &item)
                == InputEventFilterResult::Intercept
        {
            intercept = true;
        }
        true
    });
    if invalid {
        return false;
    }

    let grabber = mouse_input_state.item_stack.last().unwrap().0.upgrade().unwrap();
    let input_result = grabber.borrow().as_ref().input_event(event, window, &grabber);
    if input_result != InputEventResult::GrabMouse {
        mouse_input_state.grabbed = false;
        send_exit_events(mouse_input_state, mouse_event.pos(), window);
    }

    true
}

fn send_exit_events(
    mouse_input_state: &MouseInputState,
    mut pos: Option<Point>,
    window: &WindowRc,
) {
    for it in mouse_input_state.item_stack.iter() {
        let item = if let Some(item) = it.0.upgrade() { item } else { break };
        let g = item.borrow().as_ref().geometry();
        let contains = pos.map_or(false, |p| g.contains(p));
        if let Some(p) = pos.as_mut() {
            *p -= g.origin.to_vector();
        }
        if !contains {
            item.borrow().as_ref().input_event(MouseEvent::MouseExit, window, &item);
        }
    }
}

/// Process the `mouse_event` on the `component`, the `mouse_grabber_stack` is the previous stack
/// of mouse grabber.
/// Returns a new mouse grabber stack.
pub fn process_mouse_input(
    component: ComponentRc,
    mouse_event: MouseEvent,
    window: &WindowRc,
    mut mouse_input_state: MouseInputState,
) -> MouseInputState {
    if handle_mouse_grab(&mouse_event, window, &mut mouse_input_state) {
        return mouse_input_state;
    }

    send_exit_events(&mouse_input_state, mouse_event.pos(), window);

    let mut result = MouseInputState::default();
    type State = (Vector2D<f32>, Vec<(ItemWeak, InputEventFilterResult)>);
    crate::item_tree::visit_items_with_post_visit(
        &component,
        crate::item_tree::TraversalOrder::FrontToBack,
        |comp_rc: &ComponentRc,
         item: core::pin::Pin<ItemRef>,
         item_index: usize,
         (offset, mouse_grabber_stack): &State| {
            let item_rc = ItemRc::new(comp_rc.clone(), item_index);

            let geom = item.as_ref().geometry();
            let geom = geom.translate(*offset);

            let mut mouse_grabber_stack = mouse_grabber_stack.clone();

            let post_visit_state = if mouse_event.pos().map_or(false, |p| geom.contains(p))
                || crate::item_rendering::is_clipping_item(item)
            {
                let mut event2 = mouse_event;
                event2.translate(-geom.origin.to_vector());
                let filter_result =
                    item.as_ref().input_event_filter_before_children(event2, window, &item_rc);
                mouse_grabber_stack.push((item_rc.downgrade(), filter_result));
                match filter_result {
                    InputEventFilterResult::ForwardAndIgnore => None,
                    InputEventFilterResult::ForwardEvent => {
                        Some((event2, mouse_grabber_stack.clone(), item_rc, false))
                    }
                    InputEventFilterResult::ForwardAndInterceptGrab => {
                        Some((event2, mouse_grabber_stack.clone(), item_rc, false))
                    }
                    InputEventFilterResult::Intercept => {
                        return (
                            ItemVisitorResult::Abort,
                            Some((event2, mouse_grabber_stack, item_rc, true)),
                        )
                    }
                }
            } else {
                mouse_grabber_stack
                    .push((item_rc.downgrade(), InputEventFilterResult::ForwardAndIgnore));
                None
            };

            (
                ItemVisitorResult::Continue((geom.origin.to_vector(), mouse_grabber_stack)),
                post_visit_state,
            )
        },
        |_, item, post_state, r| {
            if let Some((event2, mouse_grabber_stack, item_rc, intercept)) = post_state {
                if r.has_aborted() && !intercept {
                    return r;
                }
                match item.as_ref().input_event(event2, window, &item_rc) {
                    InputEventResult::EventAccepted => {
                        result.item_stack = mouse_grabber_stack;
                        result.grabbed = false;
                        return VisitChildrenResult::abort(item_rc.index(), 0);
                    }
                    InputEventResult::EventIgnored => {
                        return VisitChildrenResult::CONTINUE;
                    }
                    InputEventResult::GrabMouse => {
                        result.item_stack = mouse_grabber_stack;
                        result.item_stack.last_mut().unwrap().1 =
                            InputEventFilterResult::ForwardAndInterceptGrab;
                        result.grabbed = true;
                        return VisitChildrenResult::abort(item_rc.index(), 0);
                    }
                }
            }
            r
        },
        (Vector2D::new(0., 0.), Vec::new()),
    );
    result
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
    pub fn set_binding(instance: Pin<Rc<TextCursorBlinker>>, prop: &Property<bool>) {
        instance.as_ref().cursor_visible.set(true);
        // Re-start timer, in case.
        Self::start(&instance);
        prop.set_binding(move || {
            TextCursorBlinker::FIELD_OFFSETS.cursor_visible.apply_pin(instance.as_ref()).get()
        });
    }

    /// Starts the blinking cursor timer that will toggle the cursor and update all bindings that
    /// were installed on properties with set_binding call.
    pub fn start(self: &Pin<Rc<Self>>) {
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
            self.cursor_blink_timer.start(
                crate::timers::TimerMode::Repeated,
                core::time::Duration::from_millis(500),
                toggle_cursor,
            );
        }
    }

    /// Stops the blinking cursor timer. This is usually used for example when the window that contains
    /// text editable elements looses the focus or is hidden.
    pub fn stop(&self) {
        self.cursor_blink_timer.stop()
    }
}
