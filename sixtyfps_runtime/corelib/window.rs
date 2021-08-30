/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#![warn(missing_docs)]
//! Exposed Window API

use crate::component::{ComponentRc, ComponentWeak};
use crate::graphics::{Point, Size};
use crate::input::{KeyEvent, MouseEvent, MouseInputState, TextCursorBlinker};
use crate::items::{ItemRc, ItemRef, ItemWeak};
use crate::properties::{Property, PropertyTracker};
use crate::slice::Slice;
use core::cell::Cell;
use core::pin::Pin;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

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
    fn free_graphics_resources<'a>(&self, items: &Slice<'a, Pin<ItemRef<'a>>>);

    /// Show a popup at the given position
    fn show_popup(&self, popup: &ComponentRc, position: Point);
    /// Close the active popup if any
    fn close_popup(&self);

    /// Request for the event loop to wake up and call [`Window::update_window_properties()`].
    fn request_window_properties_update(&self);
    /// Request for the given title string to be set to the windowing system for use as window title.
    fn apply_window_properties(&self, window_item: Pin<&crate::items::WindowItem>);

    /// Returns the size of the given text in logical pixels.
    /// When set, `max_width` means that one need to wrap the text so it does not go further than that
    fn text_size(
        &self,
        item_graphics_cache: &crate::item_rendering::CachedRenderingData,
        unresolved_font_request_getter: &dyn Fn() -> crate::graphics::FontRequest,
        text: &str,
        max_width: Option<f32>,
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
    /// FIXME: should only be exposed to the backend
    pub meta_properties_tracker: Pin<Rc<PropertyTracker>>,

    focus_item: RefCell<ItemWeak>,
    cursor_blinker: RefCell<pin_weak::rc::PinWeak<crate::input::TextCursorBlinker>>,

    scale_factor: Pin<Box<Property<f32>>>,
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
        let window = Rc::new(Self {
            platform_window: Default::default(),
            component: Default::default(),
            mouse_input_state: Default::default(),
            redraw_tracker: Default::default(),
            window_properties_tracker: Default::default(),
            meta_properties_tracker: Rc::pin(Default::default()),
            focus_item: Default::default(),
            cursor_blinker: Default::default(),
            scale_factor: Box::pin(Property::new(1.)),
        });
        let window_weak = Rc::downgrade(&window);
        window.platform_window.set(platform_window_fn(&window_weak)).ok().unwrap();

        window
            .window_properties_tracker
            .set(Box::pin(PropertyTracker::new_with_change_handler(WindowPropertiesTracker {
                window_weak: window_weak.clone(),
            })))
            .ok()
            .unwrap();
        window
            .redraw_tracker
            .set(Box::pin(PropertyTracker::new_with_change_handler(WindowRedrawTracker {
                window_weak,
            })))
            .ok()
            .unwrap();

        window
    }

    /// Associates this window with the specified component. Further event handling and rendering, etc. will be
    /// done with that component.
    pub fn set_component(&self, component: &ComponentRc) {
        if let Some(w) = self.platform_window.get() {
            w.close_popup(); // ensure the popup is closed
        }
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
    /// * `component`: The SixtyFPS compiled component that provides the tree of items.
    pub fn process_mouse_input(self: Rc<Self>, event: MouseEvent) {
        crate::animations::update_animations();
        let component = self.component.borrow().upgrade().unwrap();
        self.mouse_input_state.set(crate::input::process_mouse_input(
            component,
            event,
            &self.clone(),
            self.mouse_input_state.take(),
        ));
    }
    /// Receive a key event and pass it to the items of the component to
    /// change their state.
    ///
    /// Arguments:
    /// * `event`: The key event received by the windowing system.
    /// * `component`: The SixtyFPS compiled component that provides the tree of items.
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
        if let Some(old_focus_item) = self.as_ref().focus_item.borrow().upgrade() {
            old_focus_item
                .borrow()
                .as_ref()
                .focus_event(&crate::input::FocusEvent::FocusOut, &self);
        }

        *self.as_ref().focus_item.borrow_mut() = focus_item.downgrade();

        focus_item.borrow().as_ref().focus_event(&crate::input::FocusEvent::FocusIn, &self);
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

    /// Calls draw_fn using a [`crate::properties::PropertyTracker`], which is set up to issue a call to [`PlatformWindow::request_redraw`]
    /// when any properties accessed during drawing change.
    pub fn draw_tracked<R>(self: Rc<Self>, draw_fn: impl FnOnce() -> R) -> R {
        if let Some(redraw_tracker) = self.redraw_tracker.get() {
            redraw_tracker.as_ref().evaluate_as_dependency_root(|| draw_fn())
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
    fn window_handle(&self) -> &std::rc::Rc<Window>;
}

/// Internal alias for Rc<Window> so that it can be used in the vtable
/// functions and generate a good signature.
pub type WindowRc = std::rc::Rc<Window>;

/// Internal module to define the public Window API, for re-export in the regular Rust crate
/// and the interpreter crate.
pub mod api {
    /// This type represents a window towards the windowing system, that's used to render the
    /// scene of a component. It provides API to control windowing system specific aspects such
    /// as the position on the screen.
    #[repr(transparent)]
    pub struct Window(pub(super) std::rc::Rc<super::Window>);

    #[doc(hidden)]
    impl From<super::WindowRc> for Window {
        fn from(window: super::WindowRc) -> Self {
            Self(window)
        }
    }

    impl Window {
        /// Registers the window with the windowing system in order to make it visible on the screen.
        pub fn show(&self) {
            self.0.show();
        }

        /// De-registers the window from the windowing system, therefore hiding it.
        pub fn hide(&self) {
            self.0.hide();
        }
    }
}

impl WindowHandleAccess for api::Window {
    fn window_handle(&self) -> &std::rc::Rc<Window> {
        &self.0
    }
}

/// This module contains the functions needed to interface with the event loop and window traits
/// from outside the Rust language.
#[cfg(feature = "ffi")]
pub mod ffi {
    #![allow(unsafe_code)]

    use super::*;

    #[allow(non_camel_case_types)]
    type c_void = ();

    /// Same layout as WindowRc
    #[repr(C)]
    pub struct WindowRcOpaque(*const c_void);

    /// Releases the reference to the windowrc held by handle.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_windowrc_drop(handle: *mut WindowRcOpaque) {
        assert_eq!(core::mem::size_of::<WindowRc>(), core::mem::size_of::<WindowRcOpaque>());
        core::ptr::read(handle as *mut WindowRc);
    }

    /// Releases the reference to the component window held by handle.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_windowrc_clone(
        source: *const WindowRcOpaque,
        target: *mut WindowRcOpaque,
    ) {
        assert_eq!(core::mem::size_of::<WindowRc>(), core::mem::size_of::<WindowRcOpaque>());
        let window = &*(source as *const WindowRc);
        core::ptr::write(target as *mut WindowRc, window.clone());
    }

    /// Spins an event loop and renders the items of the provided component in this window.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_windowrc_show(handle: *const WindowRcOpaque) {
        let window = &*(handle as *const WindowRc);
        window.show();
    }

    /// Spins an event loop and renders the items of the provided component in this window.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_windowrc_hide(handle: *const WindowRcOpaque) {
        let window = &*(handle as *const WindowRc);
        window.hide();
    }

    /// Returns the window scale factor.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_windowrc_get_scale_factor(
        handle: *const WindowRcOpaque,
    ) -> f32 {
        assert_eq!(core::mem::size_of::<WindowRc>(), core::mem::size_of::<WindowRcOpaque>());
        let window = &*(handle as *const WindowRc);
        window.scale_factor()
    }

    /// Sets the window scale factor, merely for testing purposes.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_windowrc_set_scale_factor(
        handle: *const WindowRcOpaque,
        value: f32,
    ) {
        let window = &*(handle as *const WindowRc);
        window.set_scale_factor(value)
    }

    /// Sets the window scale factor, merely for testing purposes.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_windowrc_free_graphics_resources<'a>(
        handle: *const WindowRcOpaque,
        items: &Slice<'a, Pin<ItemRef<'a>>>,
    ) {
        let window = &*(handle as *const WindowRc);
        window.free_graphics_resources(items)
    }

    /// Sets the focus item.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_windowrc_set_focus_item(
        handle: *const WindowRcOpaque,
        focus_item: &ItemRc,
    ) {
        let window = &*(handle as *const WindowRc);
        window.clone().set_focus_item(focus_item)
    }

    /// Associates the window with the given component.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_windowrc_set_component(
        handle: *const WindowRcOpaque,
        component: &ComponentRc,
    ) {
        let window = &*(handle as *const WindowRc);
        window.set_component(component)
    }

    /// Show a popup.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_windowrc_show_popup(
        handle: *const WindowRcOpaque,
        popup: &ComponentRc,
        position: crate::graphics::Point,
    ) {
        let window = &*(handle as *const WindowRc);
        window.show_popup(popup, position);
    }
    /// Close the current popup
    pub unsafe extern "C" fn sixtyfps_windowrc_close_popup(handle: *const WindowRcOpaque) {
        let window = &*(handle as *const WindowRc);
        window.close_popup();
    }
}
