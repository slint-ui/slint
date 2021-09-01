/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! This module contains the GraphicsWindow that used to be within corelib.

// cspell:ignore corelib winit Borderless

use core::cell::{Cell, RefCell};
use core::pin::Pin;
use std::rc::{Rc, Weak};

use super::{GLRenderer, ImageCache, ItemGraphicsCache};
use const_field_offset::FieldOffsets;
use corelib::component::ComponentRc;
use corelib::graphics::*;
use corelib::input::{KeyboardModifiers, MouseEvent};
use corelib::items::ItemRef;
use corelib::layout::Orientation;
use corelib::slice::Slice;
use corelib::window::PlatformWindow;
use corelib::Property;
use sixtyfps_corelib as corelib;
use winit::dpi::LogicalSize;

use crate::ItemGraphicsCacheEntry;

/// GraphicsWindow is an implementation of the [PlatformWindow][`crate::eventloop::PlatformWindow`] trait. This is
/// typically instantiated by entry factory functions of the different graphics back ends.
pub struct GraphicsWindow {
    pub(crate) self_weak: Weak<corelib::window::Window>,
    map_state: RefCell<GraphicsWindowBackendState>,
    keyboard_modifiers: std::cell::Cell<KeyboardModifiers>,
    pub(crate) currently_pressed_key_code: std::cell::Cell<Option<winit::event::VirtualKeyCode>>,

    mouse_input_state: std::cell::Cell<corelib::input::MouseInputState>,
    /// Current popup's component and position
    /// FIXME: the popup should actually be another window, not just some overlay
    active_popup: std::cell::RefCell<Option<(ComponentRc, Point)>>,

    pub(crate) graphics_cache: RefCell<ItemGraphicsCache>,
    // This cache only contains textures. The cache for decoded CPU side images is in crate::IMAGE_CACHE.
    pub(crate) texture_cache: RefCell<ImageCache>,

