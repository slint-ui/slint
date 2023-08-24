// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore backtab

#![warn(missing_docs)]
//! Exposed Window API

use crate::api::{
    CloseRequestResponse, PhysicalPosition, PhysicalSize, PlatformError, Window, WindowPosition,
    WindowSize,
};
use crate::component::{ComponentRc, ComponentRef, ComponentVTable, ComponentWeak};
use crate::graphics::Point;
use crate::input::{
    key_codes, ClickState, InternalKeyboardModifierState, KeyEvent, KeyEventType, KeyInputEvent,
    KeyboardModifiers, MouseEvent, MouseInputState, TextCursorBlinker,
};
use crate::item_tree::ItemRc;
use crate::items::{ItemRef, MouseCursor};
use crate::lengths::{LogicalLength, LogicalPoint, LogicalRect, LogicalSize, SizeLengths};
use crate::properties::{Property, PropertyTracker};
use crate::renderer::Renderer;
use crate::{Callback, Coord, SharedString};
use alloc::boxed::Box;
use alloc::rc::{Rc, Weak};
use core::cell::{Cell, RefCell};
use core::pin::Pin;
use euclid::num::Zero;
use vtable::VRcMapped;

fn next_focus_item(item: ItemRc) -> ItemRc {
    item.next_focus_item()
}

fn previous_focus_item(item: ItemRc) -> ItemRc {
    item.previous_focus_item()
}

/// Transforms a `KeyInputEvent` into an `KeyEvent` with the given `KeyboardModifiers`.
fn input_as_key_event(input: KeyInputEvent, modifiers: KeyboardModifiers) -> KeyEvent {
    KeyEvent {
        modifiers,
        text: input.text,
        event_type: input.event_type,
        preedit_selection_start: input.preedit_selection_start,
        preedit_selection_end: input.preedit_selection_end,
    }
}

/// This trait represents the adaptation layer between the [`Window`] API, and the
/// internal type from the backend that provides functionality such as device-independent pixels,
/// window resizing, and other typically windowing system related tasks.
///
/// See also [`MinimalSoftwareWindow`](crate::software_renderer::MinimalSoftwareWindow)
/// for a minimal implementation of this trait using the software renderer
pub trait WindowAdapter {
    /// Returns the window API.
    fn window(&self) -> &Window;

    /// Show the window if the argument is true, hide otherwise.
    fn set_visible(&self, _visible: bool) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Returns the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    ///
    /// The default implementation returns `None`
    ///
    /// Called from [`Window::position()`]
    fn position(&self) -> Option<PhysicalPosition> {
        None
    }
    /// Sets the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    ///
    /// The default implementation does nothing
    ///
    /// Called from [`Window::set_position()`]
    fn set_position(&self, _position: WindowPosition) {}

    /// Request a new size for the window to the specified size on the screen, in physical or logical pixels
    /// and excluding a window frame (if present).
    ///
    /// This is called from [`Window::set_size()`]
    ///
    /// The default implementation does nothing
    ///
    /// This function should sent the size to the Windowing system. If the window size actually changes, you
    /// should dispatch a [`WindowEvent::Resized`](crate::platform::WindowEvent::Resized) using
    /// [`Window::dispatch_event()`] to propagate the new size to the slint view
    fn set_size(&self, _size: WindowSize) {}

    /// Return the size of the Window on the screen
    fn size(&self) -> PhysicalSize;

    /// Issues a request to the windowing system to re-render the contents of the window.
    ///
    /// This request is typically asynchronous.
    /// It is called when a property that was used during window rendering is marked as dirty.
    ///
    /// An implementation should repaint the window in a subsequent iteration of the event loop,
    /// throttled to the screen refresh rate if possible.
    /// It is important not to query any Slint properties to avoid introducing a dependency loop in the properties,
    /// including the use of the render function, which itself queries properties.
    ///
    /// See also [`Window::request_redraw()`]
    fn request_redraw(&self) {}

    /// Return the renderer.
    ///
    /// The `Renderer` trait is an internal trait that you are not expected to implement.
    /// In your implementation you should return a reference to an instance of one of the renderers provided by Slint.
    ///
    /// Currently, the only public struct that implement renderer is [`SoftwareRenderer`](crate::software_renderer::SoftwareRenderer).
    fn renderer(&self) -> &dyn Renderer;

    /// Re-implement this function to update the properties such as window title or layout constraints.
    ///
    /// This function is called before `set_visible(true)`, and will be called again when the properties
    /// that were queried on the last call are changed. If you do not query any properties, it may not
    /// be called again.
    fn update_window_properties(&self, _properties: WindowProperties<'_>) {}

    #[doc(hidden)]
    fn internal(&self, _: crate::InternalToken) -> Option<&dyn WindowAdapterInternal> {
        None
    }
}

/// Implementation details behind [`WindowAdapter`], but since this
/// trait is not exported in the public API, it is not possible for the
/// users to call or re-implement these functions.
// TODO: instead of a sealed trait, have an WindowAdapterInternal trait and have a secret function in WindowAdapter:
// `#[doc(hidden)] fn internal(&self, InternalToken) -> Option<&WindowAdapterInternal> {None}`
// TODO: add events for window receiving and loosing focus
#[doc(hidden)]
pub trait WindowAdapterInternal {
    /// This function is called by the generated code when a component and therefore its tree of items are created.
    fn register_component(&self) {}

    /// This function is called by the generated code when a component and therefore its tree of items are destroyed. The
    /// implementation typically uses this to free the underlying graphics resources cached via [`crate::graphics::RenderingCache`].
    fn unregister_component(
        &self,
        _component: ComponentRef,
        _items: &mut dyn Iterator<Item = Pin<ItemRef<'_>>>,
    ) {
    }

