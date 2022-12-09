// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![deny(unsafe_code)]

use std::cell::Cell;
use std::rc::{Rc, Weak};

use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

use slint::PhysicalSize as PhysicalWindowSize;

slint::include_modules!();

struct WinitWindowAdapter {
    slint_window: slint::Window,
    winit_window: winit::window::Window,
    skia_renderer: i_slint_renderer_skia::SkiaRenderer,
}

impl WinitWindowAdapter {
    fn new(winit_window: winit::window::Window) -> Rc<Self> {
        Rc::new_cyclic(|w: &Weak<Self>| Self {
            slint_window: slint::Window::new(w.clone()),
            winit_window,
            skia_renderer: i_slint_renderer_skia::SkiaRenderer::new(w.clone()),
        })
    }

    fn window_event(&self, event: WindowEvent) {
        match event {
            WindowEvent::Resized(new_size) => {
                let slint_size = PhysicalWindowSize::new(new_size.width, new_size.height);
                self.skia_renderer.resize_event(slint_size);
                self.slint_window
                    .set_size(slint::PhysicalSize::new(new_size.width, new_size.height));
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.slint_window.set_scale_factor(scale_factor as _)
            }
            _ => {}
        }
    }

    fn render(&self) {
        let winit_size: winit::dpi::PhysicalSize<u32> = self.winit_window.inner_size();
        let slint_size = PhysicalWindowSize::new(winit_size.width, winit_size.height);
        self.skia_renderer.render(slint_size);
    }
}

impl i_slint_core::window::WindowAdapterSealed for WinitWindowAdapter {
    fn renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.skia_renderer
    }

    fn show(&self) {
        self.slint_window.set_scale_factor(self.winit_window.scale_factor() as _);
        let winit_size: winit::dpi::PhysicalSize<u32> = self.winit_window.inner_size();
        let slint_size = PhysicalWindowSize::new(winit_size.width, winit_size.height);
        self.skia_renderer.show(&self.winit_window, &self.winit_window, slint_size);
    }

    fn hide(&self) {
        self.skia_renderer.hide();
    }

    fn set_size(&self, size: slint::WindowSize) {
        self.winit_window.set_inner_size(match size {
            slint::WindowSize::Logical(size) => {
                winit::dpi::Size::new(winit::dpi::LogicalSize::new(size.width, size.height))
            }
            slint::WindowSize::Physical(size) => {
                winit::dpi::Size::new(winit::dpi::PhysicalSize::new(size.width, size.height))
            }
        })
    }
}

impl slint::platform::WindowAdapter for WinitWindowAdapter {
    fn window(&self) -> &slint::Window {
        &self.slint_window
    }
}

struct WinitPlatform {
    start_time: std::time::Instant,
    window_adapter: Cell<Option<Rc<WinitWindowAdapter>>>,
}

impl WinitPlatform {
    fn new(window_adapter: &Rc<WinitWindowAdapter>) -> Self {
        Self {
            start_time: std::time::Instant::now(),
            window_adapter: Cell::new(Some(window_adapter.clone())),
        }
    }
}

impl slint::platform::Platform for WinitPlatform {
    fn create_window_adapter(&self) -> Rc<dyn slint::platform::WindowAdapter> {
        self.window_adapter.take().unwrap()
    }

    fn duration_since_start(&self) -> core::time::Duration {
        std::time::Instant::now().duration_since(self.start_time)
    }
}

pub fn main() {
    let event_loop = EventLoop::new();

    let winit_window = WindowBuilder::new().build(&event_loop).unwrap();
    let window_adapter = WinitWindowAdapter::new(winit_window);

    slint::platform::set_platform(Box::new(WinitPlatform::new(&window_adapter))).unwrap();

    let app = App::new();
    app.show();

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, window_id }
                if window_id == window_adapter.winit_window.id() =>
            {
                control_flow.set_exit()
            }
            Event::MainEventsCleared => {}
            Event::WindowEvent { event, window_id }
                if window_id == window_adapter.winit_window.id() =>
            {
                window_adapter.window_event(event)
            }
            Event::RedrawRequested(window_id) if window_id == window_adapter.winit_window.id() => {
                window_adapter.render();
            }
            _ => (),
        }
    });
}
