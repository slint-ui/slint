// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore borderless glcontext

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use embedded_graphics::prelude::*;
use embedded_graphics_simulator::SimulatorDisplay;
use i_slint_core::api::{euclid, PhysicalPx};
use i_slint_core::component::ComponentRc;
use i_slint_core::input::KeyboardModifiers;
use i_slint_core::layout::Orientation;
use i_slint_core::window::{PlatformWindow, WindowRc};
use i_slint_core::Coord;
use rgb::FromSlice;

use self::event_loop::WinitWindow;

type Canvas = femtovg::Canvas<femtovg::renderer::OpenGl>;
type CanvasRc = Rc<RefCell<Canvas>>;

pub mod event_loop;
mod glcontext;
use glcontext::*;
use i_slint_core::swrenderer::SoftwareRenderer;

pub struct SimulatorWindow {
    window_inner_weak: Weak<i_slint_core::window::WindowInner>,
    self_weak: Weak<Self>,
    keyboard_modifiers: std::cell::Cell<KeyboardModifiers>,
    currently_pressed_key_code: std::cell::Cell<Option<winit::event::VirtualKeyCode>>,
    canvas: CanvasRc,
    opengl_context: OpenGLContext,
    constraints: Cell<(i_slint_core::layout::LayoutInfo, i_slint_core::layout::LayoutInfo)>,
    visible: Cell<bool>,
    frame_buffer: RefCell<Option<SimulatorDisplay<embedded_graphics::pixelcolor::Rgb888>>>,
    renderer: SoftwareRenderer,
}

impl SimulatorWindow {
    pub(crate) fn new(window_weak: &Weak<i_slint_core::window::WindowInner>) -> Rc<Self> {
        let window_builder = winit::window::WindowBuilder::new().with_visible(false);

        let opengl_context = OpenGLContext::new_context(window_builder);

        let renderer =
            femtovg::renderer::OpenGl::new_from_glutin_context(&opengl_context.glutin_context())
                .unwrap();

        let canvas = femtovg::Canvas::new(renderer).unwrap();

        opengl_context.make_not_current();

        let canvas = Rc::new(RefCell::new(canvas));

        let window_rc = Rc::new_cyclic(|self_weak| Self {
            window_inner_weak: window_weak.clone(),
            self_weak: self_weak.clone(),
            keyboard_modifiers: Default::default(),
            currently_pressed_key_code: Default::default(),
            canvas,
            opengl_context,
            constraints: Default::default(),
            visible: Default::default(),
            frame_buffer: RefCell::default(),
            renderer: Default::default(),
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
    fn show(&self) {
        if self.visible.get() {
            return;
        }

        self.visible.set(true);

        let runtime_window = self.window();
        let component_rc = runtime_window.component();
        let component = ComponentRc::borrow_pin(&component_rc);

        let platform_window = self.opengl_context.window();

        if let Some(window_item) = runtime_window.window_item() {
            platform_window.set_title(&window_item.as_pin_ref().title());
            platform_window.set_decorations(!window_item.as_pin_ref().no_frame());
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
            if s.width > 0 as Coord && s.height > 0 as Coord {
                // Make sure that the window's inner size is in sync with the root window item's
                // width/height.
                runtime_window.set_window_item_geometry(s.width, s.height);
                platform_window.set_inner_size(s)
            }
        };

        platform_window.set_visible(true);
        let id = platform_window.id();
        drop(platform_window);
        crate::event_loop::register_window(id, self.self_weak.upgrade().unwrap());
    }

    fn hide(&self) {
        self.opengl_context.window().set_visible(false);
        self.visible.set(false);
        crate::event_loop::unregister_window(self.opengl_context.window().id());
    }

    fn request_redraw(&self) {
        if self.visible.get() {
            self.opengl_context.window().request_redraw();
        }
    }

    fn renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
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

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn position(&self) -> euclid::Point2D<i32, PhysicalPx> {
        unimplemented!()
    }

    fn set_position(&self, _position: euclid::Point2D<i32, PhysicalPx>) {
        unimplemented!()
    }

    fn window(&self) -> WindowRc {
        self.window_inner_weak.upgrade().unwrap()
    }
}

impl WinitWindow for SimulatorWindow {
    fn currently_pressed_key_code(&self) -> &Cell<Option<winit::event::VirtualKeyCode>> {
        &self.currently_pressed_key_code
    }

    fn current_keyboard_modifiers(&self) -> &Cell<KeyboardModifiers> {
        &self.keyboard_modifiers
    }

    fn draw(&self) {
        let runtime_window = self.window_inner_weak.upgrade().unwrap();

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

            let mut frame_buffer = self.frame_buffer.borrow_mut();
            let display = match frame_buffer.as_mut() {
                Some(buffer)
                    if buffer.size().width == size.width && buffer.size().height == size.height =>
                {
                    buffer
                }
                _ => frame_buffer
                    .insert(SimulatorDisplay::new(Size { width: size.width, height: size.height })),
            };

            struct BufferProvider<'a> {
                devices: &'a mut dyn crate::Devices,
            }
            impl crate::renderer::LineBufferProvider for BufferProvider<'_> {
                type TargetPixel = crate::TargetPixel;

                fn process_line(
                    &mut self,
                    line: crate::PhysicalLength,
                    range: core::ops::Range<i16>,
                    render_fn: impl FnOnce(&mut [super::TargetPixel]),
                ) {
                    let mut render_fn = Some(render_fn);
                    self.devices.render_line(line, range, &mut |buffer| {
                        (render_fn.take().unwrap())(buffer)
                    });
                }
            }
            self.renderer
                .render_by_line(&runtime_window.into(), BufferProvider { devices: display });

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

    fn set_icon(&self, _icon: i_slint_core::graphics::Image) {}

    fn inner_size(&self) -> euclid::Size2D<u32, PhysicalPx> {
        let winit_window = self.opengl_context.window();
        let size = winit_window.inner_size();
        euclid::Size2D::new(size.width, size.height)
    }
}

pub struct SimulatorBackend;

impl i_slint_core::backend::Backend for SimulatorBackend {
    fn create_window(&self) -> i_slint_core::api::Window {
        i_slint_core::window::WindowInner::new(|window| SimulatorWindow::new(window)).into()
    }

    fn run_event_loop(&self, behavior: i_slint_core::backend::EventLoopQuitBehavior) {
        event_loop::run(behavior);
        std::process::exit(0);
    }

    fn quit_event_loop(&self) {
        self::event_loop::with_window_target(|event_loop| {
            event_loop.event_loop_proxy().send_event(self::event_loop::CustomEvent::Exit).ok();
        })
    }

    fn post_event(&self, event: Box<dyn FnOnce() + Send>) {
        self::event_loop::GLOBAL_PROXY
            .get_or_init(Default::default)
            .lock()
            .unwrap()
            .send_event(self::event_loop::CustomEvent::UserEvent(event));
    }
}