    /// Create a window for a popup.
    ///
    /// `geometry` is the location of the popup in the window coordinate
    ///
    /// If this function return None (the default implementation), then the
    /// popup will be rendered within the window itself.
    fn create_popup(&self, _geometry: LogicalRect) -> Option<Rc<dyn WindowAdapter>> {
        None
    }

    /// Set the mouse cursor
    // TODO: Make the enum public and make public
    fn set_mouse_cursor(&self, _cursor: MouseCursor) {}

    /// This method allow editable input field to communicate with the platform about input methods
    fn input_method_request(&self, _: InputMethodRequest) {}

    /// Return self as any so the backend can upcast
    // TODO: consider using the as_any crate, or deriving the traint from Any to provide a better default
    fn as_any(&self) -> &dyn core::any::Any {
        &()
    }

    /// Handle focus change
    // used for accessibility
    fn handle_focus_change(&self, _old: Option<ItemRc>, _new: Option<ItemRc>) {}

    /// returns wether a dark theme is used
    fn dark_color_scheme(&self) -> bool {
        false
    }

    /// Get the visibility of the window
    // todo: replace with WindowEvent::VisibilityChanged and require backend to dispatch event
    fn is_visible(&self) -> bool {
        false
    }
}

/// This is the parameter from [`WindowAdapterInternal::input_method_request()`] which lets the editable text input field
/// communicate with the platform about input methods.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum InputMethodRequest {
    /// This request is sent when an editable text input field has received the focus and input methods such as
    /// a virtual keyboard should be shown.
    #[non_exhaustive]
    Enable {
        /// The type of input that is requesting an input method.
        input_type: crate::items::InputType,
    },
    /// This request is sent when the focused text input field lost focus and any active input method should
    /// be disabled.
    #[non_exhaustive]
    Disable {},
    /// Request an update of the position of the text cursor, so that for example the input method can adjust
    /// the location of completion popups.
    #[non_exhaustive]
    SetPosition {
        /// The position of the text cursor in window coordinates.
        position: crate::api::LogicalPosition,
    },
}

/// This struct describes layout constraints of a resizable element, such as a window.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct LayoutConstraints {
    /// The minimum size.
    pub min: Option<crate::api::LogicalSize>,
    /// The maximum size.
    pub max: Option<crate::api::LogicalSize>,
    /// The preferred size.
    pub preferred: crate::api::LogicalSize,
}

/// This struct contains getters that provide access to properties of the `Window`
/// element, and is used with [`WindowAdapter::update_window_properties`].
pub struct WindowProperties<'a>(&'a WindowInner);

impl<'a> WindowProperties<'a> {
    /// Returns the Window's title
    pub fn title(&self) -> SharedString {
        self.0.window_item().map(|w| w.as_pin_ref().title()).unwrap_or_default()
    }

    /// The background color or brush of the Window
    pub fn background(&self) -> crate::Brush {
        self.0
            .window_item()
            .map(|w: VRcMapped<ComponentVTable, crate::items::WindowItem>| {
                w.as_pin_ref().background()
            })
            .unwrap_or_default()
    }

    /// Returns the layout constraints of the window
    pub fn layout_constraints(&self) -> LayoutConstraints {
        let component = self.0.component();
        let component = ComponentRc::borrow_pin(&component);
        let h = component.as_ref().layout_info(crate::layout::Orientation::Horizontal);
        let v = component.as_ref().layout_info(crate::layout::Orientation::Vertical);
        let (min, max) = crate::layout::min_max_size_for_layout_constraints(h, v);
        LayoutConstraints {
            min,
            max,
            preferred: crate::api::LogicalSize::new(
                h.preferred_bounded() as f32,
                v.preferred_bounded() as f32,
            ),
        }
    }
}

struct WindowPropertiesTracker {
    window_adapter_weak: Weak<dyn WindowAdapter>,
}

impl crate::properties::PropertyDirtyHandler for WindowPropertiesTracker {
    fn notify(&self) {
        let win = self.window_adapter_weak.clone();
        crate::timers::Timer::single_shot(Default::default(), move || {
            if let Some(window_adapter) = win.upgrade() {
                WindowInner::from_pub(window_adapter.window()).update_window_properties();
            };
        })
    }
}

struct WindowRedrawTracker {
    window_adapter_weak: Weak<dyn WindowAdapter>,
}

impl crate::properties::PropertyDirtyHandler for WindowRedrawTracker {
    fn notify(&self) {
        if let Some(window_adapter) = self.window_adapter_weak.upgrade() {
            window_adapter.request_redraw();
        };
    }
}

/// This enum describes the different ways a popup can be rendered by the back-end.
enum PopupWindowLocation {
    /// The popup is rendered in its own top-level window that is know to the windowing system.
    TopLevel(Rc<dyn WindowAdapter>),
    /// The popup is rendered as an embedded child window at the given position.
    ChildWindow(LogicalPoint),
}

/// This structure defines a graphical element that is designed to pop up from the surrounding
/// UI content, for example to show a context menu.
struct PopupWindow {
    /// The location defines where the pop up is rendered.
    location: PopupWindowLocation,
    /// The component that is responsible for providing the popup content.
    component: ComponentRc,
    /// If true, Slint will close the popup after any mouse click within the popup.
    /// Set to false and call close() on the PopupWindow to close it manually.
    close_on_click: bool,
}

