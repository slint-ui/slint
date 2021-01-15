/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#![warn(missing_docs)]
//! Exposed Window API

use crate::component::{ComponentRc, ComponentWeak};
use crate::graphics::Point;
use crate::input::{KeyEvent, MouseEventType, MouseInputState};
use crate::items::{ItemRc, ItemRef};
use crate::slice::Slice;
use core::cell::Cell;
use core::pin::Pin;
use std::rc::Rc;

/// This trait represents the interface that the generated code and the run-time
/// require in order to implement functionality such as device-independent pixels,
/// window resizing and other typicaly windowing system related tasks.
///
/// [`crate::graphics`] provides an implementation of this trait for use with [`crate::graphics::GraphicsBackend`].
pub trait GenericWindow {
    /// Receive a key event and pass it to the items of the component to
    /// change their state.
    ///
    /// Arguments:
    /// * `event`: The key event received by the windowing system.
    /// * `component`: The SixtyFPS compiled component that provides the tree of items.
    fn process_key_input(self: Rc<Self>, event: &KeyEvent);
    /// Spins an event loop and renders the items of the provided component in this window.
    fn run(self: Rc<Self>);
    /// Issue a request to the windowing system to re-render the contents of the window. This is typically an asynchronous
    /// request.
    fn request_redraw(&self);
    /// Returns the scale factor set on the window, as provided by the windowing system.
    fn scale_factor(&self) -> f32;
    /// Sets an overriding scale factor for the window. This is typically only used for testing.
    fn set_scale_factor(&self, factor: f32);
    /// reload the scale_factor from the window manager and sets the internal scale_factor property accordingly
    fn refresh_window_scale_factor(&self);
    /// Sets the size of the window to the specified `width`. This method is typically called in response to receiving a
    /// window resize event from the windowing system.
    fn set_width(&self, width: f32);
    /// Sets the size of the window to the specified `height`. This method is typically called in response to receiving a
    /// window resize event from the windowing system.
    fn set_height(&self, height: f32);
    /// Returns the geometry of the window
    fn get_geometry(&self) -> crate::graphics::Rect;

    /// This function is called by the generated code when a component and therefore its tree of items are destroyed. The
    /// implementation typically uses this to free the underlying graphics resources cached via [`crate::graphics::RenderingCache`].
    fn free_graphics_resources<'a>(self: Rc<Self>, items: &Slice<'a, Pin<ItemRef<'a>>>);
    /// Installs a binding on the specified property that's toggled whenever the text cursor is supposed to be visible or not.
    fn set_cursor_blink_binding(&self, prop: &crate::properties::Property<bool>);

    /// Returns the currently active keyboard notifiers.
    fn current_keyboard_modifiers(&self) -> crate::input::KeyboardModifiers;
    /// Sets the currently active keyboard notifiers. This is used only for testing or directly
    /// from the event loop implementation.
    fn set_current_keyboard_modifiers(&self, modifiers: crate::input::KeyboardModifiers);

    /// Sets the focus to the item pointed to by item_ptr. This will remove the focus from any
    /// currently focused item.
    fn set_focus_item(self: Rc<Self>, focus_item: &ItemRc);
    /// Sets the focus on the window to true or false, depending on the have_focus argument.
    /// This results in WindowFocusReceived and WindowFocusLost events.
    fn set_focus(self: Rc<Self>, have_focus: bool);

    /// Show a popup at the given position
    fn show_popup(&self, popup: &ComponentRc, position: Point);
    /// Close the active popup if any
    fn close_popup(&self);

    /// Return a font metrics trait object for the given font request. This is typically provided by the backend and
    /// requested by text related items in order to measure text metrics with the item's chosen font.
    /// Note that if the FontRequest's pixel_size is 0, it is interpreted as the undefined size and that the
    /// system default font size should be used for the returned font.
    /// With some backends this may return none unless the window is mapped.
    fn font_metrics(
        &self,
        request: crate::graphics::FontRequest,
    ) -> Option<Box<dyn crate::graphics::FontMetrics>>;
}

/// Structure that represent a Window in the runtime
pub struct Window {
    /// FIXME! use Box instead;
    platform_window: Rc<dyn GenericWindow>,
    component: std::cell::RefCell<ComponentWeak>,
    mouse_input_state: Cell<MouseInputState>,
}

