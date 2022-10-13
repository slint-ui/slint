// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*! Module handling mouse events
*/
#![warn(missing_docs)]

use crate::item_tree::{ItemRc, ItemWeak, VisitChildrenResult};
pub use crate::items::PointerEventButton;
use crate::items::{ItemRef, TextCursorDirection};
use crate::lengths::{LogicalPoint, LogicalVector};
use crate::timers::Timer;
use crate::window::{WindowAdapter, WindowInner};
use crate::Property;
use crate::{component::ComponentRc, SharedString};
use alloc::rc::Rc;
use alloc::vec::Vec;
use const_field_offset::FieldOffsets;
use core::pin::Pin;

/// A mouse or touch event
///
/// The only difference with [`crate::api::WindowEvent`] us that it uses untyped `Point`
/// TODO: merge with api::WindowEvent
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(missing_docs)]
pub enum MouseEvent {
    /// The mouse or finger was pressed
    Pressed { position: LogicalPoint, button: PointerEventButton },
    /// The mouse or finger was released
    Released { position: LogicalPoint, button: PointerEventButton },
    /// The position of the pointer has changed
    Moved { position: LogicalPoint },
    /// Wheel was operated.
    /// `pos` is the position of the mouse when the event happens.
    /// `delta_x` is the amount of pixels to scroll in horizontal direction,
    /// `delta_y` is the amount of pixels to scroll in vertical direction.
    Wheel { position: LogicalPoint, delta_x: f32, delta_y: f32 },
    /// The mouse exited the item or component
    Exit,
}

impl MouseEvent {
    /// The position of the cursor for this event, if any
    pub fn position(&self) -> Option<LogicalPoint> {
        match self {
            MouseEvent::Pressed { position, .. } => Some(*position),
            MouseEvent::Released { position, .. } => Some(*position),
            MouseEvent::Moved { position } => Some(*position),
            MouseEvent::Wheel { position, .. } => Some(*position),
            MouseEvent::Exit => None,
        }
    }

    /// Translate the position by the given value
    pub fn translate(&mut self, vec: LogicalVector) {
        let pos = match self {
            MouseEvent::Pressed { position, .. } => Some(position),
            MouseEvent::Released { position, .. } => Some(position),
            MouseEvent::Moved { position } => Some(position),
            MouseEvent::Wheel { position, .. } => Some(position),
            MouseEvent::Exit => None,
        };
        if let Some(pos) = pos {
            *pos += vec;
        }
    }
}

