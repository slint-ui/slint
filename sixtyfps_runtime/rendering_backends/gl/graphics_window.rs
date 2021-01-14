/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! This module contains the GraphicsWindow that used to be within corelib.

use core::cell::{Cell, RefCell};
use core::pin::Pin;
use std::rc::Rc;

use const_field_offset::FieldOffsets;
use corelib::component::{ComponentRc, ComponentWeak};
use corelib::graphics::*;
use corelib::input::{KeyEvent, KeyboardModifiers, MouseEvent, MouseEventType, TextCursorBlinker};
use corelib::items::{ItemRc, ItemRef, ItemWeak};
use corelib::properties::PropertyTracker;
use corelib::slice::Slice;
use corelib::window::{ComponentWindow, GenericWindow};
use corelib::Property;
use sixtyfps_corelib as corelib;

/// FIXME! this is some remains from a time where the GLRenderer was called the backend
type Backend = super::GLRenderer;

type WindowFactoryFn =
    dyn Fn(&crate::eventloop::EventLoop, winit::window::WindowBuilder) -> Backend;

/// GraphicsWindow is an implementation of the [GenericWindow][`crate::eventloop::GenericWindow`] trait. This is
/// typically instantiated by entry factory functions of the different graphics backends.
pub struct GraphicsWindow {
    window_factory: Box<WindowFactoryFn>,
    map_state: RefCell<GraphicsWindowBackendState>,
    properties: Pin<Box<WindowProperties>>,
    cursor_blinker: RefCell<pin_weak::rc::PinWeak<TextCursorBlinker>>,
    keyboard_modifiers: std::cell::Cell<KeyboardModifiers>,
    component: std::cell::RefCell<ComponentWeak>,
    /// Gets dirty when the layout restrictions, or some other property of the windows change
    meta_property_listener: Pin<Rc<PropertyTracker>>,
    focus_item: std::cell::RefCell<ItemWeak>,
    mouse_input_state: std::cell::Cell<corelib::input::MouseInputState>,
    /// Current popup's component and position
    /// FIXME: the popup should actually be another window, not just some overlay
    active_popup: std::cell::RefCell<Option<(ComponentRc, Point)>>,
}

impl GraphicsWindow {
    /// Creates a new reference-counted instance.
    ///
    /// Arguments:
    /// * `graphics_backend_factory`: The factor function stored in the GraphicsWindow that's called when the state
    ///   of the window changes to mapped. The event loop and window builder parameters can be used to create a
    ///   backing window.
    pub fn new(
        graphics_backend_factory: impl Fn(&crate::eventloop::EventLoop, winit::window::WindowBuilder) -> Backend
            + 'static,
    ) -> Rc<Self> {
        Rc::new(Self {
            window_factory: Box::new(graphics_backend_factory),
            map_state: RefCell::new(GraphicsWindowBackendState::Unmapped),
            properties: Box::pin(WindowProperties::default()),
            cursor_blinker: Default::default(),
            keyboard_modifiers: Default::default(),
            component: Default::default(),
            meta_property_listener: Rc::pin(Default::default()),
            focus_item: Default::default(),
            mouse_input_state: Default::default(),
            active_popup: Default::default(),
        })
    }

