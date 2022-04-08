// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore backtab

#![warn(missing_docs)]
//! Exposed Window API

use crate::api::CloseRequestResponse;
use crate::component::{ComponentRc, ComponentWeak};
use crate::graphics::{Point, Rect, Size};
use crate::input::{
    key_codes, KeyEvent, KeyEventType, MouseEvent, MouseInputState, TextCursorBlinker,
};
use crate::item_tree::ItemRc;
use crate::items::{ItemRef, MouseCursor};
use crate::properties::{Property, PropertyTracker};
use crate::{Callback, Coord};
use alloc::boxed::Box;
use alloc::rc::{Rc, Weak};
use core::cell::{Cell, RefCell};
use core::pin::Pin;

fn next_focus_item(item: ItemRc) -> ItemRc {
    item.next_focus_item()
}

fn previous_focus_item(item: ItemRc) -> ItemRc {
    item.previous_focus_item()
}

/// This trait represents the interface that the generated code and the run-time
/// require in order to implement functionality such as device-independent pixels,
/// window resizing and other typically windowing system related tasks.
pub trait PlatformWindow {
    /// Registers the window with the windowing system.
    fn show(self: Rc<Self>);
    /// De-registers the window from the windowing system.
    fn hide(self: Rc<Self>);
    /// Issue a request to the windowing system to re-render the contents of the window. This is typically an asynchronous
    /// request.
    fn request_redraw(&self);

    /// This function is called by the generated code when a component and therefore its tree of items are destroyed. The
    /// implementation typically uses this to free the underlying graphics resources cached via [`crate::graphics::RenderingCache`].
    fn free_graphics_resources<'a>(&self, items: &mut dyn Iterator<Item = Pin<ItemRef<'a>>>);

    /// This function is called through the public API to register a callback that the backend needs to invoke during
    /// different phases of rendering.
    fn set_rendering_notifier(
        &self,
        _callback: Box<dyn crate::api::RenderingNotifier>,
    ) -> Result<(), crate::api::SetRenderingNotifierError> {
        Err(crate::api::SetRenderingNotifierError::Unsupported)
    }

    /// Show a popup at the given position
    fn show_popup(&self, popup: &ComponentRc, position: Point);

    /// Notify the platform window about the closure of a previously opened popup.
    fn close_popup(&self, _popup: &PopupWindow) {}

    /// Request for the event loop to wake up and call [`Window::update_window_properties()`].
    fn request_window_properties_update(&self);
    /// Request for the given title string to be set to the windowing system for use as window title.
    fn apply_window_properties(&self, window_item: Pin<&crate::items::WindowItem>);

    /// Apply the given horizontal and vertical constraints to the window. This typically involves communication
    /// minimum/maximum sizes to the windowing system, for example.
    fn apply_geometry_constraint(
        &self,
        constraints_horizontal: crate::layout::LayoutInfo,
        constraints_vertical: crate::layout::LayoutInfo,
    );

    /// Set the mouse cursor
    fn set_mouse_cursor(&self, cursor: MouseCursor);

    /// Returns the size of the given text in logical pixels.
    /// When set, `max_width` means that one need to wrap the text so it does not go further than that
    fn text_size(
        &self,
        font_request: crate::graphics::FontRequest,
        text: &str,
        max_width: Option<Coord>,
    ) -> Size;

    /// Returns the (UTF-8) byte offset in the text property that refers to the character that contributed to
    /// the glyph cluster that's visually nearest to the given coordinate. This is used for hit-testing,
    /// for example when receiving a mouse click into a text field. Then this function returns the "cursor"
    /// position.
    fn text_input_byte_offset_for_position(
        &self,
        text_input: Pin<&crate::items::TextInput>,
        pos: Point,
    ) -> usize;

    /// That's the opposite of [`Self::text_input_byte_offset_for_position`]
    /// It takes a (UTF-8) byte offset in the text property, and returns a Rectangle
    /// left to the char. It is one logical pixel wide and ends at the baseline.
    fn text_input_cursor_rect_for_byte_offset(
        &self,
        text_input: Pin<&crate::items::TextInput>,
        byte_offset: usize,
    ) -> Rect;

    /// This is called when the virtual keyboard should be shown because a widget that
    /// uses input has the focus.
    fn show_virtual_keyboard(&self, _: crate::items::InputType) {}
    /// This is called when the widget that needed the keyboard loses focus
    fn hide_virtual_keyboard(&self) {}

    /// Return self as any so the backend can upcast
    fn as_any(&self) -> &dyn core::any::Any;
}