#[pin_project::pin_project]
struct WindowPinnedFields {
    #[pin]
    redraw_tracker: PropertyTracker<WindowRedrawTracker>,
    /// Gets dirty when the layout restrictions, or some other property of the windows change
    #[pin]
    window_properties_tracker: PropertyTracker<WindowPropertiesTracker>,
    #[pin]
    scale_factor: Property<f32>,
    #[pin]
    active: Property<bool>,
    #[pin]
    text_input_focused: Property<bool>,
}

/// Inner datastructure for the [`crate::api::Window`]
pub struct WindowInner {
    window_adapter_weak: Weak<dyn WindowAdapter>,
    component: RefCell<ComponentWeak>,
    mouse_input_state: Cell<MouseInputState>,
    pub(crate) modifiers: Cell<InternalKeyboardModifierState>,

    /// itemRC will retrieve on wasms
    pub focus_item: RefCell<crate::item_tree::ItemWeak>,
    cursor_blinker: RefCell<pin_weak::rc::PinWeak<crate::input::TextCursorBlinker>>,

    pinned_fields: Pin<Box<WindowPinnedFields>>,
    active_popup: RefCell<Option<PopupWindow>>,
    close_requested: Callback<(), CloseRequestResponse>,
    click_state: ClickState,
}

impl Drop for WindowInner {
    fn drop(&mut self) {
        if let Some(existing_blinker) = self.cursor_blinker.borrow().upgrade() {
            existing_blinker.stop();
        }
    }
}

impl WindowInner {
    /// Create a new instance of the window, given the window_adapter factory fn
    pub fn new(window_adapter_weak: Weak<dyn WindowAdapter>) -> Self {
        #![allow(unused_mut)]

        let mut window_properties_tracker =
            PropertyTracker::new_with_dirty_handler(WindowPropertiesTracker {
                window_adapter_weak: window_adapter_weak.clone(),
            });

        let mut redraw_tracker = PropertyTracker::new_with_dirty_handler(WindowRedrawTracker {
            window_adapter_weak: window_adapter_weak.clone(),
        });

        #[cfg(slint_debug_property)]
        {
            window_properties_tracker
                .set_debug_name("i_slint_core::Window::window_properties_tracker".into());
            redraw_tracker.set_debug_name("i_slint_core::Window::redraw_tracker".into());
        }

        Self {
            window_adapter_weak,
            component: Default::default(),
            mouse_input_state: Default::default(),
            modifiers: Default::default(),
            pinned_fields: Box::pin(WindowPinnedFields {
                redraw_tracker,
                window_properties_tracker,
                scale_factor: Property::new_named(1., "i_slint_core::Window::scale_factor"),
                active: Property::new_named(false, "i_slint_core::Window::active"),
                text_input_focused: Property::new_named(
                    false,
                    "i_slint_core::Window::text_input_focused",
                ),
            }),
            focus_item: Default::default(),
            cursor_blinker: Default::default(),
            active_popup: Default::default(),
            close_requested: Default::default(),
            click_state: ClickState::default(),
        }
    }

    /// Associates this window with the specified component. Further event handling and rendering, etc. will be
    /// done with that component.
    pub fn set_component(&self, component: &ComponentRc) {
        self.close_popup();
        self.focus_item.replace(Default::default());
        self.mouse_input_state.replace(Default::default());
        self.modifiers.replace(Default::default());
        self.component.replace(ComponentRc::downgrade(component));
        self.pinned_fields.window_properties_tracker.set_dirty(); // component changed, layout constraints for sure must be re-calculated
        let window_adapter = self.window_adapter();
        window_adapter.renderer().set_window_adapter(&window_adapter);
        {
            let component = ComponentRc::borrow_pin(component);
            let root_item = component.as_ref().get_item_ref(0);
            let window_item = ItemRef::downcast_pin::<crate::items::WindowItem>(root_item).unwrap();

            let default_font_size_prop =
                crate::items::WindowItem::FIELD_OFFSETS.default_font_size.apply_pin(window_item);
            if default_font_size_prop.get().get() <= 0 as Coord {
                default_font_size_prop.set(window_adapter.renderer().default_font_size());
            }
        }
        window_adapter.request_redraw();
        let weak = Rc::downgrade(&window_adapter);
        crate::timers::Timer::single_shot(Default::default(), move || {
            if let Some(window_adapter) = weak.upgrade() {
                WindowInner::from_pub(window_adapter.window()).update_window_properties();
            }
        })
    }

    /// return the component.
    /// Panics if it wasn't set.
    pub fn component(&self) -> ComponentRc {
        self.component.borrow().upgrade().unwrap()
    }

    /// returns the component or None if it isn't set.
    pub fn try_component(&self) -> Option<ComponentRc> {
        self.component.borrow().upgrade()
    }

    /// Receive a mouse event and pass it to the items of the component to
    /// change their state.
    ///
    /// Arguments:
    /// * `pos`: The position of the mouse event in window physical coordinates.
    /// * `what`: The type of mouse event.
    /// * `component`: The Slint compiled component that provides the tree of items.
    pub fn process_mouse_input(&self, mut event: MouseEvent) {
        crate::animations::update_animations();

        // handle multiple press release
        event = self.click_state.check_repeat(event);

        let close_popup_after_click = self.close_popup_after_click();

        let embedded_popup_component =
            self.active_popup.borrow().as_ref().and_then(|popup| match popup.location {
                PopupWindowLocation::TopLevel(_) => None,
                PopupWindowLocation::ChildWindow(coordinates) => {
                    Some((popup.component.clone(), coordinates))
                }
            });

        let component = embedded_popup_component
            .as_ref()
            .and_then(|(popup_component, coordinates)| {
                event.translate(-coordinates.to_vector());

                if let MouseEvent::Pressed { position, .. } = &event {
                    // close the popup if one press outside the popup
                    let geom = ComponentRc::borrow_pin(popup_component)
                        .as_ref()
                        .get_item_ref(0)
                        .as_ref()
                        .geometry();
                    if !geom.contains(*position) {
                        self.close_popup();
                        return None;
                    }
                }
                Some(popup_component.clone())
            })
            .or_else(|| self.component.borrow().upgrade());

        let component = if let Some(component) = component {
            component
        } else {
            return;
        };

        self.mouse_input_state.set(crate::input::process_mouse_input(
            component,
            event,
            &self.window_adapter(),
            self.mouse_input_state.take(),
        ));

        if embedded_popup_component.is_some()
            && close_popup_after_click
            && matches!(event, MouseEvent::Released { .. })
        {
            self.close_popup();
        }
    }