    #[cfg(target_arch = "wasm32")]
    canvas_id: String,
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
        #[cfg(target_arch = "wasm32")] canvas_id: String,
    ) -> Rc<Self> {
        Rc::new(Self {
            self_weak: window_weak.clone(),
            map_state: RefCell::new(GraphicsWindowBackendState::Unmapped),
            keyboard_modifiers: Default::default(),
            currently_pressed_key_code: Default::default(),
            mouse_input_state: Default::default(),
            active_popup: Default::default(),
            graphics_cache: Default::default(),
            texture_cache: Default::default(),
            #[cfg(target_arch = "wasm32")]
            canvas_id,
        })
    }

    fn apply_geometry_constraint(
        &self,
        constraints_horizontal: corelib::layout::LayoutInfo,
        constraints_vertical: corelib::layout::LayoutInfo,
    ) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(window) => {
                if (constraints_horizontal, constraints_vertical) != window.constraints.get() {
                    let min_width = constraints_horizontal.min.min(constraints_horizontal.max);
                    let min_height = constraints_vertical.min.min(constraints_vertical.max);
                    let max_width = constraints_horizontal.max.max(constraints_horizontal.min);
                    let max_height = constraints_vertical.max.max(constraints_vertical.min);

                    window.backend.borrow().window().set_min_inner_size(
                        if min_width > 0. || min_height > 0. {
                            Some(winit::dpi::LogicalSize::new(min_width, min_height))
                        } else {
                            None
                        },
                    );
                    window.backend.borrow().window().set_max_inner_size(
                        if max_width < f32::MAX || max_height < f32::MAX {
                            Some(winit::dpi::LogicalSize::new(
                                max_width.min(65535.),
                                max_height.min(65535.),
                            ))
                        } else {
                            None
                        },
                    );
                    window.constraints.set((constraints_horizontal, constraints_vertical));

                    #[cfg(target_arch = "wasm32")]
                    {
                        // set_max_inner_size / set_min_inner_size don't work on wasm, so apply the size manually
                        let existing_size = window.backend.borrow().window().inner_size();
                        if !(min_width..=max_width).contains(&(existing_size.width as f32))
                            || !(min_height..=max_height).contains(&(existing_size.height as f32))
                        {
                            let new_size = winit::dpi::LogicalSize::new(
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

    fn with_current_context<T>(&self, cb: impl FnOnce() -> T) -> T {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => cb(),
            GraphicsWindowBackendState::Mapped(window) => {
                window.backend.borrow().with_current_context(cb)
            }
        }
    }

    /// Requests for the window to be mapped to the screen.
    ///
    /// Arguments:
    /// * `component`: The component that holds the root item of the scene. If the item is a [`corelib::items::WindowItem`], then
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

        let window_title = if let Some(window_item) =
            ItemRef::downcast_pin::<corelib::items::WindowItem>(root_item)
        {
            window_item.title().to_string()
        } else {
            "SixtyFPS Window".to_string()
        };
        let window_builder = winit::window::WindowBuilder::new().with_title(window_title);

        let window_builder = if std::env::var("SIXTYFPS_FULLSCREEN").is_ok() {
            window_builder.with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))
        } else {
            let component_rc = self.component();
            let component = ComponentRc::borrow_pin(&component_rc);
            let layout_info_h = component.as_ref().layout_info(Orientation::Horizontal);
            let layout_info_v = component.as_ref().layout_info(Orientation::Vertical);
            let s = LogicalSize::new(
                layout_info_h.preferred_bounded(),
                layout_info_v.preferred_bounded(),
            );
            if s.width > 0. && s.height > 0. {
                // Make sure that the window's inner size is in sync with the root window item's
                // width/height.
                self.set_geometry(s.width, s.height);
                window_builder.with_inner_size(s)
            } else {
                window_builder
            }
        };

        let backend = crate::GLRenderer::new(
            window_builder,
            #[cfg(target_arch = "wasm32")]
            &self.canvas_id,
        );

        let platform_window = backend.window();
        let runtime_window = self.self_weak.upgrade().unwrap();
        runtime_window.set_scale_factor(platform_window.scale_factor() as _);
        let id = platform_window.id();
        drop(platform_window);

        self.map_state.replace(GraphicsWindowBackendState::Mapped(MappedWindow {
            backend: RefCell::new(backend),
            clear_color: RgbaColor { red: 255_u8, green: 255, blue: 255, alpha: 255 }.into(),
            constraints: Default::default(),
        }));

        crate::event_loop::register_window(id, self);
    }
    /// Removes the window from the screen. The window is not destroyed though, it can be show (mapped) again later
    /// by calling [`PlatformWindow::map_window`].
    fn unmap_window(self: Rc<Self>) {
        // Release GL textures and other GPU bound resources.
        self.with_current_context(|| {
            self.graphics_cache.borrow_mut().clear();
            self.texture_cache.borrow_mut().remove_textures();
        });

        self.map_state.replace(GraphicsWindowBackendState::Unmapped);
        /* FIXME:
        if let Some(existing_blinker) = self.cursor_blinker.borrow().upgrade() {
            existing_blinker.stop();
        }*/
    }

    fn component(&self) -> ComponentRc {
        self.self_weak.upgrade().unwrap().component()
    }

    pub fn default_font_properties(&self) -> FontRequest {
        self.self_weak.upgrade().unwrap().default_font_properties()
    }
}