impl From<crate::api::WindowEvent> for MouseEvent {
    fn from(event: crate::api::WindowEvent) -> Self {
        match event {
            crate::api::WindowEvent::PointerPressed { position, button } => {
                MouseEvent::Pressed { position: position.to_euclid().cast(), button }
            }
            crate::api::WindowEvent::PointerReleased { position, button } => {
                MouseEvent::Released { position: position.to_euclid().cast(), button }
            }
            crate::api::WindowEvent::PointerMoved { position } => {
                MouseEvent::Moved { position: position.to_euclid().cast() }
            }
            crate::api::WindowEvent::PointerScrolled { position, delta_x, delta_y } => {
                MouseEvent::Wheel { position: position.to_euclid().cast(), delta_x, delta_y }
            }
            crate::api::WindowEvent::PointerExited => MouseEvent::Exit,
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
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum InputEventFilterResult {
    /// The event is going to be forwarded to children, then the [`crate::items::ItemVTable::input_event`]
    /// function is called
    ForwardEvent,
    /// The event will be forwarded to the children, but the [`crate::items::ItemVTable::input_event`] is not
    /// going to be called for this item
    ForwardAndIgnore,
    /// Just like `ForwardEvent`, but even in the case the children grabs the mouse, this function
    /// will still be called for further event
    ForwardAndInterceptGrab,
    /// The event will not be forwarded to children, if a children already had the grab, the
    /// grab will be cancelled with a [`MouseEvent::Exit`] event
    Intercept,
    /// Similar to `Intercept` but the contained [`MouseEvent`] will be forwarded to children
    InterceptAndDispatch(MouseEvent),
    /// The event will be forwarding to the children with a delay (in milliseconds), unless it is
    /// being intercepted.
    /// This is what happens when the flickable wants to delay the event.
    /// This should only be used for Press event, and the event will be sent after the delay, or
    /// if a release event is seen before that delay
    //(Can't use core::time::Duration because it is not repr(c))
    DelayForwarding(u64),
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

/// This enum defines the different kinds of key events that can happen.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub enum KeyEventType {
    /// A key on a keyboard was pressed.
    KeyPressed,
    /// A key on a keyboard was released.
    KeyReleased,
    /// The input method updates the currently composed text. The KeyEvent's text field is the pre-edit text and
    /// composition_selection specifies the placement of the cursor within the pre-edit text.
    UpdateComposition,
    /// The input method replaces the currently composed text with the final result of the composition.
    CommitComposition,
}

impl Default for KeyEventType {
    fn default() -> Self {
        KeyEventType::KeyPressed
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

    /// If the event type is KeyEventType::UpdateComposition, then this field specifies
    /// the start of the selection as byte offsets within the preedit text.
    pub preedit_selection_start: usize,
    /// If the event type is KeyEventType::UpdateComposition, then this field specifies
    /// the end of the selection as byte offsets within the preedit text.
    pub preedit_selection_end: usize,
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
    delayed: Option<(crate::timers::Timer, MouseEvent)>,
}

/// Try to handle the mouse grabber. Return true if the event has handled, or false otherwise
fn handle_mouse_grab(
    mouse_event: &MouseEvent,
    window_adapter: &Rc<dyn WindowAdapter>,
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
            item.borrow().as_ref().input_event(MouseEvent::Exit, window_adapter, &item);
            return false;
        }
        let g = item.geometry();
        event.translate(-g.origin.to_vector());

        let interested = matches!(
            it.1,
            InputEventFilterResult::ForwardAndInterceptGrab
                | InputEventFilterResult::DelayForwarding(_)
        );

        if interested
            && item.borrow().as_ref().input_event_filter_before_children(
                event,
                window_adapter,
                &item,
            ) == InputEventFilterResult::Intercept
        {
            intercept = true;
        }
        true
    });
    if invalid {
        return false;
    }

    let grabber = mouse_input_state.item_stack.last().unwrap().0.upgrade().unwrap();
    let input_result = grabber.borrow().as_ref().input_event(event, window_adapter, &grabber);
    if input_result != InputEventResult::GrabMouse {
        mouse_input_state.grabbed = false;
        send_exit_events(mouse_input_state, mouse_event.position(), window_adapter);
    }

    true
}

fn send_exit_events(
    mouse_input_state: &MouseInputState,
    mut pos: Option<LogicalPoint>,
    window_adapter: &Rc<dyn WindowAdapter>,
) {
    for it in mouse_input_state.item_stack.iter() {
        let item = if let Some(item) = it.0.upgrade() { item } else { break };
        let g = item.geometry();
        let contains = pos.map_or(false, |p| g.contains(p));
        if let Some(p) = pos.as_mut() {
            *p -= g.origin.to_vector();
        }
        if !contains {
            item.borrow().as_ref().input_event(MouseEvent::Exit, window_adapter, &item);
        }
    }
}

/// Process the `mouse_event` on the `component`, the `mouse_grabber_stack` is the previous stack
/// of mouse grabber.
/// Returns a new mouse grabber stack.
pub fn process_mouse_input(
    component: ComponentRc,
    mouse_event: MouseEvent,
    window_adapter: &Rc<dyn WindowAdapter>,
    mut mouse_input_state: MouseInputState,
) -> MouseInputState {
    if matches!(mouse_event, MouseEvent::Released { .. }) {
        mouse_input_state = process_delayed_event(window_adapter, mouse_input_state);
    }

    if handle_mouse_grab(&mouse_event, window_adapter, &mut mouse_input_state) {
        return mouse_input_state;
    }

    let mut result = MouseInputState::default();
    let root = ItemRc::new(component.clone(), 0);
    let r = send_mouse_event_to_item(mouse_event, root, window_adapter, &mut result, false);
    if mouse_input_state.delayed.is_some() && !r.has_aborted() {
        // Keep the delayed event
        return mouse_input_state;
    }
    send_exit_events(&mouse_input_state, mouse_event.position(), window_adapter);

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

    let top_item = match mouse_input_state.item_stack.last().unwrap().0.upgrade() {
        Some(i) => i,
        None => return MouseInputState::default(),
    };

    let mut actual_visitor =
        |component: &ComponentRc, index: usize, _: Pin<ItemRef>| -> VisitChildrenResult {
            send_mouse_event_to_item(
                event,
                ItemRc::new(component.clone(), index),
                window_adapter,
                &mut mouse_input_state,
                true,
            )
        };
    vtable::new_vref!(let mut actual_visitor : VRefMut<crate::item_tree::ItemVisitorVTable> for crate::item_tree::ItemVisitor = &mut actual_visitor);
    vtable::VRc::borrow_pin(&top_item.component()).as_ref().visit_children_item(
        top_item.index() as isize,
        crate::item_tree::TraversalOrder::FrontToBack,
        actual_visitor,
    );
    mouse_input_state
}

fn send_mouse_event_to_item(
    mouse_event: MouseEvent,
    item_rc: ItemRc,
    window_adapter: &Rc<dyn WindowAdapter>,
    result: &mut MouseInputState,
    ignore_delays: bool,
) -> VisitChildrenResult {
    let item = item_rc.borrow();
    let geom = item_rc.geometry();
    // translated in our coordinate
    let mut event_for_children = mouse_event;
    event_for_children.translate(-geom.origin.to_vector());

    let filter_result = if mouse_event.position().map_or(false, |p| geom.contains(p))
        || crate::item_rendering::is_clipping_item(item)
    {
        item.as_ref().input_event_filter_before_children(
            event_for_children,
            window_adapter,
            &item_rc,
        )
    } else {
        InputEventFilterResult::ForwardAndIgnore
    };

    let (forward_to_children, ignore) = match filter_result {
        InputEventFilterResult::ForwardEvent => (true, false),
        InputEventFilterResult::ForwardAndIgnore => (true, true),
        InputEventFilterResult::ForwardAndInterceptGrab => (true, false),
        InputEventFilterResult::Intercept => (false, false),
        InputEventFilterResult::InterceptAndDispatch(new_event) => {
            event_for_children = new_event;
            (true, false)
        }
        InputEventFilterResult::DelayForwarding(_) if ignore_delays => (true, false),
        InputEventFilterResult::DelayForwarding(duration) => {
            let timer = Timer::default();
            let w = Rc::downgrade(window_adapter);
            timer.start(
                crate::timers::TimerMode::SingleShot,
                core::time::Duration::from_millis(duration),
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
            |component: &ComponentRc, index: usize, _: Pin<ItemRef>| -> VisitChildrenResult {
                send_mouse_event_to_item(
                    event_for_children,
                    ItemRc::new(component.clone(), index),
                    window_adapter,
                    result,
                    ignore_delays,
                )
            };
        vtable::new_vref!(let mut actual_visitor : VRefMut<crate::item_tree::ItemVisitorVTable> for crate::item_tree::ItemVisitor = &mut actual_visitor);
        let r = vtable::VRc::borrow_pin(&item_rc.component()).as_ref().visit_children_item(
            item_rc.index() as isize,
            crate::item_tree::TraversalOrder::FrontToBack,
            actual_visitor,
        );
        if r.has_aborted() {
            // the event was intercepted by a children
            if matches!(filter_result, InputEventFilterResult::InterceptAndDispatch(_)) {
                let mut event = mouse_event;
                event.translate(-geom.origin.to_vector());
                item.as_ref().input_event(event, window_adapter, &item_rc);
            }
            return r;
        }
    };

    let r = if ignore {
        InputEventResult::EventIgnored
    } else {
        let mut event = mouse_event;
        event.translate(-geom.origin.to_vector());
        item.as_ref().input_event(event, window_adapter, &item_rc)
    };
    match r {
        InputEventResult::EventAccepted => {
            return VisitChildrenResult::abort(item_rc.index(), 0);
        }
        InputEventResult::EventIgnored => {
            let _pop = result.item_stack.pop();
            debug_assert_eq!(
                _pop.map(|x| (x.0.upgrade().unwrap().index(), x.1)).unwrap(),
                (item_rc.index(), filter_result)
            );
            return VisitChildrenResult::CONTINUE;
        }
        InputEventResult::GrabMouse => {
            result.item_stack.last_mut().unwrap().1 =
                InputEventFilterResult::ForwardAndInterceptGrab;
            result.grabbed = true;
            return VisitChildrenResult::abort(item_rc.index(), 0);
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