    /// Called by the input code's internal timer to send an event that was delayed
    pub(crate) fn process_delayed_event(&self) {
        self.mouse_input_state.set(crate::input::process_delayed_event(
            &self.window_adapter(),
            self.mouse_input_state.take(),
        ));
    }

    /// Receive a key event and pass it to the items of the component to
    /// change their state.
    ///
    /// Arguments:
    /// * `event`: The key event received by the windowing system.
    /// * `component`: The Slint compiled component that provides the tree of items.
    pub fn process_key_input(&self, event: KeyInputEvent) {
        if let Some(updated_modifier) = self
            .modifiers
            .get()
            .state_update(event.event_type == KeyEventType::KeyPressed, &event.text)
        {
            // Updates the key modifiers depending on the key code and pressed state.
            self.modifiers.set(updated_modifier);
        }

        let event = input_as_key_event(event, self.modifiers.get().into());

        let mut item = self.focus_item.borrow().clone().upgrade();
        while let Some(focus_item) = item {
            if !focus_item.is_visible() {
                // Reset the focus... not great, but better than keeping it.
                self.take_focus_item();
            } else if focus_item.borrow().as_ref().key_event(
                &event,
                &self.window_adapter(),
                &focus_item,
            ) == crate::input::KeyEventResult::EventAccepted
            {
                return;
            }
            item = focus_item.parent_item();
        }

        // Make Tab/Backtab handle keyboard focus
        if event.text.starts_with(key_codes::Tab)
            && !event.modifiers.shift
            && event.event_type == KeyEventType::KeyPressed
        {
            self.focus_next_item();
        } else if (event.text.starts_with(key_codes::Backtab)
            || (event.text.starts_with(key_codes::Tab) && event.modifiers.shift))
            && event.event_type == KeyEventType::KeyPressed
        {
            self.focus_previous_item();
        }
    }

    /// Installs a binding on the specified property that's toggled whenever the text cursor is supposed to be visible or not.
    pub fn set_cursor_blink_binding(&self, prop: &crate::Property<bool>) {
        let existing_blinker = self.cursor_blinker.borrow().clone();

        let blinker = existing_blinker.upgrade().unwrap_or_else(|| {
            let new_blinker = TextCursorBlinker::new();
            *self.cursor_blinker.borrow_mut() =
                pin_weak::rc::PinWeak::downgrade(new_blinker.clone());
            new_blinker
        });

        TextCursorBlinker::set_binding(blinker, prop);
    }

    /// Sets the focus to the item pointed to by item_ptr. This will remove the focus from any
    /// currently focused item.
    pub fn set_focus_item(&self, focus_item: &ItemRc) {
        let old = self.take_focus_item();
        let new = self.move_focus(focus_item.clone(), next_focus_item);
        let window_adapter = self.window_adapter();
        if let Some(window_adapter) = window_adapter.internal(crate::InternalToken) {
            window_adapter.handle_focus_change(old, new);
        }
    }

    /// Take the focus_item out of this Window
    ///
    /// This sends the FocusOut event!
    fn take_focus_item(&self) -> Option<ItemRc> {
        let focus_item = self.focus_item.take();

        if let Some(focus_item_rc) = focus_item.upgrade() {
            focus_item_rc.borrow().as_ref().focus_event(
                &crate::input::FocusEvent::FocusOut,
                &self.window_adapter(),
                &focus_item_rc,
            );
            Some(focus_item_rc)
        } else {
            None
        }
    }

    /// Publish the new focus_item to this Window and return the FocusEventResult
    ///
    /// This sends a FocusIn event!
    fn publish_focus_item(&self, item: &Option<ItemRc>) -> crate::input::FocusEventResult {
        match item {
            Some(item) => {
                *self.focus_item.borrow_mut() = item.downgrade();
                item.borrow().as_ref().focus_event(
                    &crate::input::FocusEvent::FocusIn,
                    &self.window_adapter(),
                    item,
                )
            }
            None => {
                *self.focus_item.borrow_mut() = Default::default();
                crate::input::FocusEventResult::FocusAccepted // We were removing the focus, treat that as OK
            }
        }
    }

    fn move_focus(&self, start_item: ItemRc, forward: impl Fn(ItemRc) -> ItemRc) -> Option<ItemRc> {
        let mut current_item = start_item;
        let mut visited = alloc::vec::Vec::new();

        loop {
            if current_item.is_visible()
                && self.publish_focus_item(&Some(current_item.clone()))
                    == crate::input::FocusEventResult::FocusAccepted
            {
                return Some(current_item); // Item was just published.
            }
            visited.push(current_item.clone());
            current_item = forward(current_item);

            if visited.iter().any(|i| *i == current_item) {
                return None; // Nothing to do: We took the focus_item already
            }
        }
    }

