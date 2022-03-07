// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;
use embedded_graphics_simulator::SimulatorDisplay;
use i_slint_core::component::ComponentRc;
use i_slint_core::graphics::{Image, ImageInner, StaticTextures};
use i_slint_core::input::KeyboardModifiers;
use i_slint_core::items::ItemRef;
use i_slint_core::layout::Orientation;
use i_slint_core::window::{PlatformWindow, Window};
use i_slint_core::Color;
use rgb::FromSlice;

use self::event_loop::WinitWindow;

type Canvas = femtovg::Canvas<femtovg::renderer::OpenGl>;
type CanvasRc = Rc<RefCell<Canvas>>;

pub mod event_loop;
mod glcontext;
use glcontext::*;

pub struct SimulatorWindow {
    self_weak: Weak<i_slint_core::window::Window>,
    keyboard_modifiers: std::cell::Cell<KeyboardModifiers>,
    currently_pressed_key_code: std::cell::Cell<Option<winit::event::VirtualKeyCode>>,
    canvas: CanvasRc,
    opengl_context: OpenGLContext,
    constraints: Cell<(i_slint_core::layout::LayoutInfo, i_slint_core::layout::LayoutInfo)>,
    visible: Cell<bool>,
    background_color: Cell<Color>,
}

impl SimulatorWindow {
    pub(crate) fn new(window_weak: &Weak<i_slint_core::window::Window>) -> Rc<Self> {
        let window_builder = winit::window::WindowBuilder::new().with_visible(false);

        #[cfg(target_arch = "wasm32")]
        let (opengl_context, renderer) =
            OpenGLContext::new_context_and_renderer(window_builder, &self.canvas_id);
        #[cfg(not(target_arch = "wasm32"))]
        let (opengl_context, renderer) = OpenGLContext::new_context_and_renderer(window_builder);

        let canvas = femtovg::Canvas::new(renderer).unwrap();

        opengl_context.make_not_current();

        let canvas = Rc::new(RefCell::new(canvas));

        let window_rc = Rc::new(Self {
            self_weak: window_weak.clone(),
            keyboard_modifiers: Default::default(),
            currently_pressed_key_code: Default::default(),
            canvas,
            opengl_context,
            constraints: Default::default(),
            visible: Default::default(),
            background_color: Color::from_rgb_u8(0, 0, 0).into(),
        });

        let runtime_window = window_weak.upgrade().unwrap();
        runtime_window.set_scale_factor(window_rc.opengl_context.window().scale_factor() as _);

        window_rc
    }
}

impl Drop for SimulatorWindow {
    fn drop(&mut self) {
        crate::event_loop::unregister_window(self.opengl_context.window().id());
    }
}