impl Window {
    /// Create a new instance of the window, given the platform_window
    pub fn new(platform_window: Rc<dyn GenericWindow>) -> Self {
        Self {
            platform_window,
            component: Default::default(),
            mouse_input_state: Default::default(),
        }
    }

    /// Associates this window with the specified component. Further event handling and rendering, etc. will be
    /// done with that component.
    pub fn set_component(&self, component: &ComponentRc) {
        self.component.replace(ComponentRc::downgrade(component));
    }

    /// return the component.
    /// Panics if it wasn't set.
    pub fn component(&self) -> ComponentRc {
        self.component.borrow().upgrade().unwrap()
    }

    /// Receive a mouse event and pass it to the items of the component to
    /// change their state.
    ///
    /// Arguments:
    /// * `pos`: The position of the mouse event in window physical coordinates.
    /// * `what`: The type of mouse event.
    /// * `component`: The SixtyFPS compiled component that provides the tree of items.
    pub fn process_mouse_input(self: Rc<Self>, pos: Point, what: MouseEventType) {
        crate::animations::update_animations();
        let component = self.component.borrow().upgrade().unwrap();
        self.mouse_input_state.set(crate::input::process_mouse_input(
            component,
            crate::input::MouseEvent { pos, what },
            &ComponentWindow::new(self.clone()),
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
        self.platform_window.clone().process_key_input(event)
    }

    /// Installs a binding on the specified property that's toggled whenever the text cursor is supposed to be visible or not.
    pub fn set_cursor_blink_binding(&self, prop: &crate::properties::Property<bool>) {
        self.platform_window.clone().set_cursor_blink_binding(prop)
    }

    /// Sets the focus to the item pointed to by item_ptr. This will remove the focus from any
    /// currently focused item.
    pub fn set_focus_item(self: Rc<Self>, focus_item: &ItemRc) {
        self.platform_window.clone().set_focus_item(focus_item)
    }
    /// Sets the focus on the window to true or false, depending on the have_focus argument.
    /// This results in WindowFocusReceived and WindowFocusLost events.
    pub fn set_focus(self: Rc<Self>, have_focus: bool) {
        self.platform_window.clone().set_focus(have_focus)
    }
}

impl core::ops::Deref for Window {
    type Target = dyn GenericWindow;

    fn deref(&self) -> &Self::Target {
        &*self.platform_window
    }
}

/// The ComponentWindow is the (rust) facing public type that can render the items
/// of components to the screen.
#[repr(C)]
#[derive(Clone)]
pub struct ComponentWindow(pub std::rc::Rc<Window>);

impl ComponentWindow {
    /// Creates a new instance of a CompomentWindow based on the given window implementation. Only used
    /// internally.
    pub fn new(window_impl: std::rc::Rc<Window>) -> Self {
        Self(window_impl)
    }
    /// Spins an event loop and renders the items of the provided component in this window.
    pub fn run(&self) {
        self.0.platform_window.clone().run();
    }

    /// Returns the scale factor set on the window.
    pub fn scale_factor(&self) -> f32 {
        self.0.scale_factor()
    }

    /// Sets an overriding scale factor for the window. This is typically only used for testing.
    pub fn set_scale_factor(&self, factor: f32) {
        self.0.set_scale_factor(factor)
    }

    /// This function is called by the generated code when a component and therefore its tree of items are destroyed. The
    /// implementation typically uses this to free the underlying graphics resources cached via [RenderingCache][`crate::graphics::RenderingCache`].
    pub fn free_graphics_resources<'a>(&self, items: &Slice<'a, Pin<ItemRef<'a>>>) {
        self.0.platform_window.clone().free_graphics_resources(items);
    }

    /// Installs a binding on the specified property that's toggled whenever the text cursor is supposed to be visible or not.
    pub(crate) fn set_cursor_blink_binding(&self, prop: &crate::properties::Property<bool>) {
        self.0.platform_window.clone().set_cursor_blink_binding(prop)
    }

    /// Sets the currently active keyboard notifiers. This is used only for testing or directly
    /// from the event loop implementation.
    pub(crate) fn set_current_keyboard_modifiers(
        &self,
        modifiers: crate::input::KeyboardModifiers,
    ) {
        self.0.clone().set_current_keyboard_modifiers(modifiers)
    }

    /// Returns the currently active keyboard notifiers.
    pub(crate) fn current_keyboard_modifiers(&self) -> crate::input::KeyboardModifiers {
        self.0.platform_window.clone().current_keyboard_modifiers()
    }

    pub(crate) fn process_key_input(&self, event: &KeyEvent) {
        self.0.platform_window.clone().process_key_input(event)
    }

    /// Clears the focus on any previously focused item and makes the provided
    /// item the focus item, in order to receive future key events.
    pub fn set_focus_item(&self, focus_item: &ItemRc) {
        self.0.platform_window.clone().set_focus_item(focus_item)
    }

    /// Associates this window with the specified component, for future event handling, etc.
    pub fn set_component(&self, component: &ComponentRc) {
        self.0.set_component(component)
    }

    /// Show a popup at the given position
    pub fn show_popup(&self, popup: &ComponentRc, position: Point) {
        self.0.platform_window.clone().show_popup(popup, position)
    }
    /// Close the active popup if any
    pub fn close_popup(&self) {
        self.0.platform_window.clone().close_popup()
    }
}

/// This module contains the functions needed to interface with the event loop and window traits
/// from outside the Rust language.
pub mod ffi {
    #![allow(unsafe_code)]