    /// Move keyboard focus to the next item
    pub fn focus_next_item(&self) {
        let component = self.component();
        let start_item = self
            .take_focus_item()
            .map(next_focus_item)
            .unwrap_or_else(|| ItemRc::new(component, 0));
        let end_item = self.move_focus(start_item.clone(), next_focus_item);
        let window_adapter = self.window_adapter();
        if let Some(window_adapter) = window_adapter.internal(crate::InternalToken) {
            window_adapter.handle_focus_change(Some(start_item), end_item);
        }
    }

    /// Move keyboard focus to the previous item.
    pub fn focus_previous_item(&self) {
        let component = self.component();
        let start_item = previous_focus_item(
            self.take_focus_item().unwrap_or_else(|| ItemRc::new(component, 0)),
        );
        let end_item = self.move_focus(start_item.clone(), previous_focus_item);
        let window_adapter = self.window_adapter();
        if let Some(window_adapter) = window_adapter.internal(crate::InternalToken) {
            window_adapter.handle_focus_change(Some(start_item), end_item);
        }
    }

    /// Marks the window to be the active window. This typically coincides with the keyboard
    /// focus. One exception though is when a popup is shown, in which case the window may
    /// remain active but temporarily loose focus to the popup.
    ///
    /// This results in WindowFocusReceived and WindowFocusLost events.
    pub fn set_active(&self, have_focus: bool) {
        self.pinned_fields.as_ref().project_ref().active.set(have_focus);

        let event = if have_focus {
            crate::input::FocusEvent::WindowReceivedFocus
        } else {
            crate::input::FocusEvent::WindowLostFocus
        };

        if let Some(focus_item) = self.focus_item.borrow().upgrade() {
            focus_item.borrow().as_ref().focus_event(&event, &self.window_adapter(), &focus_item);
        }

        // If we lost focus due to for example a global shortcut, then when we regain focus
        // should not assume that the modifiers are in the same state.
        if !have_focus {
            self.modifiers.take();
        }
    }

    /// Returns true of the window is the active window. That typically implies having the
    /// keyboard focus, except when a popup is shown and temporarily takes the focus.
    pub fn active(&self) -> bool {
        self.pinned_fields.as_ref().project_ref().active.get()
    }

    /// If the component's root item is a Window element, then this function synchronizes its properties, such as the title
    /// for example, with the properties known to the windowing system.
    pub fn update_window_properties(&self) {
        let window_adapter = self.window_adapter();

        // No `if !dirty { return; }` check here because the backend window may be newly mapped and not up-to-date, so force
        // an evaluation.
        self.pinned_fields
            .as_ref()
            .project_ref()
            .window_properties_tracker
            .evaluate_as_dependency_root(|| {
                window_adapter.update_window_properties(WindowProperties(self));
            });
    }

    /// Calls the render_components to render the main component and any sub-window components, tracked by a
    /// property dependency tracker.
    /// Returns None if no component is set yet.
    pub fn draw_contents<T>(
        &self,
        render_components: impl FnOnce(&[(&ComponentRc, LogicalPoint)]) -> T,
    ) -> Option<T> {
        let draw_fn = || {
            let component_rc = self.try_component()?;

            let popup_component =
                self.active_popup.borrow().as_ref().and_then(|popup| match popup.location {
                    PopupWindowLocation::TopLevel(_) => None,
                    PopupWindowLocation::ChildWindow(coordinates) => {
                        Some((popup.component.clone(), coordinates))
                    }
                });

            Some(if let Some((popup_component, popup_coordinates)) = popup_component {
                render_components(&[
                    (&component_rc, LogicalPoint::default()),
                    (&popup_component, popup_coordinates),
                ])
            } else {
                render_components(&[(&component_rc, LogicalPoint::default())])
            })
        };

        self.pinned_fields
            .as_ref()
            .project_ref()
            .redraw_tracker
            .evaluate_as_dependency_root(draw_fn)
    }

    /// Registers the window with the windowing system, in order to render the component's items and react
    /// to input events once the event loop spins.
    pub fn show(&self) -> Result<(), PlatformError> {
        self.update_window_properties();
        self.window_adapter().set_visible(true)?;
        // Make sure that the window's inner size is in sync with the root window item's
        // width/height.
        self.set_window_item_geometry(
            self.window_adapter().size().to_logical(self.scale_factor()).to_euclid(),
        );

        Ok(())
    }

    /// De-registers the window with the windowing system.
    pub fn hide(&self) -> Result<(), PlatformError> {
        self.window_adapter().set_visible(false)
    }

    /// returns wether a dark theme is used
    pub fn dark_color_scheme(&self) -> bool {
        self.window_adapter()
            .internal(crate::InternalToken)
            .map_or(false, |x| x.dark_color_scheme())
    }