impl GraphicsWindow {
    /// Draw the items of the specified `component` in the given window.
    pub fn draw(self: Rc<Self>) {
        let runtime_window = self.self_weak.upgrade().unwrap();
        runtime_window.clone().draw_tracked(|| {
            let component_rc = self.component();
            let component = ComponentRc::borrow_pin(&component_rc);

            runtime_window.meta_properties_tracker.as_ref().evaluate_if_dirty(|| {
                self.apply_geometry_constraint(
                    component.as_ref().layout_info(Orientation::Horizontal),
                    component.as_ref().layout_info(Orientation::Vertical),
                );

                if let Some((popup, _)) = &*self.active_popup.borrow() {
                    let popup = ComponentRc::borrow_pin(popup);
                    let popup_root = popup.as_ref().get_item_ref(0);
                    if let Some(window_item) = ItemRef::downcast_pin(popup_root) {
                        let layout_info_h = popup.as_ref().layout_info(Orientation::Horizontal);
                        let layout_info_v = popup.as_ref().layout_info(Orientation::Vertical);

                        let width =
                            corelib::items::WindowItem::FIELD_OFFSETS.width.apply_pin(window_item);
                        let mut w = width.get();
                        let height =
                            corelib::items::WindowItem::FIELD_OFFSETS.height.apply_pin(window_item);
                        let mut h = height.get();
                        if w <= 0. {
                            w = layout_info_h.preferred;
                        }
                        if h <= 0. {
                            h = layout_info_v.preferred;
                        }
                        w = w.clamp(layout_info_h.min, layout_info_h.max);
                        h = h.clamp(layout_info_v.min, layout_info_v.max);
                        width.set(w);
                        height.set(h);
                    };
                }
            });

            let map_state = self.map_state.borrow();
            let window = map_state.as_mapped();

            let mut renderer = window.backend.borrow_mut().new_renderer(
                self.clone(),
                &window.clear_color,
                self.scale_factor(),
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
    /// Ideally the popup should be handled as a different window or by the event loop, and
    /// this function can go away
    pub fn process_mouse_input(self: Rc<Self>, mut event: MouseEvent) {
        let active_popup = (*self.active_popup.borrow()).clone();
        let component = if let Some(popup) = &active_popup {
            event.translate(-popup.1.to_vector());
            if let MouseEvent::MousePressed { pos } = &event {
                // close the popup if one press outside the popup
                let geom =
                    ComponentRc::borrow_pin(&popup.0).as_ref().get_item_ref(0).as_ref().geometry();
                if !geom.contains(*pos) {
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
            event,
            &self.self_weak.upgrade().unwrap(),
            self.mouse_input_state.take(),
        ));

        if active_popup.is_some() {
            //FIXME: currently the ComboBox is the only thing that uses the popup, and it should close automatically
            // on release.  But ideally, there would be API to close the popup rather than always closing it on release
            if matches!(event, MouseEvent::MouseReleased { .. }) {
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

    pub fn scale_factor(&self) -> f32 {
        self.self_weak.upgrade().unwrap().scale_factor()
    }

    /// Sets the size of the window. This method is typically called in response to receiving a
    /// window resize event from the windowing system.
    /// Size is in logical pixels.
    pub fn set_geometry(&self, width: f32, height: f32) {
        if let Some(component_rc) = self.self_weak.upgrade().unwrap().try_component() {
            let component = ComponentRc::borrow_pin(&component_rc);
            let root_item = component.as_ref().get_item_ref(0);
            if let Some(window_item) =
                ItemRef::downcast_pin::<corelib::items::WindowItem>(root_item)
            {
                window_item.width.set(width);
                window_item.height.set(height);
            }
        }
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

    fn free_graphics_resources<'a>(&self, items: &Slice<'a, Pin<ItemRef<'a>>>) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(_) => {
                let mut cache_entries_to_clear = items
                    .iter()
                    .flat_map(|item| {
                        let cached_rendering_data = item.cached_rendering_data_offset();
                        cached_rendering_data.release(&mut *self.graphics_cache.borrow_mut())
                    })
                    .peekable();
                if cache_entries_to_clear.peek().is_some() {
                    self.with_current_context(|| {
                        cache_entries_to_clear.for_each(drop);
                    });
                }
            }
        }
    }

    fn show_popup(&self, popup: &ComponentRc, position: Point) {
        *self.active_popup.borrow_mut() = Some((popup.clone(), position));
        if let Some(window) = self.self_weak.upgrade() {
            window.meta_properties_tracker.set_dirty();
        }
    }

    fn close_popup(&self) {
        *self.active_popup.borrow_mut() = None;
        self.request_redraw();
    }

    fn request_window_properties_update(&self) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {
                // Nothing to be done if the window isn't visible. When it becomes visible,
                // corelib::window::Window::show() calls update_window_properties()
            }
            GraphicsWindowBackendState::Mapped(window) => {
                let backend = window.backend.borrow();
                let window_id = backend.window().id();
                crate::event_loop::with_window_target(|event_loop| {
                    event_loop.event_loop_proxy().send_event(
                        crate::event_loop::CustomEvent::UpdateWindowProperties(window_id),
                    )
                })
                .ok();
            }
        }
    }

    fn apply_window_properties(&self, window_item: Pin<&sixtyfps_corelib::items::WindowItem>) {
        let background = window_item.background();
        let title = window_item.title();
        let icon = window_item.icon();
        let width = window_item.width();
        let height = window_item.height();

        match &mut *self.map_state.borrow_mut() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(window) => {
                window.clear_color = background;
                let mut size: LogicalSize<f64> = {
                    let backend = window.backend.borrow();
                    let winit_window = backend.window();
                    winit_window.set_title(&title);
                    if let Some(rgba) = crate::IMAGE_CACHE
                        .with(|c| c.borrow_mut().load_image_resource((&icon).into()))
                        .and_then(|i| i.to_rgba())
                    {
                        let (width, height) = rgba.dimensions();
                        winit_window.set_window_icon(
                            winit::window::Icon::from_rgba(rgba.into_raw(), width, height).ok(),
                        );
                    };
                    winit_window.inner_size().to_logical(self.scale_factor() as f64)
                };
                let mut must_resize = false;
                let mut w = width;
                let mut h = height;
                if (size.width as f32 - w).abs() < 1. || (size.height as f32 - h).abs() < 1. {
                    return;
                }
                if w <= 0. || h <= 0. {
                    if let Some(component_rc) = self.self_weak.upgrade().unwrap().try_component() {
                        let component = ComponentRc::borrow_pin(&component_rc);
                        if w <= 0. {
                            let info = component.as_ref().layout_info(Orientation::Horizontal);
                            w = info.preferred_bounded();
                            must_resize = true;
                        }
                        if h <= 0. {
                            let info = component.as_ref().layout_info(Orientation::Vertical);
                            h = info.preferred_bounded();
                            must_resize = true;
                        }
                    }
                };
                if w > 0. {
                    size.width = w as _;
                }
                if h > 0. {
                    size.height = h as _;
                }
                window.backend.borrow().window().set_inner_size(size);
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

    fn text_size(
        &self,
        item_graphics_cache: &corelib::item_rendering::CachedRenderingData,
        unresolved_font_request_getter: &dyn Fn() -> corelib::graphics::FontRequest,
        text: &str,
        max_width: Option<f32>,
    ) -> Size {
        let font_request_fn =
            || unresolved_font_request_getter().merge(&self.default_font_properties());

        crate::fonts::text_size(
            &self.graphics_cache,
            item_graphics_cache,
            font_request_fn,
            self.self_weak.upgrade().unwrap().scale_factor_property(),
            text,
            max_width,
        )
    }

    fn text_input_byte_offset_for_position(
        &self,
        text_input: Pin<&sixtyfps_corelib::items::TextInput>,
        pos: Point,
    ) -> usize {
        let scale_factor = self.scale_factor();
        let pos = pos * scale_factor;
        let text = text_input.text();

        let mut result = text.len();

        let width = text_input.width() * scale_factor;
        let height = text_input.height() * scale_factor;
        if width <= 0. || height <= 0. {
            return 0;
        }

        let canvas = match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => return 0,
            GraphicsWindowBackendState::Mapped(window) => {
                window.backend.borrow().shared_data.canvas.clone()
            }
        };

        let font = text_input
            .cached_rendering_data
            .get_or_update(&self.graphics_cache, || {
                Some(ItemGraphicsCacheEntry::Font(crate::fonts::FONT_CACHE.with(|cache| {
                    cache.borrow_mut().font(
                        text_input.unresolved_font_request().merge(&self.default_font_properties()),
                        scale_factor,
                        &text_input.text(),
                    )
                })))
            })
            .unwrap()
            .as_font()
            .clone();

        let paint = font.init_paint(text_input.letter_spacing() * scale_factor, Default::default());
        let mut canvas = canvas.borrow_mut();
        let font_height = canvas.measure_font(paint).unwrap().height();
        crate::fonts::layout_text_lines(
            text.as_str(),
            &font,
            Size::new(width, height),
            (text_input.horizontal_alignment(), text_input.vertical_alignment()),
            text_input.wrap(),
            sixtyfps_corelib::items::TextOverflow::clip,
            text_input.single_line(),
            &mut canvas,
            paint,
            |_, line_text, line_pos, start, metrics| {
                if (line_pos.y..(line_pos.y + font_height)).contains(&pos.y) {
                    let mut current_x = 0.;
                    for glyph in &metrics.glyphs {
                        if line_pos.x + current_x + glyph.advance_x / 2. >= pos.x {
                            result = start + glyph.byte_index;
                            return;
                        }
                        current_x += glyph.advance_x;
                    }
                    result = start + line_text.trim_end().len();
                }
            },
        );

        result
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct MappedWindow {
    backend: RefCell<GLRenderer>,
    clear_color: Color,
    constraints: Cell<(corelib::layout::LayoutInfo, corelib::layout::LayoutInfo)>,
}

impl Drop for MappedWindow {
    fn drop(&mut self) {
        crate::event_loop::unregister_window(self.backend.borrow().window().id());
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
            GraphicsWindowBackendState::Mapped(mw) => mw,
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
