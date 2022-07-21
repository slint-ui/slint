// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! This module contains the GraphicsWindow that used to be within corelib.

// cspell:ignore borderless corelib nesw webgl winit winsys xlib

use core::cell::{Cell, RefCell};
use core::pin::Pin;
use std::rc::{Rc, Weak};

use crate::event_loop::WinitWindow;
use crate::glcontext::OpenGLContext;
use const_field_offset::FieldOffsets;
use corelib::api::{
    GraphicsAPI, PhysicalPx, RenderingNotifier, RenderingState, SetRenderingNotifierError,
};
use corelib::component::ComponentRc;
use corelib::graphics::rendering_metrics_collector::RenderingMetricsCollector;
use corelib::input::KeyboardModifiers;
use corelib::items::{ItemRef, MouseCursor};
use corelib::layout::Orientation;
use corelib::window::{PlatformWindow, PopupWindow, PopupWindowLocation};
use corelib::Property;
use corelib::{graphics::*, Coord};
use i_slint_core as corelib;
use winit::dpi::LogicalSize;

pub const PASSWORD_CHARACTER: &str = "●";

/// GraphicsWindow is an implementation of the [PlatformWindow][`crate::eventloop::PlatformWindow`] trait. This is
/// typically instantiated by entry factory functions of the different graphics back ends.
pub struct GLWindow {
    self_weak: Weak<corelib::window::Window>,
    map_state: RefCell<GraphicsWindowBackendState>,
    keyboard_modifiers: std::cell::Cell<KeyboardModifiers>,
    currently_pressed_key_code: std::cell::Cell<Option<winit::event::VirtualKeyCode>>,
    existing_size: Cell<winit::dpi::LogicalSize<f32>>,

    rendering_metrics_collector: Option<Rc<RenderingMetricsCollector>>,

    rendering_notifier: RefCell<Option<Box<dyn RenderingNotifier>>>,

    #[cfg(target_arch = "wasm32")]
    canvas_id: String,

