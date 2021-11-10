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

use super::{ImageCache, ItemGraphicsCache};
use crate::event_loop::WinitWindow;
use const_field_offset::FieldOffsets;
use corelib::component::ComponentRc;
use corelib::graphics::*;
use corelib::input::KeyboardModifiers;
use corelib::items::ItemRef;
use corelib::layout::Orientation;
use corelib::window::{PlatformWindow, PopupWindow, PopupWindowLocation};
use corelib::Property;
use sixtyfps_corelib as corelib;
use winit::dpi::LogicalSize;

use crate::CanvasRc;

/// GraphicsWindow is an implementation of the [PlatformWindow][`crate::eventloop::PlatformWindow`] trait. This is
/// typically instantiated by entry factory functions of the different graphics back ends.
pub struct GLWindow {
    self_weak: Weak<corelib::window::Window>,
    map_state: RefCell<GraphicsWindowBackendState>,
    keyboard_modifiers: std::cell::Cell<KeyboardModifiers>,
    currently_pressed_key_code: std::cell::Cell<Option<winit::event::VirtualKeyCode>>,

    pub(crate) graphics_cache: RefCell<ItemGraphicsCache>,
    // This cache only contains textures. The cache for decoded CPU side images is in crate::IMAGE_CACHE.
    pub(crate) texture_cache: RefCell<ImageCache>,

    #[cfg(target_arch = "wasm32")]
    canvas_id: String,
}

impl GLWindow {
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
            graphics_cache: Default::default(),
            texture_cache: Default::default(),
            #[cfg(target_arch = "wasm32")]
            canvas_id,
        })
    }

    fn with_current_context<T>(&self, cb: impl FnOnce() -> T) -> T {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => cb(),
            GraphicsWindowBackendState::Mapped(window) => {
                window.opengl_context.with_current_context(cb)
            }
        }
    }

    fn is_mapped(&self) -> bool {
        matches!(&*self.map_state.borrow(), GraphicsWindowBackendState::Mapped { .. })
    }

    fn borrow_mapped_window(&self) -> Option<std::cell::Ref<MappedWindow>> {
        if self.is_mapped() {
            std::cell::Ref::map(self.map_state.borrow(), |state| match state {
                GraphicsWindowBackendState::Unmapped => {
                    panic!("borrow_mapped_window must be called after checking if the window is mapped")
                }
                GraphicsWindowBackendState::Mapped(window) => window,
            }).into()
        } else {
            None
        }
    }

    fn borrow_mapped_window_mut(&self) -> Option<std::cell::RefMut<MappedWindow>> {
        if self.is_mapped() {
            std::cell::RefMut::map(self.map_state.borrow_mut(), |state| match state {
            GraphicsWindowBackendState::Unmapped => {
                panic!("borrow_mapped_window_mut must be called after checking if the window is mapped")
            }
            GraphicsWindowBackendState::Mapped(window) => window,
        }).into()
        } else {
            None
        }
    }

    pub fn default_font_properties(&self) -> FontRequest {
        self.self_weak.upgrade().unwrap().default_font_properties()
    }
}

impl WinitWindow for GLWindow {
    fn runtime_window(&self) -> Rc<corelib::window::Window> {
        self.self_weak.upgrade().unwrap()
    }