impl PlatformWindow for SimulatorWindow {
    fn show(self: Rc<Self>) {
        if self.visible.get() {
            return;
        }

        self.visible.set(true);

        let runtime_window = self.runtime_window();
        let component_rc = runtime_window.component();
        let component = ComponentRc::borrow_pin(&component_rc);
        let root_item = component.as_ref().get_item_ref(0);

        let platform_window = self.opengl_context.window();

        if let Some(window_item) =
            ItemRef::downcast_pin::<i_slint_core::items::WindowItem>(root_item)
        {
            platform_window.set_title(&window_item.title());
            platform_window.set_decorations(!window_item.no_frame());
        };

        if std::env::var("SLINT_FULLSCREEN").is_ok() {
            platform_window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))
        } else {
            let layout_info_h = component.as_ref().layout_info(Orientation::Horizontal);
            let layout_info_v = component.as_ref().layout_info(Orientation::Vertical);
            let s = winit::dpi::LogicalSize::new(
                layout_info_h.preferred_bounded(),
                layout_info_v.preferred_bounded(),
            );
            if s.width > 0. && s.height > 0. {
                // Make sure that the window's inner size is in sync with the root window item's
                // width/height.
                runtime_window.set_window_item_geometry(s.width, s.height);
                platform_window.set_inner_size(s)
            }
        };

        platform_window.set_visible(true);
        let id = platform_window.id();
        drop(platform_window);
        crate::event_loop::register_window(id, self);
    }

    fn hide(self: Rc<Self>) {
        self.opengl_context.window().set_visible(false);
        self.visible.set(false);
        crate::event_loop::unregister_window(self.opengl_context.window().id());
    }

    fn request_redraw(&self) {
        if self.visible.get() {
            self.opengl_context.window().request_redraw();
        }
    }

    fn free_graphics_resources<'a>(
        &self,
        _items: &mut dyn Iterator<Item = std::pin::Pin<i_slint_core::items::ItemRef<'a>>>,
    ) {
        // Nothing to do until we start caching stuff that needs freeing
    }

    fn show_popup(
        &self,
        _popup: &i_slint_core::component::ComponentRc,
        _position: i_slint_core::graphics::Point,
    ) {
        todo!()
    }

    fn request_window_properties_update(&self) {
        let window_id = self.opengl_context.window().id();
        crate::event_loop::with_window_target(|event_loop| {
            event_loop
                .event_loop_proxy()
                .send_event(crate::event_loop::CustomEvent::UpdateWindowProperties(window_id))
        })
        .ok();
    }

    fn apply_window_properties(
        &self,
        window_item: std::pin::Pin<&i_slint_core::items::WindowItem>,
    ) {
        WinitWindow::apply_window_properties(self as &dyn WinitWindow, window_item);
    }

    fn apply_geometry_constraint(
        &self,
        constraints_horizontal: i_slint_core::layout::LayoutInfo,
        constraints_vertical: i_slint_core::layout::LayoutInfo,
    ) {
        self.apply_constraints(constraints_horizontal, constraints_vertical)
    }

    fn set_mouse_cursor(&self, _cursor: i_slint_core::items::MouseCursor) {}

    fn text_size(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
        text: &str,
        max_width: Option<f32>,
    ) -> i_slint_core::graphics::Size {
        let runtime_window = self.self_weak.upgrade().unwrap();
        crate::fonts::text_size(
            font_request.merge(&self.self_weak.upgrade().unwrap().default_font_properties()),
            text,
            max_width,
            crate::ScaleFactor::new(runtime_window.scale_factor()),
        )
        .to_untyped()
    }

    fn text_input_byte_offset_for_position(
        &self,
        _text_input: std::pin::Pin<&i_slint_core::items::TextInput>,
        _pos: i_slint_core::graphics::Point,
    ) -> usize {
        todo!()
    }

    fn text_input_position_for_byte_offset(
        &self,
        _text_input: std::pin::Pin<&i_slint_core::items::TextInput>,
        _byte_offset: usize,
    ) -> i_slint_core::graphics::Point {
        todo!()
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

impl WinitWindow for SimulatorWindow {
    fn runtime_window(&self) -> Rc<i_slint_core::window::Window> {
        self.self_weak.upgrade().unwrap()
    }

    fn currently_pressed_key_code(&self) -> &Cell<Option<winit::event::VirtualKeyCode>> {
        &self.currently_pressed_key_code
    }

    fn current_keyboard_modifiers(&self) -> &Cell<KeyboardModifiers> {
        &self.keyboard_modifiers
    }

    fn draw(self: Rc<Self>) {
        let runtime_window = self.self_weak.upgrade().unwrap();

        let size = self.opengl_context.window().inner_size();

        self.opengl_context.with_current_context(|opengl_context| {
            opengl_context.ensure_resized();

            {
                let mut canvas = self.canvas.borrow_mut();
                // We pass 1.0 as dpi / device pixel ratio as femtovg only uses this factor to scale
                // text metrics. Since we do the entire translation from logical pixels to physical
                // pixels on our end, we don't need femtovg to scale a second time.
                canvas.set_size(size.width, size.height, 1.0);
            }

            let mut display: SimulatorDisplay<embedded_graphics::pixelcolor::Rgb888> =
                SimulatorDisplay::new(Size { width: size.width, height: size.height });

            let background =
                crate::renderer::to_rgb888_color_discard_alpha(self.background_color.get());
            display.clear(background).unwrap();

            super::PARTIAL_RENDERING_CACHE.with(|cache| {
                crate::renderer::render_window_frame(
                    runtime_window,
                    background,
                    &mut display,
                    &mut cache.borrow_mut(),
                );
            });

            let output_image = display
                .to_rgb_output_image(&embedded_graphics_simulator::OutputSettings::default());
            let image_buffer = output_image.as_image_buffer();
            let image_ref: imgref::ImgRef<rgb::RGB8> = imgref::ImgRef::new(
                image_buffer.as_rgb(),
                image_buffer.width() as usize,
                image_buffer.height() as usize,
            )
            .into();

            let mut canvas = self.canvas.borrow_mut();
            let image_id = canvas.create_image(image_ref, femtovg::ImageFlags::empty()).unwrap();

            let mut path = femtovg::Path::new();
            path.rect(0., 0., image_ref.width() as _, image_ref.height() as _);

            let fill_paint = femtovg::Paint::image(
                image_id,
                0.,
                0.,
                image_ref.width() as _,
                image_ref.height() as _,
                0.0,
                1.0,
            );

            canvas.fill_path(&mut path, fill_paint);

            canvas.flush();
            canvas.delete_image(image_id);

            opengl_context.swap_buffers();
        });
    }

    fn with_window_handle(&self, callback: &mut dyn FnMut(&winit::window::Window)) {
        callback(&*self.opengl_context.window())
    }

    fn constraints(&self) -> (i_slint_core::layout::LayoutInfo, i_slint_core::layout::LayoutInfo) {
        self.constraints.get()
    }
    fn set_constraints(
        &self,
        constraints: (i_slint_core::layout::LayoutInfo, i_slint_core::layout::LayoutInfo),
    ) {
        self.constraints.set(constraints)
    }

    fn set_background_color(&self, color: Color) {
        self.background_color.set(color);
    }
    fn set_icon(&self, _icon: i_slint_core::graphics::Image) {}
}

pub struct SimulatorBackend;

impl i_slint_core::backend::Backend for SimulatorBackend {
    fn create_window(&'static self) -> Rc<Window> {
        i_slint_core::window::Window::new(|window| SimulatorWindow::new(window))
    }

    fn run_event_loop(&'static self, behavior: i_slint_core::backend::EventLoopQuitBehavior) {
        event_loop::run(behavior);
    }

    fn quit_event_loop(&'static self) {
        self::event_loop::with_window_target(|event_loop| {
            event_loop.event_loop_proxy().send_event(self::event_loop::CustomEvent::Exit).ok();
        })
    }

    fn register_font_from_memory(
        &'static self,
        _data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        //TODO
        Err("Not implemented".into())
    }

    fn register_font_from_path(
        &'static self,
        _path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        unimplemented!()
    }

    fn register_bitmap_font(&'static self, font_data: &'static i_slint_core::graphics::BitmapFont) {
        crate::fonts::register_bitmap_font(font_data);
    }

    fn set_clipboard_text(&'static self, _text: String) {
        unimplemented!()
    }

    fn clipboard_text(&'static self) -> Option<String> {
        unimplemented!()
    }

    fn post_event(&'static self, event: Box<dyn FnOnce() + Send>) {
        self::event_loop::GLOBAL_PROXY
            .get_or_init(Default::default)
            .lock()
            .unwrap()
            .send_event(self::event_loop::CustomEvent::UserEvent(event));
    }

    fn image_size(&'static self, image: &Image) -> i_slint_core::graphics::IntSize {
        let inner: &ImageInner = image.into();
        match inner {
            ImageInner::None => Default::default(),
            ImageInner::AbsoluteFilePath(_) | ImageInner::EmbeddedData { .. } => unimplemented!(),
            ImageInner::EmbeddedImage(buffer) => buffer.size(),
            ImageInner::StaticTextures(StaticTextures { original_size, .. }) => *original_size,
        }
    }
}
