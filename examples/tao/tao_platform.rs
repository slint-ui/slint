// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    num::NonZeroU32,
    rc::{Rc, Weak},
};

use glutin::{
    context::ContextAttributesBuilder,
    display::GetGlDisplay,
    prelude::*,
    surface::{SurfaceAttributesBuilder, WindowSurface},
};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use slint::platform::femtovg_renderer;
use tao::{
    event::WindowEvent, event_loop::EventLoopWindowTarget,
    platform::run_return::EventLoopExtRunReturn,
};

struct GlutinGLInterface {
    context: glutin::context::PossiblyCurrentContext,
    surface: glutin::surface::Surface<glutin::surface::WindowSurface>,
}

unsafe impl femtovg_renderer::OpenGLInterface for GlutinGLInterface {
    fn ensure_current(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.context.make_current(&self.surface).unwrap();
        Ok(())
    }

    fn swap_buffers(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.surface.swap_buffers(&self.context).unwrap();
        Ok(())
    }

    fn resize(
        &self,
        width: std::num::NonZeroU32,
        height: std::num::NonZeroU32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.surface.resize(&self.context, width, height);
        Ok(())
    }

    fn get_proc_address(&self, name: &std::ffi::CStr) -> *const std::ffi::c_void {
        self.context.display().get_proc_address(name)
    }
}

struct TaoWindowAdapter {
    slint_window: slint::Window,
    renderer: femtovg_renderer::FemtoVGRenderer,
    tao_window: tao::window::Window,
    cursor_pos: Cell<slint::LogicalPosition>,
    mouse_pressed: Cell<bool>,
}

impl TaoWindowAdapter {
    fn new(tao_window: tao::window::Window) -> Rc<Self> {
        let gl_display = unsafe {
            glutin::display::Display::new(
                tao_window.raw_display_handle(),
                glutin::display::DisplayApiPreference::Cgl,
            )
            .unwrap()
        };

        let config_template_builder = glutin::config::ConfigTemplateBuilder::new();

        // Upstream advises to use this only on Windows.
        #[cfg(target_family = "windows")]
        let config_template_builder =
            config_template_builder.compatible_with_native_window(tao_window.raw_window_handle());

        let config_template = config_template_builder.build();

        let config = unsafe {
            gl_display
                .find_configs(config_template)
                .unwrap()
                .reduce(|accum, config| {
                    let transparency_check = config.supports_transparency().unwrap_or(false)
                        & !accum.supports_transparency().unwrap_or(false);

                    if transparency_check || config.num_samples() < accum.num_samples() {
                        config
                    } else {
                        accum
                    }
                })
                .ok_or("Unable to find suitable GL config")
                .unwrap()
        };

        let gles_context_attributes =
            ContextAttributesBuilder::new().build(Some(tao_window.raw_window_handle()));

        let context =
            unsafe { gl_display.create_context(&config, &gles_context_attributes).unwrap() };

        let size = tao_window.inner_size();

        let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            tao_window.raw_window_handle(),
            NonZeroU32::new(size.width).unwrap(),
            NonZeroU32::new(size.height).unwrap(),
        );

        let surface = unsafe { config.display().create_window_surface(&config, &attrs).unwrap() };

        let context = context.make_current(&surface).unwrap();

        let gl_interface = GlutinGLInterface { context, surface };

        let adapter = Rc::new_cyclic(|self_weak: &Weak<Self>| Self {
            slint_window: slint::Window::new(self_weak.clone()),
            renderer: femtovg_renderer::FemtoVGRenderer::new(gl_interface).unwrap(),
            tao_window,
            cursor_pos: Default::default(),
            mouse_pressed: Default::default(),
        });

        adapter.slint_window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged {
            scale_factor: adapter.tao_window.scale_factor() as _,
        });

        adapter
    }

    fn dispatch_tao_window_event(&self, event: WindowEvent<'_>) {
        match event {
            WindowEvent::Resized(new_size) => {
                let logical_size = new_size.to_logical(self.tao_window.scale_factor());
                self.slint_window.dispatch_event(slint::platform::WindowEvent::Resized {
                    size: slint::LogicalSize::new(logical_size.width, logical_size.height),
                })
            }
            WindowEvent::Focused(focused) => self
                .slint_window
                .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(focused)),
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    tao::event::MouseButton::Left => slint::platform::PointerEventButton::Left,
                    tao::event::MouseButton::Right => slint::platform::PointerEventButton::Right,
                    tao::event::MouseButton::Middle => slint::platform::PointerEventButton::Middle,
                    tao::event::MouseButton::Other(_) => slint::platform::PointerEventButton::Other,
                    _ => unimplemented!(),
                };
                let ev = match state {
                    tao::event::ElementState::Pressed => {
                        self.mouse_pressed.set(true);
                        slint::platform::WindowEvent::PointerPressed {
                            position: self.cursor_pos.get(),
                            button,
                        }
                    }
                    tao::event::ElementState::Released => {
                        self.mouse_pressed.set(false);
                        slint::platform::WindowEvent::PointerReleased {
                            position: self.cursor_pos.get(),
                            button,
                        }
                    }
                    _ => unimplemented!(),
                };
                self.slint_window.dispatch_event(ev);
            }
            WindowEvent::CursorMoved { position, .. } => {
                let position = position.to_logical(self.tao_window.scale_factor());
                self.cursor_pos.set(slint::LogicalPosition::new(position.x, position.y));
                self.slint_window.dispatch_event(slint::platform::WindowEvent::PointerMoved {
                    position: self.cursor_pos.get(),
                })
            }
            WindowEvent::CursorLeft { .. } => {
                if !self.mouse_pressed.get() {
                    self.slint_window.dispatch_event(slint::platform::WindowEvent::PointerExited);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (delta_x, delta_y) = match delta {
                    tao::event::MouseScrollDelta::LineDelta(lx, ly) => (lx * 60., ly * 60.),
                    tao::event::MouseScrollDelta::PixelDelta(d) => {
                        let d = d.to_logical(self.tao_window.scale_factor());
                        (d.x, d.y)
                    }
                    _ => unimplemented!(),
                };
                self.slint_window.dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                    position: self.cursor_pos.get(),
                    delta_x,
                    delta_y,
                });
            }
            WindowEvent::ScaleFactorChanged { scale_factor, new_inner_size } => {
                self.slint_window.dispatch_event(
                    slint::platform::WindowEvent::ScaleFactorChanged {
                        scale_factor: scale_factor as _,
                    },
                );

                let logical_size = new_inner_size.to_logical(self.tao_window.scale_factor());
                self.slint_window.dispatch_event(slint::platform::WindowEvent::Resized {
                    size: slint::LogicalSize::new(logical_size.width, logical_size.height),
                })
            }

            WindowEvent::CloseRequested => {
                self.slint_window.dispatch_event(slint::platform::WindowEvent::CloseRequested)
            }
            _ => {}
        }
    }
}