    fn apply_geometry_constraint(&self, constraints: corelib::layout::LayoutInfo) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(window) => {
                if constraints != window.constraints.get() {
                    let min_width = constraints.min_width.min(constraints.max_width);
                    let min_height = constraints.min_height.min(constraints.max_height);
                    let max_width = constraints.max_width.max(constraints.min_width);
                    let max_height = constraints.max_height.max(constraints.min_height);

                    window.backend.borrow().window().set_min_inner_size(
                        if min_width > 0. || min_height > 0. {
                            Some(winit::dpi::PhysicalSize::new(min_width, min_height))
                        } else {
                            None
                        },
                    );
                    window.backend.borrow().window().set_max_inner_size(
                        if max_width < f32::MAX || max_height < f32::MAX {
                            Some(winit::dpi::PhysicalSize::new(
                                max_width.min(65535.),
                                max_height.min(65535.),
                            ))
                        } else {
                            None
                        },
                    );
                    window.constraints.set(constraints);
                }
            }
        }
    }
    fn apply_window_properties(&self, window_item: Pin<&corelib::items::Window>) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(window) => {
                let backend = window.backend.borrow();
                backend.window().set_title(window_item.title().as_str());
            }
        }
    }

    /// Requests for the window to be mapped to the screen.
    ///
    /// Arguments:
    /// * `event_loop`: The event loop used to drive further event handling for this window
    ///   as it will receive events.
    /// * `component`: The component that holds the root item of the scene. If the item is a [`corelib::items::Window`], then
    ///   the `width` and `height` properties are read and the values are passed to the windowing system as request
    ///   for the initial size of the window. Then bindings are installed on these properties to keep them up-to-date
    ///   with the size as it may be changed by the user or the windowing system in general.
    fn map_window(self: Rc<Self>, event_loop: &crate::eventloop::EventLoop) {
        if matches!(&*self.map_state.borrow(), GraphicsWindowBackendState::Mapped(..)) {
            return;
        }

        let component = self.component.borrow().upgrade().unwrap();
        let component = ComponentRc::borrow_pin(&component);
        let root_item = component.as_ref().get_item_ref(0);

        let window_title =
            if let Some(window_item) = ItemRef::downcast_pin::<corelib::items::Window>(root_item) {
                window_item.title().to_string()
            } else {
                "SixtyFPS Window".to_string()
            };
        let window_builder = winit::window::WindowBuilder::new().with_title(window_title);

        let id = {
            let backend = self.window_factory.as_ref()(&event_loop, window_builder);

            let platform_window = backend.window();

            if std::env::var("SIXTYFPS_FULLSCREEN").is_ok() {
                platform_window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
            }

            let window_id = platform_window.id();

            // Ideally we should be passing the initial requested size to the window builder, but those properties
            // may be specified in logical pixels, relative to the scale factory, which we only know *after* mapping
            // the window to the screen. So we first map the window then, propagate the scale factory and *then* the
            // width/height properties should have the correct values calculated via their bindings that multiply with
            // the scale factor.
            // We could pass the logical requested size at window builder time, *if* we knew what the values are.
            {
                self.properties.as_ref().scale_factor.set(platform_window.scale_factor() as _);
                let existing_size = platform_window.inner_size();

                let mut new_size = existing_size;

                if let Some(window_item) =
                    ItemRef::downcast_pin::<corelib::items::Window>(root_item)
                {
                    let width = window_item.width();
                    if width > 0. {
                        new_size.width = width as _;
                    }
                    let height = window_item.height();
                    if height > 0. {
                        new_size.height = height as _;
                    }

                    {
                        let window = self.clone();
                        window_item.as_ref().width.set_binding(move || {
                            WindowProperties::FIELD_OFFSETS
                                .width
                                .apply_pin(window.properties.as_ref())
                                .get()
                        });
                    }
                    {
                        let window = self.clone();
                        window_item.as_ref().height.set_binding(move || {
                            WindowProperties::FIELD_OFFSETS
                                .height
                                .apply_pin(window.properties.as_ref())
                                .get()
                        });
                    }
                }

                if new_size != existing_size {
                    platform_window.set_inner_size(new_size)
                }

                self.properties.as_ref().width.set(new_size.width as _);
                self.properties.as_ref().height.set(new_size.height as _);
            }

            self.map_state.replace(GraphicsWindowBackendState::Mapped(MappedWindow {
                backend: RefCell::new(backend),
                constraints: Default::default(),
            }));

            window_id
        };

        crate::eventloop::register_window(id, self.clone() as Rc<dyn GenericWindow>);
    }
    /// Removes the window from the screen. The window is not destroyed though, it can be show (mapped) again later
    /// by calling [`GenericWindow::map_window`].
    fn unmap_window(self: Rc<Self>) {
        self.map_state.replace(GraphicsWindowBackendState::Unmapped);
        if let Some(existing_blinker) = self.cursor_blinker.borrow().upgrade() {
            existing_blinker.stop();
        }
    }
}

impl Drop for GraphicsWindow {
    fn drop(&mut self) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(mw) => {
                crate::eventloop::unregister_window(mw.backend.borrow().window().id());
            }
        }
        if let Some(existing_blinker) = self.cursor_blinker.borrow().upgrade() {
            existing_blinker.stop();
        }
    }
}

impl GenericWindow for GraphicsWindow {
    fn set_component(self: Rc<Self>, component: &ComponentRc) {
        *self.component.borrow_mut() = vtable::VRc::downgrade(&component)
    }