    use super::*;

    #[allow(non_camel_case_types)]
    type c_void = ();

    /// Same layout as ComponentWindow
    #[repr(C)]
    pub struct ComponentWindowOpaque(*const c_void);

    /// Releases the reference to the component window held by handle.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_drop(handle: *mut ComponentWindowOpaque) {
        assert_eq!(
            core::mem::size_of::<ComponentWindow>(),
            core::mem::size_of::<ComponentWindowOpaque>()
        );
        core::ptr::read(handle as *mut ComponentWindow);
    }

    /// Releases the reference to the component window held by handle.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_clone(
        source: *const ComponentWindowOpaque,
        target: *mut ComponentWindowOpaque,
    ) {
        assert_eq!(
            core::mem::size_of::<ComponentWindow>(),
            core::mem::size_of::<ComponentWindowOpaque>()
        );
        let window = &*(source as *const ComponentWindow);
        core::ptr::write(target as *mut ComponentWindow, window.clone());
    }

    /// Spins an event loop and renders the items of the provided component in this window.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_run(handle: *const ComponentWindowOpaque) {
        let window = &*(handle as *const ComponentWindow);
        window.run();
    }

    /// Returns the window scale factor.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_get_scale_factor(
        handle: *const ComponentWindowOpaque,
    ) -> f32 {
        assert_eq!(
            core::mem::size_of::<ComponentWindow>(),
            core::mem::size_of::<ComponentWindowOpaque>()
        );
        let window = &*(handle as *const ComponentWindow);
        window.scale_factor()
    }

    /// Sets the window scale factor, merely for testing purposes.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_set_scale_factor(
        handle: *const ComponentWindowOpaque,
        value: f32,
    ) {
        let window = &*(handle as *const ComponentWindow);
        window.set_scale_factor(value)
    }

    /// Sets the window scale factor, merely for testing purposes.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_free_graphics_resources<'a>(
        handle: *const ComponentWindowOpaque,
        items: &Slice<'a, Pin<ItemRef<'a>>>,
    ) {
        let window = &*(handle as *const ComponentWindow);
        window.free_graphics_resources(items)
    }

    /// Sets the focus item.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_set_focus_item(
        handle: *const ComponentWindowOpaque,
        focus_item: &ItemRc,
    ) {
        let window = &*(handle as *const ComponentWindow);
        window.set_focus_item(focus_item)
    }

    /// Associates the window with the given component.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_set_component(
        handle: *const ComponentWindowOpaque,
        component: &ComponentRc,
    ) {
        let window = &*(handle as *const ComponentWindow);
        window.set_component(component)
    }

    /// Show a popup.
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_component_window_show_popup(
        handle: *const ComponentWindowOpaque,
        popup: &ComponentRc,
        position: crate::graphics::Point,
    ) {
        let window = &*(handle as *const ComponentWindow);
        window.show_popup(popup, position);
    }
    /// Close the current popup
    pub unsafe extern "C" fn sixtyfps_component_window_close_popup(
        handle: *const ComponentWindowOpaque,
    ) {
        let window = &*(handle as *const ComponentWindow);
        window.close_popup();
    }
}