    #[cfg(target_arch = "wasm32")]
    virtual_keyboard_helper: RefCell<Option<super::wasm_input_helper::WasmInputHelper>>,
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
            map_state: RefCell::new(GraphicsWindowBackendState::Unmapped {
                requested_position: None,
                requested_size: None,
            }),
            keyboard_modifiers: Default::default(),
            currently_pressed_key_code: Default::default(),
            existing_size: Default::default(),
            rendering_metrics_collector: RenderingMetricsCollector::new(window_weak.clone()),
            rendering_notifier: Default::default(),
            #[cfg(target_arch = "wasm32")]
            canvas_id,
            #[cfg(target_arch = "wasm32")]
            virtual_keyboard_helper: Default::default(),
        })
    }

    fn with_current_context<T>(
        &self,
        cb: impl FnOnce(&MappedWindow, &OpenGLContext) -> T,
    ) -> Option<T> {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped { .. } => None,
            GraphicsWindowBackendState::Mapped(window) => Some(
                window.opengl_context.with_current_context(|gl_context| cb(window, gl_context)),
            ),
        }
    }

    fn is_mapped(&self) -> bool {
        matches!(&*self.map_state.borrow(), GraphicsWindowBackendState::Mapped { .. })
    }

    fn borrow_mapped_window(&self) -> Option<std::cell::Ref<MappedWindow>> {
        if self.is_mapped() {
            std::cell::Ref::map(self.map_state.borrow(), |state| match state {
                GraphicsWindowBackendState::Unmapped{..} => {
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
            GraphicsWindowBackendState::Unmapped{..} => {
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

    fn release_graphics_resources(&self) {
        // Release GL textures and other GPU bound resources.
        self.with_current_context(|mapped_window, context| {
            mapped_window.femtovg_renderer.release_graphics_resources();

            self.invoke_rendering_notifier(RenderingState::RenderingTeardown, context);
        });
    }

    /// Invoke any registered rendering notifiers about the state the backend renderer is currently in.
    fn invoke_rendering_notifier(&self, state: RenderingState, opengl_context: &OpenGLContext) {
        if let Some(callback) = self.rendering_notifier.borrow_mut().as_mut() {
            #[cfg(not(target_arch = "wasm32"))]
            let api = GraphicsAPI::NativeOpenGL {
                get_proc_address: &|name| opengl_context.get_proc_address(name),
            };
            #[cfg(target_arch = "wasm32")]
            let canvas_element_id = opengl_context.html_canvas_element().id();
            #[cfg(target_arch = "wasm32")]
            let api = GraphicsAPI::WebGL {
                canvas_element_id: canvas_element_id.as_str(),
                context_type: "webgl",
            };
            callback.notify(state, &api)
        }
    }

    fn has_rendering_notifier(&self) -> bool {
        self.rendering_notifier.borrow().is_some()
    }
}

impl WinitWindow for GLWindow {
    fn runtime_window(&self) -> Rc<corelib::window::Window> {
        self.self_weak.upgrade().unwrap()
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
                let mut canvas = window.femtovg_renderer.canvas.as_ref().borrow_mut();
                // We pass 1.0 as dpi / device pixel ratio as femtovg only uses this factor to scale
                // text metrics. Since we do the entire translation from logical pixels to physical
                // pixels on our end, we don't need femtovg to scale a second time.
                canvas.set_size(size.width, size.height, 1.0);
                canvas.clear_rect(
                    0,
                    0,
                    size.width,
                    size.height,
                    crate::renderer::femtovg::itemrenderer::to_femtovg_color(&window.clear_color),
                );
                // For the BeforeRendering rendering notifier callback it's important that this happens *after* clearing
                // the back buffer, in order to allow the callback to provide its own rendering of the background.
                // femtovg's clear_rect() will merely schedule a clear call, so flush right away to make it immediate.
                if self.has_rendering_notifier() {
                    canvas.flush();
                    canvas.set_size(size.width, size.height, 1.0);

                    self.invoke_rendering_notifier(
                        RenderingState::BeforeRendering,
                        &window.opengl_context,
                    );
                }
            }

            let mut renderer = crate::renderer::femtovg::itemrenderer::GLItemRenderer::new(
                &window.femtovg_renderer,
                &self,
                scale_factor,
                size,
            );

            for (component, origin) in components {
                corelib::item_rendering::render_component_items(component, &mut renderer, *origin);
            }

            if let Some(collector) = &self.rendering_metrics_collector {
                collector.measure_frame_rendered(&mut renderer);
            }

            window.femtovg_renderer.finish(renderer);

            self.invoke_rendering_notifier(RenderingState::AfterRendering, &window.opengl_context);

            window.opengl_context.swap_buffers();
            window.opengl_context.make_not_current();
        });
    }

    fn with_window_handle(&self, callback: &mut dyn FnMut(&winit::window::Window)) {
        if let Some(mapped_window) = self.borrow_mapped_window() {
            callback(&*mapped_window.opengl_context.window())
        }
    }

    fn constraints(&self) -> (corelib::layout::LayoutInfo, corelib::layout::LayoutInfo) {
        self.borrow_mapped_window().map(|window| window.constraints.get()).unwrap_or_default()
    }

    fn set_constraints(
        &self,
        constraints: (corelib::layout::LayoutInfo, corelib::layout::LayoutInfo),
    ) {
        if let Some(window) = self.borrow_mapped_window() {
            window.constraints.set(constraints);
        }
    }

    fn existing_size(&self) -> winit::dpi::LogicalSize<f32> {
        self.existing_size.get()
    }

    fn set_existing_size(&self, size: winit::dpi::LogicalSize<f32>) {
        self.existing_size.set(size);
    }

    fn set_background_color(&self, color: Color) {
        if let Some(mut window) = self.borrow_mapped_window_mut() {
            window.clear_color = color;
        }
    }

    fn set_icon(&self, icon: corelib::graphics::Image) {
        let image_inner: &ImageInner = (&icon).into();
        let pixel_buffer = match image_inner {
            ImageInner::EmbeddedImage { buffer, .. } => buffer.clone(),
            _ => return,
        };

        // This could become a method in SharedPixelBuffer...
        let rgba_pixels: Vec<u8> = match &pixel_buffer {
            SharedImageBuffer::RGB8(pixels) => pixels
                .as_bytes()
                .chunks(3)
                .flat_map(|rgb| IntoIterator::into_iter([rgb[0], rgb[1], rgb[2], 255]))
                .collect(),
            SharedImageBuffer::RGBA8(pixels) => pixels.as_bytes().to_vec(),
            SharedImageBuffer::RGBA8Premultiplied(pixels) => pixels
                .as_bytes()
                .chunks(4)
                .flat_map(|rgba| {
                    let alpha = rgba[3] as u32;
                    IntoIterator::into_iter(rgba)
                        .take(3)
                        .map(move |component| (*component as u32 * alpha / 255) as u8)
                        .chain(std::iter::once(alpha as u8))
                })
                .collect(),
        };

        if let Some(window) = self.borrow_mapped_window() {
            window.opengl_context.window().set_window_icon(
                winit::window::Icon::from_rgba(
                    rgba_pixels,
                    pixel_buffer.width(),
                    pixel_buffer.height(),
                )
                .ok(),
            );
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn input_method_focused(&self) -> bool {
        match self.virtual_keyboard_helper.try_borrow() {
            Ok(vkh) => vkh.as_ref().map_or(false, |h| h.has_focus()),
            // the only location in which the virtual_keyboard_helper is mutably borrowed is from
            // show_virtual_keyboard, which means we have the focus
            Err(_) => true,
        }
    }
}

impl PlatformWindow for GLWindow {
    fn request_redraw(&self) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped { .. } => {}
            GraphicsWindowBackendState::Mapped(window) => {
                window.opengl_context.window().request_redraw()
            }
        }
    }

    fn register_component(&self) {}

    fn unregister_component<'a>(
        &self,
        component: corelib::component::ComponentRef,
        _items: &mut dyn Iterator<Item = Pin<ItemRef<'a>>>,
    ) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped { .. } => {}
            GraphicsWindowBackendState::Mapped(_) => {
                self.with_current_context(|mapped_window, _| {
                    mapped_window.femtovg_renderer.component_destroyed(component)
                });
            }
        }
    }

    /// This function is called through the public API to register a callback that the backend needs to invoke during
    /// different phases of rendering.
    fn set_rendering_notifier(
        &self,
        callback: Box<dyn RenderingNotifier>,
    ) -> std::result::Result<(), SetRenderingNotifierError> {
        let mut notifier = self.rendering_notifier.borrow_mut();
        if notifier.replace(callback).is_some() {
            Err(SetRenderingNotifierError::AlreadySet)
        } else {
            Ok(())
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
            GraphicsWindowBackendState::Unmapped { .. } => {
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

    fn apply_window_properties(&self, window_item: Pin<&i_slint_core::items::WindowItem>) {
        // Make the unwrap() calls on self.borrow_mapped_window*() safe
        if !self.is_mapped() {
            return;
        }

        WinitWindow::apply_window_properties(self as &dyn WinitWindow, window_item);
    }

    fn apply_geometry_constraint(
        &self,
        constraints_horizontal: corelib::layout::LayoutInfo,
        constraints_vertical: corelib::layout::LayoutInfo,
    ) {
        self.apply_constraints(constraints_horizontal, constraints_vertical)
    }

    fn show(self: Rc<Self>) {
        let (requested_position, requested_size) = match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped { requested_position, requested_size } => {
                (requested_position.clone(), requested_size.clone())
            }
            GraphicsWindowBackendState::Mapped(_) => return,
        };

        let runtime_window = self.runtime_window();
        let component_rc = runtime_window.component();
        let component = ComponentRc::borrow_pin(&component_rc);
        let root_item = component.as_ref().get_item_ref(0);

        let (window_title, no_frame, is_resizable) = if let Some(window_item) =
            ItemRef::downcast_pin::<corelib::items::WindowItem>(root_item)
        {
            (
                window_item.title().to_string(),
                window_item.no_frame(),
                window_item.height() <= 0 as _ && window_item.width() <= 0 as _,
            )
        } else {
            ("Slint Window".to_string(), false, true)
        };

        let window_builder = winit::window::WindowBuilder::new()
            .with_title(window_title)
            .with_resizable(is_resizable);

        let scale_factor_override = runtime_window.scale_factor();
        // If the scale factor was already set programmatically, use that
        // else, use the SLINT_SCALE_FACTOR if set, otherwise use the one from winit
        let scale_factor_override = if scale_factor_override > 1. {
            Some(scale_factor_override as f64)
        } else {
            std::env::var("SLINT_SCALE_FACTOR")
                .ok()
                .and_then(|x| x.parse::<f64>().ok())
                .filter(|f| *f > 0.)
        };

        let window_builder = if std::env::var("SLINT_FULLSCREEN").is_ok() {
            window_builder.with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))
        } else {
            let layout_info_h = component.as_ref().layout_info(Orientation::Horizontal);
            let layout_info_v = component.as_ref().layout_info(Orientation::Vertical);
            let s = LogicalSize::new(
                layout_info_h.preferred_bounded(),
                layout_info_v.preferred_bounded(),
            );

            if let Some(requested_size) = requested_size {
                // It would be nice to bound this with our constraints, but those are in logical coordinates
                // and we don't know the scale factor yet...
                window_builder.with_inner_size(winit::dpi::Size::new(
                    winit::dpi::PhysicalSize::new(requested_size.width, requested_size.height),
                ))
            } else if s.width > 0 as Coord && s.height > 0 as Coord {
                // Make sure that the window's inner size is in sync with the root window item's
                // width/height.
                runtime_window.set_window_item_geometry(s.width, s.height);
                if let Some(f) = scale_factor_override {
                    window_builder.with_inner_size(s.to_physical::<f32>(f))
                } else {
                    window_builder.with_inner_size(s)
                }
            } else {
                window_builder
            }
        };

        let window_builder =
            if no_frame { window_builder.with_decorations(false) } else { window_builder };

        let window_builder = if let Some(requested_position) = requested_position {
            window_builder.with_position(winit::dpi::Position::new(
                winit::dpi::PhysicalPosition::new(requested_position.x, requested_position.y),
            ))
        } else {
            window_builder
        };

        #[cfg(target_arch = "wasm32")]
        let (opengl_context, renderer) =
            crate::OpenGLContext::new_context_and_renderer(window_builder, &self.canvas_id);
        #[cfg(not(target_arch = "wasm32"))]
        let (opengl_context, renderer) =
            crate::OpenGLContext::new_context_and_renderer(window_builder);

        let canvas = femtovg::Canvas::new_with_text_context(
            renderer,
            crate::renderer::femtovg::fonts::FONT_CACHE
                .with(|cache| cache.borrow().text_context.clone()),
        )
        .unwrap();

        self.invoke_rendering_notifier(RenderingState::RenderingSetup, &opengl_context);

        opengl_context.make_not_current();

        let canvas = Rc::new(RefCell::new(canvas));

        let platform_window = opengl_context.window();
        let runtime_window = self.self_weak.upgrade().unwrap();
        runtime_window.set_scale_factor(
            scale_factor_override.unwrap_or_else(|| platform_window.scale_factor()) as _,
        );
        let id = platform_window.id();

        if let Some(collector) = &self.rendering_metrics_collector {
            cfg_if::cfg_if! {
                if #[cfg(target_arch = "wasm32")] {
                    let winsys = "HTML Canvas";
                } else if #[cfg(any(
                    target_os = "linux",
                    target_os = "dragonfly",
                    target_os = "freebsd",
                    target_os = "netbsd",
                    target_os = "openbsd"
                ))] {
                    use winit::platform::unix::WindowExtUnix;
                    let mut winsys = "unknown";

                    #[cfg(feature = "x11")]
                    if platform_window.xlib_window().is_some() {
                        winsys = "x11";
                    }

                    #[cfg(feature = "wayland")]
                    if platform_window.wayland_surface().is_some() {
                        winsys = "wayland"
                    }
                } else if #[cfg(target_os = "windows")] {
                    let winsys = "windows";
                } else if #[cfg(target_os = "macos")] {
                    let winsys = "macos";
                } else {
                    let winsys = "unknown";
                }
            }

            collector.start(&format!("GL backend (windowing system: {})", winsys));
        }

        drop(platform_window);

        self.map_state.replace(GraphicsWindowBackendState::Mapped(MappedWindow {
            femtovg_renderer: crate::renderer::femtovg::FemtoVGRenderer::new(canvas),
            opengl_context,
            clear_color: RgbaColor { red: 255_u8, green: 255, blue: 255, alpha: 255 }.into(),
            constraints: Default::default(),
        }));

        crate::event_loop::register_window(id, self);
    }

    fn hide(self: Rc<Self>) {
        // Release GL textures and other GPU bound resources.
        self.release_graphics_resources();

        self.map_state.replace(GraphicsWindowBackendState::Unmapped {
            requested_position: None,
            requested_size: None,
        });
        /* FIXME:
        if let Some(existing_blinker) = self.cursor_blinker.borrow().upgrade() {
            existing_blinker.stop();
        }*/
        crate::event_loop::with_window_target(|event_loop| {
            event_loop.event_loop_proxy().send_event(crate::event_loop::CustomEvent::WindowHidden)
        })
        .unwrap();
    }

    fn set_mouse_cursor(&self, cursor: MouseCursor) {
        let winit_cursor = match cursor {
            MouseCursor::default => winit::window::CursorIcon::Default,
            MouseCursor::none => winit::window::CursorIcon::Default,
            MouseCursor::help => winit::window::CursorIcon::Help,
            MouseCursor::pointer => winit::window::CursorIcon::Hand,
            MouseCursor::progress => winit::window::CursorIcon::Progress,
            MouseCursor::wait => winit::window::CursorIcon::Wait,
            MouseCursor::crosshair => winit::window::CursorIcon::Crosshair,
            MouseCursor::text => winit::window::CursorIcon::Text,
            MouseCursor::alias => winit::window::CursorIcon::Alias,
            MouseCursor::copy => winit::window::CursorIcon::Copy,
            MouseCursor::r#move => winit::window::CursorIcon::Move,
            MouseCursor::no_drop => winit::window::CursorIcon::NoDrop,
            MouseCursor::not_allowed => winit::window::CursorIcon::NotAllowed,
            MouseCursor::grab => winit::window::CursorIcon::Grab,
            MouseCursor::grabbing => winit::window::CursorIcon::Grabbing,
            MouseCursor::col_resize => winit::window::CursorIcon::ColResize,
            MouseCursor::row_resize => winit::window::CursorIcon::RowResize,
            MouseCursor::n_resize => winit::window::CursorIcon::NResize,
            MouseCursor::e_resize => winit::window::CursorIcon::EResize,
            MouseCursor::s_resize => winit::window::CursorIcon::SResize,
            MouseCursor::w_resize => winit::window::CursorIcon::WResize,
            MouseCursor::ne_resize => winit::window::CursorIcon::NeResize,
            MouseCursor::nw_resize => winit::window::CursorIcon::NwResize,
            MouseCursor::se_resize => winit::window::CursorIcon::SeResize,
            MouseCursor::sw_resize => winit::window::CursorIcon::SwResize,
            MouseCursor::ew_resize => winit::window::CursorIcon::EwResize,
            MouseCursor::ns_resize => winit::window::CursorIcon::NsResize,
            MouseCursor::nesw_resize => winit::window::CursorIcon::NeswResize,
            MouseCursor::nwse_resize => winit::window::CursorIcon::NwseResize,
        };
        self.with_window_handle(&mut |winit_window| {
            winit_window.set_cursor_visible(cursor != MouseCursor::none);
            winit_window.set_cursor_icon(winit_cursor);
        });
    }

    fn text_size(
        &self,
        font_request: corelib::graphics::FontRequest,
        text: &str,
        max_width: Option<Coord>,
    ) -> Size {
        let font_request = font_request.merge(&self.default_font_properties());

        crate::renderer::femtovg::fonts::text_size(
            &font_request,
            self.self_weak.upgrade().unwrap().scale_factor(),
            text,
            max_width,
        )
    }

    fn text_input_byte_offset_for_position(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        pos: Point,
    ) -> usize {
        let scale_factor = self.self_weak.upgrade().unwrap().scale_factor();
        let pos = pos * scale_factor;
        let text = text_input.text();

        let mut result = text.len();

        let width = text_input.width() * scale_factor;
        let height = text_input.height() * scale_factor;
        if width <= 0. || height <= 0. || pos.y < 0. {
            return 0;
        }

        let font = crate::renderer::femtovg::fonts::FONT_CACHE.with(|cache| {
            cache.borrow_mut().font(
                text_input.unresolved_font_request().merge(&self.default_font_properties()),
                scale_factor,
                &text_input.text(),
            )
        });

        let is_password = matches!(text_input.input_type(), corelib::items::InputType::password);
        let password_string;
        let actual_text = if is_password {
            password_string = PASSWORD_CHARACTER.repeat(text.chars().count());
            password_string.as_str()
        } else {
            text.as_str()
        };

        let paint = font.init_paint(text_input.letter_spacing() * scale_factor, Default::default());
        let text_context = crate::renderer::femtovg::fonts::FONT_CACHE
            .with(|cache| cache.borrow().text_context.clone());
        let font_height = text_context.measure_font(paint).unwrap().height();
        crate::renderer::femtovg::fonts::layout_text_lines(
            actual_text,
            &font,
            Size::new(width, height),
            (text_input.horizontal_alignment(), text_input.vertical_alignment()),
            text_input.wrap(),
            i_slint_core::items::TextOverflow::clip,
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

        if is_password {
            text.char_indices()
                .nth(result / PASSWORD_CHARACTER.len())
                .map_or(text.len(), |(r, _)| r)
        } else {
            result
        }
    }

    fn text_input_cursor_rect_for_byte_offset(
        &self,
        text_input: Pin<&corelib::items::TextInput>,
        byte_offset: usize,
    ) -> Rect {
        use crate::renderer::femtovg::fonts;
        let scale_factor = self.self_weak.upgrade().unwrap().scale_factor();
        let text = text_input.text();

        let font_size = text_input
            .unresolved_font_request()
            .merge(&self.default_font_properties())
            .pixel_size
            .unwrap_or(fonts::DEFAULT_FONT_SIZE);

        let mut result = Point::default();

        let width = text_input.width() * scale_factor;
        let height = text_input.height() * scale_factor;
        if width <= 0. || height <= 0. {
            return Rect::new(result, Size::new(1.0, font_size));
        }

        let font = crate::renderer::femtovg::fonts::FONT_CACHE.with(|cache| {
            cache.borrow_mut().font(
                text_input.unresolved_font_request().merge(&self.default_font_properties()),
                scale_factor,
                &text_input.text(),
            )
        });

        let paint = font.init_paint(text_input.letter_spacing() * scale_factor, Default::default());
        fonts::layout_text_lines(
            text.as_str(),
            &font,
            Size::new(width, height),
            (text_input.horizontal_alignment(), text_input.vertical_alignment()),
            text_input.wrap(),
            i_slint_core::items::TextOverflow::clip,
            text_input.single_line(),
            paint,
            |line_text, line_pos, start, metrics| {
                if (start..=(start + line_text.len())).contains(&byte_offset) {
                    for glyph in &metrics.glyphs {
                        if glyph.byte_index == (byte_offset - start) {
                            result = line_pos + euclid::vec2(glyph.x, 0.0);
                            return;
                        }
                    }
                    if let Some(last) = metrics.glyphs.last() {
                        result = line_pos + euclid::vec2(last.x + last.advance_x, last.y);
                    }
                }
            },
        );

        Rect::new(result / scale_factor, Size::new(1.0, font_size))
    }

    #[cfg(target_arch = "wasm32")]
    fn show_virtual_keyboard(&self, _it: corelib::items::InputType) {
        let mut vkh = self.virtual_keyboard_helper.borrow_mut();
        let h = vkh.get_or_insert_with(|| {
            let canvas =
                self.borrow_mapped_window().unwrap().opengl_context.html_canvas_element().clone();
            super::wasm_input_helper::WasmInputHelper::new(self.self_weak.clone(), canvas)
        });
        h.show();
    }

    #[cfg(target_arch = "wasm32")]
    fn hide_virtual_keyboard(&self) {
        if let Some(h) = &*self.virtual_keyboard_helper.borrow() {
            h.hide()
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn position(&self) -> euclid::Point2D<i32, PhysicalPx> {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped { requested_position, .. } => {
                requested_position.unwrap_or_default()
            }
            GraphicsWindowBackendState::Mapped(mapped_window) => {
                let winit_window = &*mapped_window.opengl_context.window();
                match winit_window.outer_position() {
                    Ok(position) => euclid::Point2D::new(position.x, position.y),
                    Err(_) => Default::default(),
                }
            }
        }
    }

    fn set_position(&self, position: euclid::Point2D<i32, PhysicalPx>) {
        match &mut *self.map_state.borrow_mut() {
            GraphicsWindowBackendState::Unmapped { requested_position, .. } => {
                *requested_position = Some(position)
            }
            GraphicsWindowBackendState::Mapped(mapped_window) => {
                let winit_window = &*mapped_window.opengl_context.window();
                winit_window.set_outer_position(winit::dpi::Position::new(
                    winit::dpi::PhysicalPosition::new(position.x, position.y),
                ))
            }
        }
    }

    fn inner_size(&self) -> euclid::Size2D<u32, PhysicalPx> {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped { requested_size, .. } => {
                requested_size.unwrap_or_default()
            }
            GraphicsWindowBackendState::Mapped(mapped_window) => {
                let winit_window = &*mapped_window.opengl_context.window();
                let size = winit_window.inner_size();
                euclid::Size2D::new(size.width, size.height)
            }
        }
    }

    fn set_inner_size(&self, size: euclid::Size2D<u32, PhysicalPx>) {
        match &mut *self.map_state.borrow_mut() {
            GraphicsWindowBackendState::Unmapped { requested_size, .. } => {
                *requested_size = Some(size)
            }
            GraphicsWindowBackendState::Mapped(mapped_window) => {
                let winit_window = &*mapped_window.opengl_context.window();
                winit_window.set_inner_size(winit::dpi::Size::new(winit::dpi::PhysicalSize::new(
                    size.width,
                    size.height,
                )))
            }
        }
    }
}

impl Drop for GLWindow {
    fn drop(&mut self) {
        self.release_graphics_resources();
    }
}

struct MappedWindow {
    femtovg_renderer: crate::renderer::femtovg::FemtoVGRenderer,
    opengl_context: crate::OpenGLContext,
    clear_color: Color,
    constraints: Cell<(corelib::layout::LayoutInfo, corelib::layout::LayoutInfo)>,
}

impl Drop for MappedWindow {
    fn drop(&mut self) {
        // The GL renderer must be destructed with a GL context current, in order to clean up correctly.
        self.opengl_context.make_current();
        crate::event_loop::unregister_window(self.opengl_context.window().id());
    }
}

enum GraphicsWindowBackendState {
    Unmapped {
        requested_position: Option<euclid::Point2D<i32, PhysicalPx>>,
        requested_size: Option<euclid::Size2D<u32, PhysicalPx>>,
    },
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