    fn draw(self: Rc<Self>) {
        let component_rc = self.component.borrow().upgrade().unwrap();
        let component = ComponentRc::borrow_pin(&component_rc);

        {
            if self.meta_property_listener.as_ref().is_dirty() {
                self.meta_property_listener.as_ref().evaluate(|| {
                    self.apply_geometry_constraint(component.as_ref().layout_info());
                    component.as_ref().apply_layout(self.get_geometry());

                    let root_item = component.as_ref().get_item_ref(0);
                    if let Some(window_item) = ItemRef::downcast_pin(root_item) {
                        self.apply_window_properties(window_item);
                    }

                    if let Some((popup, pos)) = &*self.active_popup.borrow() {
                        let popup = ComponentRc::borrow_pin(popup);
                        let popup_root = popup.as_ref().get_item_ref(0);
                        let size = if let Some(window_item) = ItemRef::downcast_pin(popup_root) {
                            let layout_info = popup.as_ref().layout_info();

                            let width =
                                corelib::items::Window::FIELD_OFFSETS.width.apply_pin(window_item);
                            let mut w = width.get();
                            if w < layout_info.min_width {
                                w = layout_info.min_width;
                                width.set(w);
                            }

                            let height =
                                corelib::items::Window::FIELD_OFFSETS.height.apply_pin(window_item);
                            let mut h = height.get();
                            if h < layout_info.min_height {
                                h = layout_info.min_height;
                                height.set(h);
                            }
                            Size::new(h, w)
                        } else {
                            Size::default()
                        };
                        popup.as_ref().apply_layout(Rect::new(pos.clone(), size));
                    }
                })
            }
        }

        let map_state = self.map_state.borrow();
        let window = map_state.as_mapped();
        let root_item = component.as_ref().get_item_ref(0);
        let background_color =
            if let Some(window_item) = ItemRef::downcast_pin::<corelib::items::Window>(root_item) {
                window_item.color()
            } else {
                RgbaColor { red: 255 as u8, green: 255, blue: 255, alpha: 255 }.into()
            };

        let mut renderer = window.backend.borrow_mut().new_renderer(&background_color);
        corelib::item_rendering::render_component_items(
            &component_rc,
            &mut renderer,
            Point::default(),
        );
        if let Some(popup) = &*self.active_popup.borrow() {
            corelib::item_rendering::render_component_items(&popup.0, &mut renderer, popup.1);
        }
        window.backend.borrow_mut().flush_renderer(renderer);
    }

    fn process_mouse_input(self: Rc<Self>, mut pos: Point, what: MouseEventType) {
        let active_popup = (*self.active_popup.borrow()).clone();
        let component = if let Some(popup) = &active_popup {
            pos -= popup.1.to_vector();
            if what == MouseEventType::MousePressed {
                // close the popup if one press outside the popup
                let geom =
                    ComponentRc::borrow_pin(&popup.0).as_ref().get_item_ref(0).as_ref().geometry();
                if !geom.contains(pos) {
                    self.close_popup();
                    return;
                }
            }
            popup.0.clone()
        } else {
            self.component.borrow().upgrade().unwrap()
        };

        self.mouse_input_state.set(corelib::input::process_mouse_input(
            component,
            MouseEvent { pos, what },
            &ComponentWindow::new(self.clone()),
            self.mouse_input_state.take(),
        ));

        if active_popup.is_some() {
            //FIXME: currently the ComboBox is the only thing that uses the popup, and it should close automatically
            // on release.  But ideally, there would be API to close the popup rather than always closing it on release
            if what == MouseEventType::MouseReleased {
                self.close_popup();
            }
        }
    }

    fn process_key_input(self: Rc<Self>, event: &KeyEvent) {
        if let Some(focus_item) = self.as_ref().focus_item.borrow().upgrade() {
            let window = &ComponentWindow::new(self.clone());
            focus_item.borrow().as_ref().key_event(event, &window);
        }
    }