struct WindowPropertiesTracker {
    window_weak: Weak<Window>,
}

impl crate::properties::PropertyChangeHandler for WindowPropertiesTracker {
    fn notify(&self) {
        if let Some(platform_window) =
            self.window_weak.upgrade().and_then(|window| window.platform_window.get().cloned())
        {
            platform_window.request_window_properties_update();
        };
    }
}

struct WindowRedrawTracker {
    window_weak: Weak<Window>,
}

impl crate::properties::PropertyChangeHandler for WindowRedrawTracker {
    fn notify(&self) {
        if let Some(platform_window) =
            self.window_weak.upgrade().and_then(|window| window.platform_window.get().cloned())
        {
            platform_window.request_redraw();
        };
    }
}

/// This enum describes the different ways a popup can be rendered by the back-end.
pub enum PopupWindowLocation {
    /// The popup is rendered in its own top-level window that is know to the windowing system.
    TopLevel(Rc<Window>),
    /// The popup is rendered as an embedded child window at the given position.
    ChildWindow(Point),
}

/// This structure defines a graphical element that is designed to pop up from the surrounding
/// UI content, for example to show a context menu.
pub struct PopupWindow {
    /// The location defines where the pop up is rendered.
    pub location: PopupWindowLocation,
    /// The component that is responsible for providing the popup content.
    pub component: ComponentRc,
}

/// Structure that represent a Window in the runtime
pub struct Window {
    /// FIXME! use Box instead;
    platform_window: once_cell::unsync::OnceCell<Rc<dyn PlatformWindow>>,
    component: RefCell<ComponentWeak>,
    mouse_input_state: Cell<MouseInputState>,
    redraw_tracker: once_cell::unsync::OnceCell<Pin<Box<PropertyTracker<WindowRedrawTracker>>>>,
    window_properties_tracker:
        once_cell::unsync::OnceCell<Pin<Box<PropertyTracker<WindowPropertiesTracker>>>>,
    /// Gets dirty when the layout restrictions, or some other property of the windows change
    meta_properties_tracker: Pin<Rc<PropertyTracker>>,

    focus_item: RefCell<crate::item_tree::ItemWeak>,
    cursor_blinker: RefCell<pin_weak::rc::PinWeak<crate::input::TextCursorBlinker>>,

    scale_factor: Pin<Box<Property<f32>>>,
    active: Pin<Box<Property<bool>>>,
    active_popup: RefCell<Option<PopupWindow>>,
    close_requested: Callback<(), CloseRequestResponse>,
}

impl Drop for Window {
    fn drop(&mut self) {
        if let Some(existing_blinker) = self.cursor_blinker.borrow().upgrade() {
            existing_blinker.stop();
        }
    }
}

impl Window {
    /// Create a new instance of the window, given the platform_window factory fn
    pub fn new(
        platform_window_fn: impl FnOnce(&Weak<Window>) -> Rc<dyn PlatformWindow>,
    ) -> Rc<Self> {
        #![allow(unused_mut)]
        let window = Rc::new(Self {
            platform_window: Default::default(),
            component: Default::default(),
            mouse_input_state: Default::default(),
            redraw_tracker: Default::default(),
            window_properties_tracker: Default::default(),
            meta_properties_tracker: Rc::pin(Default::default()),
            focus_item: Default::default(),
            cursor_blinker: Default::default(),
            scale_factor: Box::pin(Property::new_named(1., "i_slint_core::Window::scale_factor")),
            active: Box::pin(Property::new_named(false, "i_slint_core::Window::active")),
            active_popup: Default::default(),
            close_requested: Default::default(),
        });
        let window_weak = Rc::downgrade(&window);
        window.platform_window.set(platform_window_fn(&window_weak)).ok().unwrap();

        let mut window_properties_tracker =
            PropertyTracker::new_with_change_handler(WindowPropertiesTracker {
                window_weak: window_weak.clone(),
            });

        let mut redraw_tracker =
            PropertyTracker::new_with_change_handler(WindowRedrawTracker { window_weak });

        #[cfg(slint_debug_property)]
        {
            window_properties_tracker
                .set_debug_name("i_slint_core::Window::window_properties_tracker".into());
            redraw_tracker.set_debug_name("i_slint_core::Window::redraw_tracker".into());
        }

        window.window_properties_tracker.set(Box::pin(window_properties_tracker)).ok().unwrap();
        window.redraw_tracker.set(Box::pin(redraw_tracker)).ok().unwrap();

        window
    }