    /// Show a popup at the given position relative to the item
    pub fn show_popup(
        &self,
        popup_componentrc: &ComponentRc,
        position: Point,
        close_on_click: bool,
        parent_item: &ItemRc,
    ) {
        let position = parent_item.map_to_window(
            parent_item.geometry().origin + LogicalPoint::from_untyped(position).to_vector(),
        );
        let popup_component = ComponentRc::borrow_pin(popup_componentrc);
        let popup_root = popup_component.as_ref().get_item_ref(0);

        let (mut w, mut h) = if let Some(window_item) =
            ItemRef::downcast_pin::<crate::items::WindowItem>(popup_root)
        {
            (window_item.width(), window_item.height())
        } else {
            (LogicalLength::zero(), LogicalLength::zero())
        };

        let layout_info_h =
            popup_component.as_ref().layout_info(crate::layout::Orientation::Horizontal);
        let layout_info_v =
            popup_component.as_ref().layout_info(crate::layout::Orientation::Vertical);

        if w <= LogicalLength::zero() {
            w = LogicalLength::new(layout_info_h.preferred);
        }
        if h <= LogicalLength::zero() {
            h = LogicalLength::new(layout_info_v.preferred);
        }
        w = w.max(LogicalLength::new(layout_info_h.min)).min(LogicalLength::new(layout_info_h.max));
        h = h.max(LogicalLength::new(layout_info_v.min)).min(LogicalLength::new(layout_info_v.max));

        let size = LogicalSize::from_lengths(w, h);

        if let Some(window_item) = ItemRef::downcast_pin(popup_root) {
            let width_property =
                crate::items::WindowItem::FIELD_OFFSETS.width.apply_pin(window_item);
            let height_property =
                crate::items::WindowItem::FIELD_OFFSETS.height.apply_pin(window_item);
            width_property.set(size.width_length());
            height_property.set(size.height_length());
        };

        let location = match self
            .window_adapter()
            .internal(crate::InternalToken)
            .and_then(|x| x.create_popup(LogicalRect::new(position, size)))
        {
            None => {
                self.window_adapter().request_redraw();
                PopupWindowLocation::ChildWindow(position)
            }

            Some(window_adapter) => {
                WindowInner::from_pub(window_adapter.window()).set_component(popup_componentrc);
                PopupWindowLocation::TopLevel(window_adapter)
            }
        };

        self.active_popup.replace(Some(PopupWindow {
            location,
            component: popup_componentrc.clone(),
            close_on_click,
        }));
    }

    /// Removes any active popup.
    /// TODO: this function should take a component ref as parameter, to close a specific popup - i.e. when popup menus create a hierarchy of popups.
    pub fn close_popup(&self) {
        if let Some(current_popup) = self.active_popup.replace(None) {
            if let PopupWindowLocation::ChildWindow(offset) = current_popup.location {
                // Refresh the area that was previously covered by the popup.
                let popup_region = crate::properties::evaluate_no_tracking(|| {
                    let popup_component = ComponentRc::borrow_pin(&current_popup.component);
                    popup_component.as_ref().get_item_ref(0).as_ref().geometry()
                })
                .translate(offset.to_vector());

                if !popup_region.is_empty() {
                    let window_adapter = self.window_adapter();
                    window_adapter.renderer().mark_dirty_region(popup_region.to_box2d());
                    window_adapter.request_redraw();
                }
            }
        }
    }

    /// Returns true if the currently active popup is configured to close on click. None if there is no active popup.
    pub fn close_popup_after_click(&self) -> bool {
        self.active_popup.borrow().as_ref().map_or(false, |popup| popup.close_on_click)
    }

    /// Returns the scale factor set on the window, as provided by the windowing system.
    pub fn scale_factor(&self) -> f32 {
        self.pinned_fields.as_ref().project_ref().scale_factor.get()
    }

    /// Sets the scale factor for the window. This is set by the backend or for testing.
    pub(crate) fn set_scale_factor(&self, factor: f32) {
        self.pinned_fields.scale_factor.set(factor)
    }

    /// Returns the scale factor set on the window, as provided by the windowing system.
    pub fn text_input_focused(&self) -> bool {
        self.pinned_fields.as_ref().project_ref().text_input_focused.get()
    }

    /// Sets the scale factor for the window. This is set by the backend or for testing.
    pub fn set_text_input_focused(&self, value: bool) {
        self.pinned_fields.text_input_focused.set(value)
    }

    /// Returns the window item that is the first item in the component.
    pub fn window_item(&self) -> Option<VRcMapped<ComponentVTable, crate::items::WindowItem>> {
        self.try_component().and_then(|component_rc| {
            ItemRc::new(component_rc, 0).downcast::<crate::items::WindowItem>()
        })
    }

    /// Sets the size of the window item. This method is typically called in response to receiving a
    /// window resize event from the windowing system.
    pub(crate) fn set_window_item_geometry(&self, size: LogicalSize) {
        if let Some(component_rc) = self.try_component() {
            let component = ComponentRc::borrow_pin(&component_rc);
            let root_item = component.as_ref().get_item_ref(0);
            if let Some(window_item) = ItemRef::downcast_pin::<crate::items::WindowItem>(root_item)
            {
                window_item.width.set(size.width_length());
                window_item.height.set(size.height_length());
            }
        }
    }

    /// Sets the close_requested callback. The callback will be run when the user tries to close a window.
    pub fn on_close_requested(&self, mut callback: impl FnMut() -> CloseRequestResponse + 'static) {
        self.close_requested.set_handler(move |()| callback());
    }

    /// Runs the close_requested callback.
    /// If the callback returns KeepWindowShown, this function returns false. That should prevent the Window from closing.
    /// Otherwise it returns true, which allows the Window to hide.
    pub fn request_close(&self) -> bool {
        match self.close_requested.call(&()) {
            CloseRequestResponse::HideWindow => true,
            CloseRequestResponse::KeepWindowShown => false,
        }
    }

    /// Returns the upgraded window adapter
    pub fn window_adapter(&self) -> Rc<dyn WindowAdapter> {
        self.window_adapter_weak.upgrade().unwrap()
    }

    /// Private access to the WindowInner for a given window.
    pub fn from_pub(window: &crate::api::Window) -> &Self {
        &window.0
    }
}

/// Internal alias for `Rc<dyn WindowAdapter>`.
pub type WindowAdapterRc = Rc<dyn WindowAdapter>;