    fn request_redraw(&self) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(window) => {
                window.backend.borrow().window().request_redraw()
            }
        }
    }

    fn scale_factor(&self) -> f32 {
        WindowProperties::FIELD_OFFSETS.scale_factor.apply_pin(self.properties.as_ref()).get()
    }

    fn set_scale_factor(&self, factor: f32) {
        self.properties.as_ref().scale_factor.set(factor);
    }

    fn refresh_window_scale_factor(&self) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(window) => {
                let sf = window.backend.borrow().window().scale_factor();
                self.set_scale_factor(sf as f32)
            }
        }
    }

    fn set_width(&self, width: f32) {
        self.properties.as_ref().width.set(width);
    }

    fn set_height(&self, height: f32) {
        self.properties.as_ref().height.set(height);
    }

    fn get_geometry(&self) -> corelib::graphics::Rect {
        euclid::rect(
            0.,
            0.,
            WindowProperties::FIELD_OFFSETS.width.apply_pin(self.properties.as_ref()).get(),
            WindowProperties::FIELD_OFFSETS.height.apply_pin(self.properties.as_ref()).get(),
        )
    }

    fn free_graphics_resources<'a>(self: Rc<Self>, items: &Slice<'a, Pin<ItemRef<'a>>>) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(window) => {
                for item in items.iter() {
                    let cached_rendering_data = item.cached_rendering_data_offset();
                    cached_rendering_data
                        .release(&mut window.backend.borrow().item_rendering_cache.borrow_mut())
                }
            }
        }
    }

    fn set_cursor_blink_binding(&self, prop: &corelib::properties::Property<bool>) {
        let existing_blinker = self.cursor_blinker.borrow().clone();

        let blinker = existing_blinker.upgrade().unwrap_or_else(|| {
            let new_blinker = TextCursorBlinker::new();
            *self.cursor_blinker.borrow_mut() =
                pin_weak::rc::PinWeak::downgrade(new_blinker.clone());
            new_blinker
        });

        TextCursorBlinker::set_binding(blinker, prop);
    }

    /// Returns the currently active keyboard notifiers.
    fn current_keyboard_modifiers(&self) -> KeyboardModifiers {
        self.keyboard_modifiers.get()
    }
    /// Sets the currently active keyboard notifiers. This is used only for testing or directly
    /// from the event loop implementation.
    fn set_current_keyboard_modifiers(&self, state: KeyboardModifiers) {
        self.keyboard_modifiers.set(state)
    }

    fn set_focus_item(self: Rc<Self>, focus_item: &ItemRc) {
        let window = ComponentWindow::new(self.clone());

        if let Some(old_focus_item) = self.as_ref().focus_item.borrow().upgrade() {
            old_focus_item
                .borrow()
                .as_ref()
                .focus_event(&corelib::input::FocusEvent::FocusOut, &window);
        }

        *self.as_ref().focus_item.borrow_mut() = focus_item.downgrade();

        focus_item.borrow().as_ref().focus_event(&corelib::input::FocusEvent::FocusIn, &window);
    }

    fn set_focus(self: Rc<Self>, have_focus: bool) {
        let window = ComponentWindow::new(self.clone());
        let event = if have_focus {
            corelib::input::FocusEvent::WindowReceivedFocus
        } else {
            corelib::input::FocusEvent::WindowLostFocus
        };

        if let Some(focus_item) = self.as_ref().focus_item.borrow().upgrade() {
            focus_item.borrow().as_ref().focus_event(&event, &window);
        }
    }

    fn show_popup(&self, popup: &ComponentRc, position: Point) {
        self.meta_property_listener.set_dirty();
        *self.active_popup.borrow_mut() = Some((popup.clone(), position));
    }

    fn close_popup(&self) {
        *self.active_popup.borrow_mut() = None;
    }

    fn run(self: Rc<Self>) {
        let event_loop = crate::eventloop::EventLoop::new();

        self.clone().map_window(&event_loop);
        event_loop.run();
        self.unmap_window();
    }

    fn font_metrics(
        &self,
        request: corelib::graphics::FontRequest,
    ) -> Option<Box<dyn corelib::graphics::FontMetrics>> {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => None,
            GraphicsWindowBackendState::Mapped(window) => {
                Some(window.backend.borrow_mut().font_metrics(request))
            }
        }
    }
}

struct MappedWindow {
    backend: RefCell<Backend>,
    constraints: Cell<corelib::layout::LayoutInfo>,
}

enum GraphicsWindowBackendState {
    Unmapped,
    Mapped(MappedWindow),
}

impl GraphicsWindowBackendState {
    fn as_mapped(&self) -> &MappedWindow {
        match self {
            GraphicsWindowBackendState::Unmapped => panic!(
                "internal error: tried to access window functions that require a mapped window"
            ),
            GraphicsWindowBackendState::Mapped(mw) => &mw,
        }
    }
}

#[derive(FieldOffsets)]
#[repr(C)]
#[pin]
struct WindowProperties {
    scale_factor: Property<f32>,
    width: Property<f32>,
    height: Property<f32>,
}

impl Default for WindowProperties {
    fn default() -> Self {
        Self {
            scale_factor: Property::new(1.0),
            width: Property::new(800.),
            height: Property::new(600.),
        }
    }
}
