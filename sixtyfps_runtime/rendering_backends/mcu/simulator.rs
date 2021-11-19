/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;
use embedded_graphics_simulator::SimulatorDisplay;
use rgb::FromSlice;
use sixtyfps_corelib::component::ComponentRc;
use sixtyfps_corelib::input::KeyboardModifiers;
use sixtyfps_corelib::items::ItemRef;
use sixtyfps_corelib::layout::Orientation;
use sixtyfps_corelib::window::PlatformWindow;
use sixtyfps_corelib::Color;

use self::event_loop::WinitWindow;

type Canvas = femtovg::Canvas<femtovg::renderer::OpenGl>;
type CanvasRc = Rc<RefCell<Canvas>>;

pub mod event_loop;
mod glcontext;
use glcontext::*;

pub struct SimulatorWindow {
    self_weak: Weak<sixtyfps_corelib::window::Window>,
    keyboard_modifiers: std::cell::Cell<KeyboardModifiers>,
    currently_pressed_key_code: std::cell::Cell<Option<winit::event::VirtualKeyCode>>,
    canvas: CanvasRc,
    opengl_context: OpenGLContext,
    constraints: Cell<(sixtyfps_corelib::layout::LayoutInfo, sixtyfps_corelib::layout::LayoutInfo)>,
    visible: Cell<bool>,
    background_color: Cell<Color>,
}

impl SimulatorWindow {
    pub(crate) fn new(window_weak: &Weak<sixtyfps_corelib::window::Window>) -> Rc<Self> {
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
            ItemRef::downcast_pin::<sixtyfps_corelib::items::WindowItem>(root_item)
        {
            platform_window.set_title(&window_item.title());
            platform_window.set_decorations(!window_item.no_frame());
        };

        if std::env::var("SIXTYFPS_FULLSCREEN").is_ok() {
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
        _items: &mut dyn Iterator<Item = std::pin::Pin<sixtyfps_corelib::items::ItemRef<'a>>>,
    ) {
        // Nothing to do until we start caching stuff that needs freeing
    }

    fn show_popup(
        &self,
        _popup: &sixtyfps_corelib::component::ComponentRc,
        _position: sixtyfps_corelib::graphics::Point,
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
        window_item: std::pin::Pin<&sixtyfps_corelib::items::WindowItem>,
    ) {
        WinitWindow::apply_window_properties(self as &dyn WinitWindow, window_item);
    }

    fn apply_geometry_constraint(
        &self,
        constraints_horizontal: sixtyfps_corelib::layout::LayoutInfo,
        constraints_vertical: sixtyfps_corelib::layout::LayoutInfo,
    ) {
        self.apply_constraints(constraints_horizontal, constraints_vertical)
    }

    fn text_size(
        &self,
        _font_request: sixtyfps_corelib::graphics::FontRequest,
        _text: &str,
        _max_width: Option<f32>,
    ) -> sixtyfps_corelib::graphics::Size {
        // TODO
        Default::default()
    }

    fn text_input_byte_offset_for_position(
        &self,
        _text_input: std::pin::Pin<&sixtyfps_corelib::items::TextInput>,
        _pos: sixtyfps_corelib::graphics::Point,
    ) -> usize {
        todo!()
    }

    fn text_input_position_for_byte_offset(
        &self,
        _text_input: std::pin::Pin<&sixtyfps_corelib::items::TextInput>,
        _byte_offset: usize,
    ) -> sixtyfps_corelib::graphics::Point {
        todo!()
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

impl WinitWindow for SimulatorWindow {
    fn runtime_window(&self) -> Rc<sixtyfps_corelib::window::Window> {
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

        self.opengl_context.with_current_context(|| {
            self.opengl_context.ensure_resized();

            {
                let mut canvas = self.canvas.borrow_mut();
                // We pass 1.0 as dpi / device pixel ratio as femtovg only uses this factor to scale
                // text metrics. Since we do the entire translation from logical pixels to physical
                // pixels on our end, we don't need femtovg to scale a second time.
                canvas.set_size(size.width, size.height, 1.0);
            }

            let mut display: SimulatorDisplay<embedded_graphics::pixelcolor::Rgb888> =
                SimulatorDisplay::new(Size { width: size.width, height: size.height });

            display.clear(to_rgb888_color_discard_alpha(self.background_color.get())).unwrap();

            // Debug
            {
                use embedded_graphics::{
                    prelude::*,
                    primitives::{PrimitiveStyleBuilder, Rectangle},
                };

                let style = PrimitiveStyleBuilder::new()
                    .stroke_color(Rgb888::RED)
                    .stroke_width(3)
                    .fill_color(Rgb888::GREEN)
                    .build();

                Rectangle::new(Point::new(30, 20), Size::new(10, 15))
                    .into_styled(style)
                    .draw(&mut display)
                    .unwrap();
            }

            crate::renderer::render_window_frame(runtime_window, &mut display);

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

            self.opengl_context.swap_buffers();
        });
    }

    fn with_window_handle(&self, callback: &mut dyn FnMut(&winit::window::Window)) {
        callback(&*self.opengl_context.window())
    }

    fn constraints(
        &self,
    ) -> (sixtyfps_corelib::layout::LayoutInfo, sixtyfps_corelib::layout::LayoutInfo) {
        self.constraints.get()
    }
    fn set_constraints(
        &self,
        constraints: (sixtyfps_corelib::layout::LayoutInfo, sixtyfps_corelib::layout::LayoutInfo),
    ) {
        self.constraints.set(constraints)
    }

    fn set_background_color(&self, color: Color) {
        self.background_color.set(color);
    }
    fn set_icon(&self, _icon: sixtyfps_corelib::graphics::Image) {}
}

fn to_rgb888_color_discard_alpha(col: Color) -> Rgb888 {
    Rgb888::new(col.red(), col.green(), col.blue())
}