    /// Sets the size of the window. This method is typically called in response to receiving a
    /// window resize event from the windowing system.
    /// Size is in logical pixels.
    fn set_geometry(&self, width: f32, height: f32) {
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
    fn currently_pressed_key_code(&self) -> &Cell<Option<winit::event::VirtualKeyCode>> {
        &self.currently_pressed_key_code
    }

    fn current_keyboard_modifiers(&self) -> &Cell<KeyboardModifiers> {
        &self.keyboard_modifiers
    }

    /// Draw the items of the specified `component` in the given window.
    fn draw(self: Rc<Self>) {
        let runtime_window = self.self_weak.upgrade().unwrap();
        let scale_factor = runtime_window.scale_factor();
        runtime_window.draw_contents(|components| {
            let window = match self.borrow_mapped_window() {
                Some(window) => window,
                None => return, // caller bug, doesn't make sense to call draw() when not mapped
            };

            let size = window.opengl_context.window().inner_size();

            window.opengl_context.make_current();
            window.opengl_context.ensure_resized();

            {
                let mut canvas = window.canvas.borrow_mut();
                // We pass 1.0 as dpi / device pixel ratio as femtovg only uses this factor to scale
                // text metrics. Since we do the entire translation from logical pixels to physical
                // pixels on our end, we don't need femtovg to scale a second time.
                canvas.set_size(size.width, size.height, 1.0);
                canvas.clear_rect(
                    0,
                    0,
                    size.width,
                    size.height,
                    crate::to_femtovg_color(&window.clear_color),
                );
            }

            let mut renderer = crate::GLItemRenderer {
                canvas: window.canvas.clone(),
                layer_images_to_delete_after_flush: Default::default(),
                graphics_window: self.clone(),
                scale_factor,
                state: vec![crate::State {
                    scissor: Rect::new(
                        Point::default(),
                        Size::new(size.width as _, size.height as _),
                    ),
                    global_alpha: 1.,
                    layer: None,
                }],
            };

            for (component, origin) in components {
                corelib::item_rendering::render_component_items(
                    &component,
                    &mut renderer,
                    origin.clone(),
                );
            }

            renderer.canvas.borrow_mut().flush();

            // Delete any images and layer images (and their FBOs) before making the context not current anymore, to
            // avoid GPU memory leaks.
            renderer.graphics_window.texture_cache.borrow_mut().drain();

            drop(renderer);

            window.opengl_context.swap_buffers();
            window.opengl_context.make_not_current();
        });
    }
}

impl PlatformWindow for GLWindow {
    fn request_redraw(&self) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(window) => {
                window.opengl_context.window().request_redraw()
            }
        }
    }

    fn free_graphics_resources<'a>(&self, items: &mut dyn Iterator<Item = Pin<ItemRef<'a>>>) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(_) => {
                let mut cache_entries_to_clear = items
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
        let runtime_window = self.self_weak.upgrade().unwrap();
        let size = runtime_window.set_active_popup(PopupWindow {
            location: PopupWindowLocation::ChildWindow(position),
            component: popup.clone(),
        });

        let popup = ComponentRc::borrow_pin(popup);
        let popup_root = popup.as_ref().get_item_ref(0);
        if let Some(window_item) = ItemRef::downcast_pin(popup_root) {
            let width_property =
                corelib::items::WindowItem::FIELD_OFFSETS.width.apply_pin(window_item);
            let height_property =
                corelib::items::WindowItem::FIELD_OFFSETS.height.apply_pin(window_item);
            width_property.set(size.width);
            height_property.set(size.height);
        }
    }

    fn request_window_properties_update(&self) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {
                // Nothing to be done if the window isn't visible. When it becomes visible,
                // corelib::window::Window::show() calls update_window_properties()
            }
            GraphicsWindowBackendState::Mapped(window) => {
                let window_id = window.opengl_context.window().id();
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
        let no_frame = window_item.no_frame();
        let icon = window_item.icon();
        let width = window_item.width();
        let height = window_item.height();

        // Make the unwrap() calls on self.borrow_mapped_window*() safe
        if !self.is_mapped() {
            return;
        }

        self.borrow_mapped_window_mut().unwrap().clear_color = background;

        let mut size: LogicalSize<f64> = {
            let window = self.borrow_mapped_window().unwrap();
            let winit_window = window.opengl_context.window();
            winit_window.set_title(&title);
            if no_frame && winit_window.fullscreen().is_none() {
                winit_window.set_decorations(false);
            } else {
                winit_window.set_decorations(true);
            }
            if let Some(rgba) = crate::IMAGE_CACHE
                .with(|c| c.borrow_mut().load_image_resource((&icon).into()))
                .and_then(|i| i.to_rgba())
            {
                let (width, height) = rgba.dimensions();
                winit_window.set_window_icon(
                    winit::window::Icon::from_rgba(rgba.into_raw(), width, height).ok(),
                );
            };
            winit_window.inner_size().to_logical(winit_window.scale_factor() as f64)
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
        {
            let mapped_window = self.borrow_mapped_window().unwrap();
            let winit_window = mapped_window.opengl_context.window();
            // If we're in fullscreen state, don't try to resize the window but maintain the surface
            // size we've been assigned to from the windowing system. Weston/Wayland don't like it
            // when we create a surface that's bigger than the screen due to constraints (#532).
            if winit_window.fullscreen().is_none() {
                winit_window.set_inner_size(size);
            }
        }
        if must_resize {
            self.set_geometry(size.width as _, size.height as _)
        }
    }

    fn apply_geometry_constraint(
        &self,
        constraints_horizontal: corelib::layout::LayoutInfo,
        constraints_vertical: corelib::layout::LayoutInfo,
    ) {
        if let Some(window) = self.borrow_mapped_window() {
            let winit_window = window.opengl_context.window();
            // If we're in fullscreen state, don't try to resize the window but maintain the surface
            // size we've been assigned to from the windowing system. Weston/Wayland don't like it
            // when we create a surface that's bigger than the screen due to constraints (#532).
            if winit_window.fullscreen().is_some() {
                return;
            }

            if (constraints_horizontal, constraints_vertical) != window.constraints.get() {
                let min_width = constraints_horizontal.min.min(constraints_horizontal.max);
                let min_height = constraints_vertical.min.min(constraints_vertical.max);
                let max_width = constraints_horizontal.max.max(constraints_horizontal.min);
                let max_height = constraints_vertical.max.max(constraints_vertical.min);

                winit_window.set_min_inner_size(if min_width > 0. || min_height > 0. {
                    Some(winit::dpi::LogicalSize::new(min_width, min_height))
                } else {
                    None
                });
                winit_window.set_max_inner_size(if max_width < f32::MAX || max_height < f32::MAX {
                    Some(winit::dpi::LogicalSize::new(
                        max_width.min(65535.),
                        max_height.min(65535.),
                    ))
                } else {
                    None
                });
                window.constraints.set((constraints_horizontal, constraints_vertical));

                #[cfg(target_arch = "wasm32")]
                {
                    // set_max_inner_size / set_min_inner_size don't work on wasm, so apply the size manually
                    let existing_size = window.opengl_context.window().inner_size();
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
                        window.opengl_context.window().set_inner_size(new_size);
                    }
                }
            }
        }
    }

    fn show(self: Rc<Self>) {
        if self.is_mapped() {
            return;
        }

        let component_rc = self.self_weak.upgrade().unwrap().component();
        let component = ComponentRc::borrow_pin(&component_rc);
        let root_item = component.as_ref().get_item_ref(0);

        let (window_title, no_frame, is_resizable) = if let Some(window_item) =
            ItemRef::downcast_pin::<corelib::items::WindowItem>(root_item)
        {
            (
                window_item.title().to_string(),
                window_item.no_frame(),
                window_item.height() == 0. && window_item.width() == 0.,
            )
        } else {
            ("SixtyFPS Window".to_string(), false, true)
        };

        let window_builder = winit::window::WindowBuilder::new()
            .with_title(window_title)
            .with_resizable(is_resizable);

        let window_builder = if std::env::var("SIXTYFPS_FULLSCREEN").is_ok() {
            window_builder.with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))
        } else {
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

        let window_builder =
            if no_frame { window_builder.with_decorations(false) } else { window_builder };

        #[cfg(target_arch = "wasm32")]
        let (opengl_context, renderer) =
            crate::OpenGLContext::new_context_and_renderer(window_builder, &self.canvas_id);
        #[cfg(not(target_arch = "wasm32"))]
        let (opengl_context, renderer) =
            crate::OpenGLContext::new_context_and_renderer(window_builder);

        let canvas = femtovg::Canvas::new_with_text_context(
            renderer,
            crate::fonts::FONT_CACHE.with(|cache| cache.borrow().text_context.clone()),
        )
        .unwrap();

        opengl_context.make_not_current();

        let canvas = Rc::new(RefCell::new(canvas));

        let platform_window = opengl_context.window();
        let runtime_window = self.self_weak.upgrade().unwrap();
        runtime_window.set_scale_factor(platform_window.scale_factor() as _);
        let id = platform_window.id();
        drop(platform_window);

        self.map_state.replace(GraphicsWindowBackendState::Mapped(MappedWindow {
            canvas,
            opengl_context,
            clear_color: RgbaColor { red: 255_u8, green: 255, blue: 255, alpha: 255 }.into(),
            constraints: Default::default(),
        }));

        crate::event_loop::register_window(id, self);
    }

    fn hide(self: Rc<Self>) {
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

    fn text_size(
        &self,
        font_request: corelib::graphics::FontRequest,
        text: &str,
        max_width: Option<f32>,
    ) -> Size {
        let font_request = font_request.merge(&self.default_font_properties());

        crate::fonts::text_size(
            &font_request,
            self.self_weak.upgrade().unwrap().scale_factor(),
            text,
            max_width,
        )
    }

    fn text_input_byte_offset_for_position(
        &self,
        text_input: Pin<&sixtyfps_corelib::items::TextInput>,
        pos: Point,
    ) -> usize {
        let scale_factor = self.self_weak.upgrade().unwrap().scale_factor();
        let pos = pos * scale_factor;
        let text = text_input.text();

        let mut result = text.len();

        let width = text_input.width() * scale_factor;
        let height = text_input.height() * scale_factor;
        if width <= 0. || height <= 0. {
            return 0;
        }

        let font = crate::fonts::FONT_CACHE.with(|cache| {
            cache.borrow_mut().font(
                text_input.unresolved_font_request().merge(&self.default_font_properties()),
                scale_factor,
                &text_input.text(),
            )
        });

        let paint = font.init_paint(text_input.letter_spacing() * scale_factor, Default::default());
        let text_context =
            crate::fonts::FONT_CACHE.with(|cache| cache.borrow().text_context.clone());
        let font_height = text_context.measure_font(paint).unwrap().height();
        crate::fonts::layout_text_lines(
            text.as_str(),
            &font,
            Size::new(width, height),
            (text_input.horizontal_alignment(), text_input.vertical_alignment()),
            text_input.wrap(),
            sixtyfps_corelib::items::TextOverflow::clip,
            text_input.single_line(),
            paint,
            |line_text, line_pos, start, metrics| {
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

    fn text_input_position_for_byte_offset(
        &self,
        text_input: Pin<&corelib::items::TextInput>,
        byte_offset: usize,
    ) -> Point {
        let scale_factor = self.self_weak.upgrade().unwrap().scale_factor();
        let text = text_input.text();

        let mut result = Point::default();

        let width = text_input.width() * scale_factor;
        let height = text_input.height() * scale_factor;
        if width <= 0. || height <= 0. {
            return result;
        }

        let font = crate::fonts::FONT_CACHE.with(|cache| {
            cache.borrow_mut().font(
                text_input.unresolved_font_request().merge(&self.default_font_properties()),
                scale_factor,
                &text_input.text(),
            )
        });

        let paint = font.init_paint(text_input.letter_spacing() * scale_factor, Default::default());
        crate::fonts::layout_text_lines(
            text.as_str(),
            &font,
            Size::new(width, height),
            (text_input.horizontal_alignment(), text_input.vertical_alignment()),
            text_input.wrap(),
            sixtyfps_corelib::items::TextOverflow::clip,
            text_input.single_line(),
            paint,
            |line_text, line_pos, start, metrics| {
                if (start..=(start + line_text.len())).contains(&byte_offset) {
                    for glyph in &metrics.glyphs {
                        if glyph.byte_index == (byte_offset - start) {
                            result = line_pos + euclid::vec2(glyph.x, glyph.y);
                            return;
                        }
                    }
                    if let Some(last) = metrics.glyphs.last() {
                        result = line_pos + euclid::vec2(last.x + last.advance_x, last.y);
                    }
                }
            },
        );

        result / scale_factor
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct MappedWindow {
    canvas: CanvasRc,
    opengl_context: crate::OpenGLContext,
    clear_color: Color,
    constraints: Cell<(corelib::layout::LayoutInfo, corelib::layout::LayoutInfo)>,
}

impl Drop for MappedWindow {
    fn drop(&mut self) {
        crate::event_loop::unregister_window(self.opengl_context.window().id());
    }
}

enum GraphicsWindowBackendState {
    Unmapped,
    Mapped(MappedWindow),
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