    /// Associates this window with the specified component. Further event handling and rendering, etc. will be
    /// done with that component.
    pub fn set_component(&self, component: &ComponentRc) {
        self.close_popup();
        self.focus_item.replace(Default::default());
        self.mouse_input_state.replace(Default::default());
        self.component.replace(ComponentRc::downgrade(component));
        self.meta_properties_tracker.set_dirty(); // component changed, layout constraints for sure must be re-calculated
        self.request_window_properties_update();
        self.request_redraw();
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
    pub fn process_mouse_input(self: Rc<Self>, mut event: MouseEvent) {
        crate::animations::update_animations();

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

                if let MouseEvent::MousePressed { pos, .. } = &event {
                    // close the popup if one press outside the popup
                    let geom = ComponentRc::borrow_pin(popup_component)
                        .as_ref()
                        .get_item_ref(0)
                        .as_ref()
                        .geometry();
                    if !geom.contains(*pos) {
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
            &self.clone(),
            self.mouse_input_state.take(),
        ));

        if embedded_popup_component.is_some() {
            //FIXME: currently the ComboBox is the only thing that uses the popup, and it should close automatically
            // on release.  But ideally, there would be API to close the popup rather than always closing it on release
            if matches!(event, MouseEvent::MouseReleased { .. }) {
                self.close_popup();
            }
        }
    }
    /// Receive a key event and pass it to the items of the component to
    /// change their state.
    ///
    /// Arguments:
    /// * `event`: The key event received by the windowing system.
    /// * `component`: The Slint compiled component that provides the tree of items.
    pub fn process_key_input(self: Rc<Self>, event: &KeyEvent) {
        let mut item = self.focus_item.borrow().clone();
        while let Some(focus_item) = item.upgrade() {
            if focus_item.borrow().as_ref().key_event(event, &self.clone())
                == crate::input::KeyEventResult::EventAccepted
            {
                return;
            }
            item = focus_item.parent_item();
        }

        // Make Tab/Backtab handle keyboard focus
        if event.text.starts_with(key_codes::Tab) && event.event_type == KeyEventType::KeyPressed {
            self.focus_next_item();
        } else if event.text.starts_with(key_codes::Backtab)
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
    pub fn set_focus_item(self: Rc<Self>, focus_item: &ItemRc) {
        self.take_focus_item();
        self.move_focus(focus_item.clone(), next_focus_item);
    }

    /// Sets the focus on the window to true or false, depending on the have_focus argument.
    /// This results in WindowFocusReceived and WindowFocusLost events.
    pub fn set_focus(self: Rc<Self>, have_focus: bool) {
        let event = if have_focus {
            crate::input::FocusEvent::WindowReceivedFocus
        } else {
            crate::input::FocusEvent::WindowLostFocus
        };

        if let Some(focus_item) = self.as_ref().focus_item.borrow().upgrade() {
            focus_item.borrow().as_ref().focus_event(&event, &self);
        }
    }

    /// Take the focus_item out of this Window
    ///
    /// This sends the FocusOut event!
    fn take_focus_item(self: &Rc<Self>) -> Option<ItemRc> {
        let focus_item = self.as_ref().focus_item.take();

        if let Some(focus_item_rc) = focus_item.upgrade() {
            focus_item_rc.borrow().as_ref().focus_event(&crate::input::FocusEvent::FocusOut, &self);
            Some(focus_item_rc)
        } else {
            None
        }
    }

    /// Publish the new focus_item to this Window and return the FocusEventResult
    ///
    /// This sends a FocusIn event!
    fn publish_focus_item(self: Rc<Self>, item: &Option<ItemRc>) -> crate::input::FocusEventResult {
        match item {
            Some(item) => {
                *self.as_ref().focus_item.borrow_mut() = item.downgrade();
                item.borrow().as_ref().focus_event(&crate::input::FocusEvent::FocusIn, &self)
            }
            None => {
                *self.as_ref().focus_item.borrow_mut() = Default::default();
                crate::input::FocusEventResult::FocusAccepted // We were removing the focus, treat that as OK
            }
        }
    }

    fn move_focus(self: Rc<Self>, start_item: ItemRc, forward: impl Fn(ItemRc) -> ItemRc) {
        let mut current_item = start_item;
        let mut visited = alloc::vec::Vec::new();

        loop {
            if current_item.is_visible()
                && self.clone().publish_focus_item(&Some(current_item.clone()))
                    == crate::input::FocusEventResult::FocusAccepted
            {
                return; // Item was just published.
            }
            visited.push(current_item.clone());
            current_item = forward(current_item);

            if visited.iter().any(|i| *i == current_item) {
                return; // Nothing to do: We took the focus_item already
            }
        }
    }

    /// Move keyboard focus to the next item
    pub fn focus_next_item(self: Rc<Self>) {
        let component = self.component();
        let start_item = self
            .take_focus_item()
            .map(next_focus_item)
            .unwrap_or_else(|| ItemRc::new(component, 0));
        self.move_focus(start_item, next_focus_item);
    }

    /// Move keyboard focus to the previous item.
    pub fn focus_previous_item(self: Rc<Self>) {
        let component = self.component();
        let start_item = previous_focus_item(
            self.take_focus_item().unwrap_or_else(|| ItemRc::new(component, 0)),
        );
        self.move_focus(start_item, previous_focus_item);
    }

    /// Marks the window to be the active window. This typically coincides with the keyboard
    /// focus. One exception though is when a popup is shown, in which case the window may
    /// remain active but temporarily loose focus to the popup.
    pub fn set_active(&self, active: bool) {
        self.active.as_ref().set(active);
    }

    /// Returns true of the window is the active window. That typically implies having the
    /// keyboard focus, except when a popup is shown and temporarily takes the focus.
    pub fn active(&self) -> bool {
        self.active.as_ref().get()
    }

    /// If the component's root item is a Window element, then this function synchronizes its properties, such as the title
    /// for example, with the properties known to the windowing system.
    pub fn update_window_properties(&self) {
        if let Some(window_properties_tracker) = self.window_properties_tracker.get() {
            // No `if !dirty { return; }` check here because the backend window may be newly mapped and not up-to-date, so force
            // an evaluation.
            window_properties_tracker.as_ref().evaluate_as_dependency_root(|| {
                let component = self.component();
                let component = ComponentRc::borrow_pin(&component);
                let root_item = component.as_ref().get_item_ref(0);

                if let Some(window_item) =
                    ItemRef::downcast_pin::<crate::items::WindowItem>(root_item)
                {
                    self.platform_window.get().unwrap().apply_window_properties(window_item);
                }
            });
        }
    }

    /// Calls the render_components to render the main component and any sub-window components, tracked by a
    /// property dependency tracker.
    pub fn draw_contents(self: Rc<Self>, render_components: impl FnOnce(&[(&ComponentRc, Point)])) {
        let draw_fn = || {
            let component_rc = self.component();
            let component = ComponentRc::borrow_pin(&component_rc);

            self.meta_properties_tracker.as_ref().evaluate_if_dirty(|| {
                self.apply_geometry_constraint(
                    component.as_ref().layout_info(crate::layout::Orientation::Horizontal),
                    component.as_ref().layout_info(crate::layout::Orientation::Vertical),
                );
            });

            let popup_component =
                self.active_popup.borrow().as_ref().and_then(|popup| match popup.location {
                    PopupWindowLocation::TopLevel(_) => None,
                    PopupWindowLocation::ChildWindow(coordinates) => {
                        Some((popup.component.clone(), coordinates))
                    }
                });

            if let Some((popup_component, popup_coordinates)) = popup_component {
                render_components(&[
                    (&component_rc, Point::default()),
                    (&popup_component, popup_coordinates),
                ])
            } else {
                render_components(&[(&component_rc, Point::default())]);
            }
        };

        if let Some(redraw_tracker) = self.redraw_tracker.get() {
            redraw_tracker.as_ref().evaluate_as_dependency_root(draw_fn)
        } else {
            draw_fn()
        }
    }

    /// Registers the window with the windowing system, in order to render the component's items and react
    /// to input events once the event loop spins.
    pub fn show(&self) {
        self.platform_window.get().unwrap().clone().show();
        self.update_window_properties();
    }

    /// De-registers the window with the windowing system.
    pub fn hide(&self) {
        self.platform_window.get().unwrap().clone().hide();
    }

    /// Registers the specified window and component to be considered the active popup.
    /// Returns the size of the popup.
    pub fn set_active_popup(&self, popup: PopupWindow) -> Size {
        if matches!(popup.location, PopupWindowLocation::ChildWindow(..)) {
            self.meta_properties_tracker.set_dirty();
        }

        let popup_component = ComponentRc::borrow_pin(&popup.component);
        let popup_root = popup_component.as_ref().get_item_ref(0);

        let (mut w, mut h) = if let Some(window_item) =
            ItemRef::downcast_pin::<crate::items::WindowItem>(popup_root)
        {
            (window_item.width(), window_item.height())
        } else {
            (0 as Coord, 0 as Coord)
        };

        let layout_info_h =
            popup_component.as_ref().layout_info(crate::layout::Orientation::Horizontal);
        let layout_info_v =
            popup_component.as_ref().layout_info(crate::layout::Orientation::Vertical);

        if w <= 0 as Coord {
            w = layout_info_h.preferred;
        }
        if h <= 0 as Coord {
            h = layout_info_v.preferred;
        }
        w = w.clamp(layout_info_h.min, layout_info_h.max);
        h = h.clamp(layout_info_v.min, layout_info_v.max);

        let size = Size::new(w, h);

        if let Some(window_item) = ItemRef::downcast_pin(popup_root) {
            let width_property =
                crate::items::WindowItem::FIELD_OFFSETS.width.apply_pin(window_item);
            let height_property =
                crate::items::WindowItem::FIELD_OFFSETS.height.apply_pin(window_item);
            width_property.set(size.width);
            height_property.set(size.height);
        };

        self.active_popup.replace(Some(popup));

        size
    }

    /// Show a popup at the given position relative to the item
    pub fn show_popup(&self, popup: &ComponentRc, mut position: Point, parent_item: &ItemRc) {
        let mut parent_item = parent_item.clone();
        loop {
            position += parent_item.borrow().as_ref().geometry().origin.to_vector();
            parent_item = match parent_item.parent_item().upgrade() {
                None => break,
                Some(pi) => pi,
            }
        }
        self.platform_window.get().unwrap().show_popup(popup, position)
    }

    /// Removes any active popup.
    pub fn close_popup(&self) {
        if let Some(current_popup) = self.active_popup.replace(None) {
            if matches!(current_popup.location, PopupWindowLocation::ChildWindow(..)) {
                self.platform_window.get().unwrap().close_popup(&current_popup);
                // Refresh the area that was previously covered by the popup. I wonder if this
                // is still needed, shouldn't the redraw tracker be dirty due to the removal of
                // dependent properties?
                self.request_redraw();
            }
        }
    }

    /// Returns the scale factor set on the window, as provided by the windowing system.
    pub fn scale_factor(&self) -> f32 {
        self.scale_factor_property().get()
    }

    /// Returns the scale factor set on the window, as provided by the windowing system.
    pub fn scale_factor_property(&self) -> Pin<&Property<f32>> {
        self.scale_factor.as_ref()
    }

    /// Sets the scale factor for the window. This is set by the backend or for testing.
    pub fn set_scale_factor(&self, factor: f32) {
        self.scale_factor.as_ref().set(factor)
    }

    /// Returns the font properties that are set on the root item if it's a Window item.
    pub fn default_font_properties(&self) -> crate::graphics::FontRequest {
        self.try_component()
            .and_then(|component_rc| {
                let component = ComponentRc::borrow_pin(&component_rc);
                let root_item = component.as_ref().get_item_ref(0);
                ItemRef::downcast_pin(root_item).map(
                    |window_item: Pin<&crate::items::WindowItem>| {
                        window_item.default_font_properties()
                    },
                )
            })
            .unwrap_or_default()
    }

    /// Sets the size of the window item. This method is typically called in response to receiving a
    /// window resize event from the windowing system.
    /// Size is in logical pixels.
    pub fn set_window_item_geometry(&self, width: Coord, height: Coord) {
        if let Some(component_rc) = self.try_component() {
            let component = ComponentRc::borrow_pin(&component_rc);
            let root_item = component.as_ref().get_item_ref(0);
            if let Some(window_item) = ItemRef::downcast_pin::<crate::items::WindowItem>(root_item)
            {
                window_item.width.set(width);
                window_item.height.set(height);
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
}

impl core::ops::Deref for Window {
    type Target = dyn PlatformWindow;

    fn deref(&self) -> &Self::Target {
        self.platform_window.get().unwrap().as_ref()
    }
}

/// Internal trait used by generated code to access window internals.
pub trait WindowHandleAccess {
    /// Returns a reference to the window implementation.
    fn window_handle(&self) -> &Rc<Window>;
}

/// Internal alias for Rc<Window> so that it can be used in the vtable
/// functions and generate a good signature.
pub type WindowRc = Rc<Window>;

/// This module contains the functions needed to interface with the event loop and window traits
/// from outside the Rust language.
#[cfg(feature = "ffi")]
pub mod ffi {
    #![allow(unsafe_code)]
    #![allow(clippy::missing_safety_doc)]

    use super::*;
    use crate::api::{RenderingNotifier, RenderingState, SetRenderingNotifierError};
    use crate::slice::Slice;

    /// This enum describes a low-level access to specific graphics APIs used
    /// by the renderer.
    #[repr(C)]
    pub enum GraphicsAPI {
        /// The rendering is done using OpenGL.
        NativeOpenGL,
    }

    #[allow(non_camel_case_types)]
    type c_void = ();

    /// Same layout as WindowRc
    #[repr(C)]
    pub struct WindowRcOpaque(*const c_void);

    /// Releases the reference to the windowrc held by handle.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_drop(handle: *mut WindowRcOpaque) {
        assert_eq!(core::mem::size_of::<WindowRc>(), core::mem::size_of::<WindowRcOpaque>());
        core::ptr::read(handle as *mut WindowRc);
    }

    /// Releases the reference to the component window held by handle.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_clone(
        source: *const WindowRcOpaque,
        target: *mut WindowRcOpaque,
    ) {
        assert_eq!(core::mem::size_of::<WindowRc>(), core::mem::size_of::<WindowRcOpaque>());
        let window = &*(source as *const WindowRc);
        core::ptr::write(target as *mut WindowRc, window.clone());
    }

    /// Spins an event loop and renders the items of the provided component in this window.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_show(handle: *const WindowRcOpaque) {
        let window = &*(handle as *const WindowRc);
        window.show();
    }

    /// Spins an event loop and renders the items of the provided component in this window.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_hide(handle: *const WindowRcOpaque) {
        let window = &*(handle as *const WindowRc);
        window.hide();
    }

    /// Returns the window scale factor.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_get_scale_factor(handle: *const WindowRcOpaque) -> f32 {
        assert_eq!(core::mem::size_of::<WindowRc>(), core::mem::size_of::<WindowRcOpaque>());
        let window = &*(handle as *const WindowRc);
        window.scale_factor()
    }