impl slint::platform::WindowAdapter for TaoWindowAdapter {
    fn window(&self) -> &slint::Window {
        &self.slint_window
    }

    fn size(&self) -> slint::PhysicalSize {
        let size = self.tao_window.inner_size();
        slint::PhysicalSize::new(size.width, size.height)
    }

    fn renderer(&self) -> &dyn slint::platform::Renderer {
        &self.renderer
    }

    fn set_visible(&self, visible: bool) -> Result<(), slint::PlatformError> {
        self.tao_window.set_visible(visible);
        Ok(())
    }

    fn position(&self) -> Option<slint::PhysicalPosition> {
        self.tao_window.inner_position().ok().map(|pos| slint::PhysicalPosition::new(pos.x, pos.y))
    }

    fn set_position(&self, _position: slint::WindowPosition) {
        todo!()
    }

    fn set_size(&self, size: slint::WindowSize) {
        let tao_size = match size {
            slint::WindowSize::Physical(size) => {
                tao::dpi::Size::Physical(tao::dpi::PhysicalSize::new(size.width, size.height))
            }
            slint::WindowSize::Logical(size) => tao::dpi::Size::Logical(
                tao::dpi::LogicalSize::new(size.width as f64, size.height as f64),
            ),
        };
        self.tao_window.set_inner_size(tao_size);
    }

    fn request_redraw(&self) {
        self.tao_window.request_redraw()
    }
}

struct SlintTask(Box<dyn FnOnce() + Send>);

scoped_tls_hkt::scoped_thread_local!(static CURRENT_WINDOW_TARGET : EventLoopWindowTarget<SlintTask>);

pub struct TaoPlatform {
    windows: RefCell<HashMap<tao::window::WindowId, Weak<TaoWindowAdapter>>>,
    event_loop: RefCell<Option<tao::event_loop::EventLoop<SlintTask>>>,
}

impl TaoPlatform {
    pub fn new() -> Self {
        Self {
            windows: Default::default(),
            event_loop: RefCell::new(Some(tao::event_loop::EventLoop::with_user_event())),
        }
    }
}

impl TaoPlatform {
    fn with_window_target<R>(&self, cb: impl FnOnce(&EventLoopWindowTarget<SlintTask>) -> R) -> R {
        if CURRENT_WINDOW_TARGET.is_set() {
            CURRENT_WINDOW_TARGET.with(|loop_target| cb(loop_target))
        } else {
            cb(self.event_loop.borrow().as_ref().unwrap())
        }
    }
}

impl slint::platform::Platform for TaoPlatform {
    fn create_window_adapter(
        &self,
    ) -> Result<std::rc::Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        let window = self.with_window_target(|target| {
            let tao_window = tao::window::WindowBuilder::new().build(target).unwrap();
            TaoWindowAdapter::new(tao_window)
        });

        self.windows.borrow_mut().insert(window.tao_window.id(), Rc::downgrade(&window));

        Ok(window)
    }

    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        let mut event_loop = self.event_loop.borrow_mut().take().unwrap();

        event_loop.run_return(|event, event_loop_target, control_flow| {
            CURRENT_WINDOW_TARGET.set(event_loop_target, || {
                match event {
                    tao::event::Event::NewEvents(_) => {}
                    tao::event::Event::WindowEvent { window_id, event, .. } => {
                        if let Some(window) =
                            self.windows.borrow().get(&window_id).and_then(|w| w.upgrade())
                        {
                            window.dispatch_tao_window_event(event)
                        }
                    }
                    tao::event::Event::UserEvent(task) => (task.0)(),
                    tao::event::Event::RedrawRequested(window_id) => {
                        if let Some(window) =
                            self.windows.borrow().get(&window_id).and_then(|w| w.upgrade())
                        {
                            window.renderer.render().unwrap()
                        }
                    }
                    tao::event::Event::RedrawEventsCleared => {}
                    tao::event::Event::LoopDestroyed => {}
                    _ => {}
                }

                let mut any_visible = false;
                self.windows.borrow_mut().retain(|_, weak_window| {
                    if let Some(window) = weak_window.upgrade() {
                        any_visible |= window.tao_window.is_visible();
                        true
                    } else {
                        false
                    }
                });
                if !any_visible {
                    *control_flow = tao::event_loop::ControlFlow::Exit;
                }
            })
        });

        *self.event_loop.borrow_mut() = Some(event_loop);

        Ok(())
    }
}
