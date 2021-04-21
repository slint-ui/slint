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
use std::rc::{Rc, Weak};

use const_field_offset::FieldOffsets;
use corelib::component::ComponentRc;
use corelib::graphics::*;
use corelib::input::{KeyboardModifiers, MouseEvent, MouseEventType};
use corelib::items::ItemRef;
use corelib::slice::Slice;
use corelib::window::{ComponentWindow, PlatformWindow};
use corelib::Property;
use corelib::SharedString;
use sixtyfps_corelib as corelib;

/// FIXME! this is some remains from a time where the GLRenderer was called the backend
type Backend = super::GLRenderer;

type WindowFactoryFn =
    dyn Fn(&dyn crate::eventloop::EventLoopInterface, winit::window::WindowBuilder) -> Backend;

/// GraphicsWindow is an implementation of the [PlatformWindow][`crate::eventloop::PlatformWindow`] trait. This is
/// typically instantiated by entry factory functions of the different graphics backends.
pub struct GraphicsWindow {
    pub(crate) self_weak: Weak<corelib::window::Window>,
    window_factory: Box<WindowFactoryFn>,
    map_state: RefCell<GraphicsWindowBackendState>,
    properties: Pin<Box<WindowProperties>>,
    keyboard_modifiers: std::cell::Cell<KeyboardModifiers>,

    mouse_input_state: std::cell::Cell<corelib::input::MouseInputState>,
    /// Current popup's component and position
    /// FIXME: the popup should actually be another window, not just some overlay
    active_popup: std::cell::RefCell<Option<(ComponentRc, Point)>>,

    default_font_properties: Pin<Rc<Property<FontRequest>>>,
}

impl GraphicsWindow {
    /// Creates a new reference-counted instance.
    ///
    /// Arguments:
    /// * `graphics_backend_factory`: The factor function stored in the GraphicsWindow that's called when the state
    ///   of the window changes to mapped. The event loop and window builder parameters can be used to create a
    ///   backing window.
    pub(crate) fn new(
        window_weak: &Weak<corelib::window::Window>,
        graphics_backend_factory: impl Fn(&dyn crate::eventloop::EventLoopInterface, winit::window::WindowBuilder) -> Backend
            + 'static,
    ) -> Rc<Self> {
        let default_font_properties_prop = Rc::pin(Property::default());
        default_font_properties_prop.set_binding({
            let self_weak = window_weak.clone();
            move || {
                self_weak
                    .upgrade()
                    .unwrap()
                    .try_component()
                    .and_then(|component_rc| {
                        let component = ComponentRc::borrow_pin(&component_rc);
                        let root_item = component.as_ref().get_item_ref(0);
                        ItemRef::downcast_pin(root_item).map(
                            |window_item: Pin<&corelib::items::Window>| {
                                window_item.default_font_properties()
                            },
                        )
                    })
                    .unwrap_or_default()
            }
        });

        Rc::new(Self {
            self_weak: window_weak.clone(),
            window_factory: Box::new(graphics_backend_factory),
            map_state: RefCell::new(GraphicsWindowBackendState::Unmapped),
            properties: Box::pin(WindowProperties::default()),
            keyboard_modifiers: Default::default(),
            mouse_input_state: Default::default(),
            active_popup: Default::default(),
            default_font_properties: default_font_properties_prop,
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

                    #[cfg(target_arch = "wasm32")]
                    {
                        // set_max_inner_size / set_min_inner_size don't work on wasm, so apply the size manually
                        let existing_size = window.backend.borrow().window().inner_size();
                        if !(min_width..=max_width).contains(&(existing_size.width as f32))
                            || !(min_height..=max_height).contains(&(existing_size.height as f32))
                        {
                            let new_size = winit::dpi::PhysicalSize::new(
                                existing_size
                                    .width
                                    .min(max_width.ceil() as u32)
                                    .max(min_width.ceil() as u32),
                                existing_size
                                    .height
                                    .min(max_height.ceil() as u32)
                                    .max(min_height.ceil() as u32),
                            );
                            window.backend.borrow().window().set_inner_size(new_size);
                        }
                    }
                }
            }
        }
    }

    /// Requests for the window to be mapped to the screen.
    ///
    /// Arguments:
    /// * `component`: The component that holds the root item of the scene. If the item is a [`corelib::items::Window`], then
    ///   the `width` and `height` properties are read and the values are passed to the windowing system as request
    ///   for the initial size of the window. Then bindings are installed on these properties to keep them up-to-date
    ///   with the size as it may be changed by the user or the windowing system in general.
    fn map_window(self: Rc<Self>) {
        if matches!(&*self.map_state.borrow(), GraphicsWindowBackendState::Mapped(..)) {
            return;
        }

        let component = self.component();
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
            let backend = crate::eventloop::with_window_target(|event_loop| {
                self.window_factory.as_ref()(event_loop, window_builder)
            });

            // Ideally we should be passing the initial requested size to the window builder, but those properties
            // may be specified in logical pixels, relative to the scale factory, which we only know *after* mapping
            // the window to the screen. So we first map the window then, propagate the scale factory and *then* the
            // width/height properties should have the correct values calculated via their bindings that multiply with
            // the scale factor.
            // We could pass the logical requested size at window builder time, *if* we knew what the values are.
            let window_id = {
                let platform_window = backend.window();

                if std::env::var("SIXTYFPS_FULLSCREEN").is_ok() {
                    platform_window
                        .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                }

                self.properties.as_ref().scale_factor.set(platform_window.scale_factor() as _);

                platform_window.id()
            };

            self.map_state.replace(GraphicsWindowBackendState::Mapped(MappedWindow {
                backend: RefCell::new(backend),
                constraints: Default::default(),
            }));

            window_id
        };

        crate::eventloop::register_window(id, self.clone());
    }
    /// Removes the window from the screen. The window is not destroyed though, it can be show (mapped) again later
    /// by calling [`PlatformWindow::map_window`].
    fn unmap_window(self: Rc<Self>) {
        self.map_state.replace(GraphicsWindowBackendState::Unmapped);
        /* FIXME:
        if let Some(existing_blinker) = self.cursor_blinker.borrow().upgrade() {
            existing_blinker.stop();
        }*/
    }

    fn component(&self) -> ComponentRc {
        self.self_weak.upgrade().unwrap().component()
    }

    fn default_font_properties(&self) -> &Pin<Rc<Property<FontRequest>>> {
        &self.default_font_properties
    }
}