    /// Sets the window scale factor, merely for testing purposes.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_set_scale_factor(
        handle: *const WindowRcOpaque,
        value: f32,
    ) {
        let window = &*(handle as *const WindowRc);
        window.set_scale_factor(value)
    }

    /// Sets the window scale factor, merely for testing purposes.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_free_graphics_resources<'a>(
        handle: *const WindowRcOpaque,
        items: &Slice<'a, Pin<ItemRef<'a>>>,
    ) {
        let window = &*(handle as *const WindowRc);
        window.free_graphics_resources(&mut items.iter().cloned())
    }

    /// Sets the focus item.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_set_focus_item(
        handle: *const WindowRcOpaque,
        focus_item: &ItemRc,
    ) {
        let window = &*(handle as *const WindowRc);
        window.clone().set_focus_item(focus_item)
    }

    /// Associates the window with the given component.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_set_component(
        handle: *const WindowRcOpaque,
        component: &ComponentRc,
    ) {
        let window = &*(handle as *const WindowRc);
        window.set_component(component)
    }

    /// Show a popup.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_show_popup(
        handle: *const WindowRcOpaque,
        popup: &ComponentRc,
        position: crate::graphics::Point,
        parent_item: &ItemRc,
    ) {
        let window = &*(handle as *const WindowRc);
        window.show_popup(popup, position, parent_item);
    }
    /// Close the current popup
    pub unsafe extern "C" fn slint_windowrc_close_popup(handle: *const WindowRcOpaque) {
        let window = &*(handle as *const WindowRc);
        window.close_popup();
    }

    /// C binding to the set_rendering_notifier() API of Window
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_set_rendering_notifier(
        handle: *const WindowRcOpaque,
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

        let window = &*(handle as *const WindowRc);
        match window.set_rendering_notifier(Box::new(CNotifier {
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
        handle: *const WindowRcOpaque,
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

        let window = &*(handle as *const WindowRc);
        window.on_close_requested(move || with_user_data.call());
    }

    /// This function issues a request to the windowing system to redraw the contents of the window.
    #[no_mangle]
    pub unsafe extern "C" fn slint_windowrc_request_redraw(handle: *const WindowRcOpaque) {
        let window = &*(handle as *const WindowRc);
        window.request_redraw();
    }
}