/// This module contains the functions needed to interface with the event loop and window traits
/// from outside the Rust language.
#[cfg(feature = "ffi")]
pub mod ffi {
    #![allow(unsafe_code)]
    #![allow(clippy::missing_safety_doc)]

    use super::*;
    use crate::api::{RenderingNotifier, RenderingState, SetRenderingNotifierError};
    use crate::graphics::IntSize;
    use crate::graphics::Size;

    /// This enum describes a low-level access to specific graphics APIs used
    /// by the renderer.
    #[repr(C)]
    pub enum GraphicsAPI {
        /// The rendering is done using OpenGL.
        NativeOpenGL,
    }

    #[allow(non_camel_case_types)]
    type c_void = ();

    /// Same layout as WindowAdapterRc
    #[repr(C)]
    pub struct WindowAdapterRcOpaque(*const c_void, *const c_void);

    /// Releases the reference to the windowrc held by handle.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_drop(handle: *mut WindowAdapterRcOpaque) {
        assert_eq!(
            core::mem::size_of::<Rc<dyn WindowAdapter>>(),
            core::mem::size_of::<WindowAdapterRcOpaque>()
        );
        core::ptr::read(handle as *mut Rc<dyn WindowAdapter>);
    }

    /// Releases the reference to the component window held by handle.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_clone(
        source: *const WindowAdapterRcOpaque,
        target: *mut WindowAdapterRcOpaque,
    ) {
        assert_eq!(
            core::mem::size_of::<Rc<dyn WindowAdapter>>(),
            core::mem::size_of::<WindowAdapterRcOpaque>()
        );
        let window = &*(source as *const Rc<dyn WindowAdapter>);
        core::ptr::write(target as *mut Rc<dyn WindowAdapter>, window.clone());
    }

    /// Spins an event loop and renders the items of the provided component in this window.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_show(handle: *const WindowAdapterRcOpaque) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);

        window_adapter.set_visible(true).unwrap();
    }

    /// Spins an event loop and renders the items of the provided component in this window.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_hide(handle: *const WindowAdapterRcOpaque) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.set_visible(false).unwrap();
    }

    /// Returns the visibility state of the window. This function can return false even if you previously called show()
    /// on it, for example if the user minimized the window.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_is_visible(
        handle: *const WindowAdapterRcOpaque,
    ) -> bool {
        let window = &*(handle as *const Rc<dyn WindowAdapter>);
        window.internal(crate::InternalToken).map_or(false, |w| w.is_visible())
    }

    /// Returns the window scale factor.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_get_scale_factor(
        handle: *const WindowAdapterRcOpaque,
    ) -> f32 {
        assert_eq!(
            core::mem::size_of::<Rc<dyn WindowAdapter>>(),
            core::mem::size_of::<WindowAdapterRcOpaque>()
        );
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        WindowInner::from_pub(window_adapter.window()).scale_factor()
    }

    /// Sets the window scale factor, merely for testing purposes.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_set_scale_factor(
        handle: *const WindowAdapterRcOpaque,
        value: f32,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        WindowInner::from_pub(window_adapter.window()).set_scale_factor(value)
    }

    /// Returns the text-input-focused property value.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_get_text_input_focused(
        handle: *const WindowAdapterRcOpaque,
    ) -> bool {
        assert_eq!(
            core::mem::size_of::<Rc<dyn WindowAdapter>>(),
            core::mem::size_of::<WindowAdapterRcOpaque>()
        );
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        WindowInner::from_pub(window_adapter.window()).text_input_focused()
    }

    /// Set the text-input-focused property.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_set_text_input_focused(
        handle: *const WindowAdapterRcOpaque,
        value: bool,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        WindowInner::from_pub(window_adapter.window()).set_text_input_focused(value)
    }

    /// Sets the focus item.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_set_focus_item(
        handle: *const WindowAdapterRcOpaque,
        focus_item: &ItemRc,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        WindowInner::from_pub(window_adapter.window()).set_focus_item(focus_item)
    }

    /// Associates the window with the given component.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_set_component(
        handle: *const WindowAdapterRcOpaque,
        component: &ComponentRc,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        WindowInner::from_pub(window_adapter.window()).set_component(component)
    }

    /// Show a popup.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_show_popup(
        handle: *const WindowAdapterRcOpaque,
        popup: &ComponentRc,
        position: crate::graphics::Point,
        close_on_click: bool,
        parent_item: &ItemRc,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        WindowInner::from_pub(window_adapter.window()).show_popup(
            popup,
            position,
            close_on_click,
            parent_item,
        );
    }
    /// Close the current popup
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_close_popup(handle: *const WindowAdapterRcOpaque) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        WindowInner::from_pub(window_adapter.window()).close_popup();
    }

    /// C binding to the set_rendering_notifier() API of Window
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_set_rendering_notifier(
        handle: *const WindowAdapterRcOpaque,
        callback: extern "C" fn(
            rendering_state: RenderingState,
            graphics_api: GraphicsAPI,
            user_data: *mut c_void,
        ),
        drop_user_data: extern "C" fn(user_data: *mut c_void),
        user_data: *mut c_void,
        error: *mut SetRenderingNotifierError,
    ) -> bool {
        struct CNotifier {
            callback: extern "C" fn(
                rendering_state: RenderingState,
                graphics_api: GraphicsAPI,
                user_data: *mut c_void,
            ),
            drop_user_data: extern "C" fn(*mut c_void),
            user_data: *mut c_void,
        }

        impl Drop for CNotifier {
            fn drop(&mut self) {
                (self.drop_user_data)(self.user_data)
            }
        }

        impl RenderingNotifier for CNotifier {
            fn notify(&mut self, state: RenderingState, graphics_api: &crate::api::GraphicsAPI) {
                let cpp_graphics_api = match graphics_api {
                    crate::api::GraphicsAPI::NativeOpenGL { .. } => GraphicsAPI::NativeOpenGL,
                    crate::api::GraphicsAPI::WebGL { .. } => unreachable!(), // We don't support wasm with C++
                };
                (self.callback)(state, cpp_graphics_api, self.user_data)
            }
        }

        let window = &*(handle as *const Rc<dyn WindowAdapter>);
        match window.renderer().set_rendering_notifier(Box::new(CNotifier {
            callback,
            drop_user_data,
            user_data,
        })) {
            Ok(()) => true,
            Err(err) => {
                *error = err;
                false
            }
        }
    }

    /// C binding to the on_close_requested() API of Window
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_on_close_requested(
        handle: *const WindowAdapterRcOpaque,
        callback: extern "C" fn(user_data: *mut c_void) -> CloseRequestResponse,
        drop_user_data: extern "C" fn(user_data: *mut c_void),
        user_data: *mut c_void,
    ) {
        struct WithUserData {
            callback: extern "C" fn(user_data: *mut c_void) -> CloseRequestResponse,
            drop_user_data: extern "C" fn(*mut c_void),
            user_data: *mut c_void,
        }

        impl Drop for WithUserData {
            fn drop(&mut self) {
                (self.drop_user_data)(self.user_data)
            }
        }

        impl WithUserData {
            fn call(&self) -> CloseRequestResponse {
                (self.callback)(self.user_data)
            }
        }

        let with_user_data = WithUserData { callback, drop_user_data, user_data };

        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().on_close_requested(move || with_user_data.call());
    }

    /// This function issues a request to the windowing system to redraw the contents of the window.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_request_redraw(handle: *const WindowAdapterRcOpaque) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.request_redraw();
    }

    /// Returns the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_position(
        handle: *const WindowAdapterRcOpaque,
        pos: &mut euclid::default::Point2D<i32>,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        *pos = window_adapter.position().unwrap_or_default().to_euclid()
    }

    /// Sets the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    /// Note that on some windowing systems, such as Wayland, this functionality is not available.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_set_physical_position(
        handle: *const WindowAdapterRcOpaque,
        pos: &euclid::default::Point2D<i32>,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.set_position(crate::api::PhysicalPosition::new(pos.x, pos.y).into());
    }

    /// Sets the position of the window on the screen, in physical screen coordinates and including
    /// a window frame (if present).
    /// Note that on some windowing systems, such as Wayland, this functionality is not available.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_set_logical_position(
        handle: *const WindowAdapterRcOpaque,
        pos: &euclid::default::Point2D<f32>,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.set_position(crate::api::LogicalPosition::new(pos.x, pos.y).into());
    }

    /// Returns the size of the window on the screen, in physical screen coordinates and excluding
    /// a window frame (if present).
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_size(handle: *const WindowAdapterRcOpaque) -> IntSize {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.size().to_euclid().cast()
    }

    /// Resizes the window to the specified size on the screen, in physical pixels and excluding
    /// a window frame (if present).
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_set_physical_size(
        handle: *const WindowAdapterRcOpaque,
        size: &IntSize,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().set_size(crate::api::PhysicalSize::new(size.width, size.height));
    }

    /// Resizes the window to the specified size on the screen, in physical pixels and excluding
    /// a window frame (if present).
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_set_logical_size(
        handle: *const WindowAdapterRcOpaque,
        size: &Size,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().set_size(crate::api::LogicalSize::new(size.width, size.height));
    }

    /// Return wether the style is using a dark theme
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_dark_color_scheme(
        handle: *const WindowAdapterRcOpaque,
    ) -> bool {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.internal(crate::InternalToken).map_or(false, |x| x.dark_color_scheme())
    }

    /// Dispatch a key pressed or release event
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_dispatch_key_event(
        handle: *const WindowAdapterRcOpaque,
        event: &crate::input::KeyInputEvent,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().0.process_key_input(event.clone());
    }

    /// Dispatch a mouse event
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_dispatch_pointer_event(
        handle: *const WindowAdapterRcOpaque,
        event: crate::input::MouseEvent,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().0.process_mouse_input(event);
    }

    /// Dispatch a window event
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_dispatch_event(
        handle: *const WindowAdapterRcOpaque,
        event: &crate::platform::WindowEvent,
    ) {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().dispatch_event(event.clone());
    }
}

#[test]
fn test_empty_window() {
    // Test that when creating an empty window without a component, we don't panic when render() is called.
    // This isn't typically done intentionally, but for example if we receive a paint event in Qt before a component
    // is set, this may happen. Concretely as per #2799 this could happen with popups where the call to
    // QWidget::show() with egl delivers an immediate paint event, before we've had a chance to call set_component.
    // Let's emulate this scenario here using public platform API.

    let msw = crate::software_renderer::MinimalSoftwareWindow::new(
        crate::software_renderer::RepaintBufferType::NewBuffer,
    );
    msw.window().request_redraw();
    let mut region = None;
    let render_called = msw.draw_if_needed(|renderer| {
        let mut buffer =
            crate::graphics::SharedPixelBuffer::<crate::graphics::Rgb8Pixel>::new(100, 100);
        let stride = buffer.width() as usize;
        region = Some(renderer.render(buffer.make_mut_slice(), stride));
    });
    assert!(render_called);
    let region = region.unwrap();
    assert_eq!(region.bounding_box_size(), PhysicalSize::default());
    assert_eq!(region.bounding_box_origin(), PhysicalPosition::default());
}