impl GraphicsWindow {
    /// Draw the items of the specified `component` in the given window.
    pub fn draw(self: Rc<Self>) {
        let runtime_window = self.self_weak.upgrade().unwrap();
        runtime_window.clone().draw_tracked(|| {
            let component_rc = self.component();
            let component = ComponentRc::borrow_pin(&component_rc);

            {
                if runtime_window.meta_properties_tracker.as_ref().is_dirty() {
                    runtime_window.meta_properties_tracker.as_ref().evaluate(|| {
                        self.apply_geometry_constraint(component.as_ref().layout_info());
                        component.as_ref().apply_layout(Default::default());

                        if let Some((popup, pos)) = &*self.active_popup.borrow() {
                            let popup = ComponentRc::borrow_pin(popup);
                            let popup_root = popup.as_ref().get_item_ref(0);
                            let size = if let Some(window_item) = ItemRef::downcast_pin(popup_root)
                            {
                                let layout_info = popup.as_ref().layout_info();

                                let width = corelib::items::Window::FIELD_OFFSETS
                                    .width
                                    .apply_pin(window_item);
                                let mut w = width.get();
                                if w < layout_info.min_width {
                                    w = layout_info.min_width;
                                    width.set(w);
                                }

                                let height = corelib::items::Window::FIELD_OFFSETS
                                    .height
                                    .apply_pin(window_item);
                                let mut h = height.get();
                                if h < layout_info.min_height {
                                    h = layout_info.min_height;
                                    height.set(h);
                                }
                                Size::new(h, w) * self.scale_factor()
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
            let background_color = if let Some(window_item) =
                ItemRef::downcast_pin::<corelib::items::Window>(root_item)
            {
                window_item.background()
            } else {
                RgbaColor { red: 255 as u8, green: 255, blue: 255, alpha: 255 }.into()
            };

            let mut renderer = window.backend.borrow_mut().new_renderer(
                &background_color,
                self.scale_factor(),
                self.default_font_properties(),
            );
            corelib::item_rendering::render_component_items(
                &component_rc,
                &mut renderer,
                Point::default(),
            );
            if let Some(popup) = &*self.active_popup.borrow() {
                corelib::item_rendering::render_component_items(&popup.0, &mut renderer, popup.1);
            }
            window.backend.borrow_mut().flush_renderer(renderer);
        })
    }

    /// FIXME: this is the same as Window::process_mouse_input, but this handle the popup.
    /// Ideally the popup should be handled as a different window or by theevent loop, and
    /// this function can go away
    pub fn process_mouse_input(self: Rc<Self>, mut pos: Point, what: MouseEventType) {
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
            self.component()
        };

        self.mouse_input_state.set(corelib::input::process_mouse_input(
            component,
            MouseEvent { pos, what },
            &ComponentWindow::new(self.self_weak.upgrade().unwrap()),
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

    /// Returns the currently active keyboard notifiers.
    pub fn current_keyboard_modifiers(&self) -> KeyboardModifiers {
        self.keyboard_modifiers.get()
    }
    /// Sets the currently active keyboard notifiers. This is used only for testing or directly
    /// from the event loop implementation.
    pub fn set_current_keyboard_modifiers(&self, state: KeyboardModifiers) {
        self.keyboard_modifiers.set(state)
    }

    /// reload the scale_factor from the window manager and sets the internal scale_factor property accordingly
    pub fn refresh_window_scale_factor(&self) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(window) => {
                let sf = window.backend.borrow().window().scale_factor();
                self.set_scale_factor(sf as f32)
            }
        }
    }

    /// Sets the size of the window. This method is typically called in response to receiving a
    /// window resize event from the windowing system.
    /// Size is in logical pixels.
    pub fn set_geometry(&self, width: f32, height: f32) {
        self.self_weak.upgrade().unwrap().try_component().map(|component_rc| {
            let component = ComponentRc::borrow_pin(&component_rc);
            let root_item = component.as_ref().get_item_ref(0);
            if let Some(window_item) = ItemRef::downcast_pin::<corelib::items::Window>(root_item) {
                window_item.width.set(width);
                window_item.height.set(height);
            }
        });
    }
}

impl PlatformWindow for GraphicsWindow {
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

    fn free_graphics_resources<'a>(self: Rc<Self>, items: &Slice<'a, Pin<ItemRef<'a>>>) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(window) => {
                for item in items.iter() {
                    let cached_rendering_data = item.cached_rendering_data_offset();
                    cached_rendering_data.release(
                        &mut window.backend.borrow().shared_data.item_graphics_cache.borrow_mut(),
                    )
                }
            }
        }
    }

    fn show_popup(&self, popup: &ComponentRc, position: Point) {
        self.self_weak.upgrade().map(|window| window.meta_properties_tracker.set_dirty());
        *self.active_popup.borrow_mut() = Some((popup.clone(), position));
    }

    fn close_popup(&self) {
        *self.active_popup.borrow_mut() = None;
    }

    fn request_window_properties_update(&self) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {
                // Nothing to be done if the window isn't visible. When it becomes visible,
                // ComponentWindow::show() calls update_window_properties()
            }
            GraphicsWindowBackendState::Mapped(window) => {
                let backend = window.backend.borrow();
                let window_id = backend.window().id();
                crate::eventloop::with_window_target(|event_loop| {
                    event_loop.event_loop_proxy().send_event(
                        crate::eventloop::CustomEvent::UpdateWindowProperties(window_id),
                    )
                })
                .ok();
            }
        }
    }

    fn apply_window_properties(&self, window_item: Pin<&sixtyfps_corelib::items::Window>) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(window) => {
                let backend = window.backend.borrow();
                let winit_window = backend.window();
                winit_window.set_title(&window_item.title());
                let mut size = winit_window.inner_size().to_logical(self.scale_factor() as f64);
                let mut must_resize = false;
                let mut apply = |r: &mut u32, v| {
                    if v > 0. {
                        *r = v as _
                    } else {
                        must_resize = true
                    }
                };
                apply(&mut size.width, window_item.width());
                apply(&mut size.height, window_item.height());
                winit_window.set_inner_size(size);
                if must_resize {
                    self.set_geometry(size.width as _, size.height as _)
                }
            }
        }
    }

    fn show(self: Rc<Self>) {
        self.map_window();
    }

    fn hide(self: Rc<Self>) {
        self.unmap_window();
    }

    fn font_metrics(
        &self,
        item_graphics_cache: &corelib::item_rendering::CachedRenderingData,
        unresolved_font_request_getter: &dyn Fn() -> corelib::graphics::FontRequest,
        reference_text: Pin<&Property<SharedString>>,
    ) -> Option<Box<dyn corelib::graphics::FontMetrics>> {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => None,
            GraphicsWindowBackendState::Mapped(window) => Some(
                window.backend.borrow_mut().font_metrics(
                    item_graphics_cache,
                    &|| {
                        unresolved_font_request_getter()
                            .merge(&self.default_font_properties().as_ref().get())
                    },
                    WindowProperties::FIELD_OFFSETS
                        .scale_factor
                        .apply_pin(self.properties.as_ref()),
                    reference_text,
                ),
            ),
        }
    }

    fn image_size(
        &self,
        source: Pin<&sixtyfps_corelib::properties::Property<ImageReference>>,
    ) -> sixtyfps_corelib::graphics::Size {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => Default::default(),
            GraphicsWindowBackendState::Mapped(window) => {
                window.backend.borrow().image_size(source)
            }
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct MappedWindow {
    backend: RefCell<Backend>,
    constraints: Cell<corelib::layout::LayoutInfo>,
}

impl Drop for MappedWindow {
    fn drop(&mut self) {
        crate::eventloop::unregister_window(self.backend.borrow().window().id());
    }
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
}

impl Default for WindowProperties {
    fn default() -> Self {
        Self { scale_factor: Property::new(1.0) }
    }
}
