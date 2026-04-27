// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! # Custom Event Loop with ChannelEventLoopProxy
//!
//! Demonstrates how to integrate a Slint UI into a custom winit event loop
//! using [`slint::platform::channel_event_loop_proxy`].
//!
//! ## The problem
//!
//! When you drive the event loop yourself (instead of calling `slint::run_event_loop()`),
//! Slint needs a way to wake your loop when work is queued through
//! [`slint::invoke_from_event_loop()`] or [`slint::quit_event_loop()`]. Without a wakeup
//! mechanism, callbacks from worker threads are delayed until the next user input.
//!
//! ## The solution: ChannelEventLoopProxy
//!
//! ```text
//! channel_event_loop_proxy(wakeup_fn) -> (ChannelEventLoopProxy, ChannelEventLoopReceiver)
//! ```
//!
//! - Pass the **proxy** to your custom platform's `new_event_loop_proxy()`.
//!   Slint uses it to post events that need to be processed.
//! - The **wakeup_fn** is called whenever Slint wants to unblock your loop
//!   after queueing work. Here we send a winit user event.
//! - In `about_to_wait`, call `receiver.drain()` to run pending Slint work.

use slint::platform::{
    ChannelEventLoopProxy, ChannelEventLoopReceiver, EventLoopProxy, Platform, WindowAdapter,
    WindowEvent,
    software_renderer::{MinimalSoftwareWindow, RepaintBufferType, SoftwareRenderer},
};
use slint::{ComponentHandle, LogicalPosition, PhysicalSize, SharedString, WindowSize};
use softbuffer::Surface;
use std::{ops::ControlFlow, rc::Rc, sync::Arc, time::Duration};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, WindowEvent as WinitWindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window as WinitWindow, WindowId},
};

slint::include_modules!();

// --- Custom platform ---------------------------------------------------------

struct CustomPlatform {
    proxy: ChannelEventLoopProxy,
    window: Rc<MinimalSoftwareWindow>,
}

impl Platform for CustomPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        Some(Box::new(self.proxy.clone()))
    }

    fn duration_since_start(&self) -> core::time::Duration {
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        START.get_or_init(std::time::Instant::now).elapsed()
    }
}

// --- Application state -------------------------------------------------------

struct App {
    window: Option<Arc<WinitWindow>>,
    surface: Option<Surface<Arc<WinitWindow>, Arc<WinitWindow>>>,
    slint_window: Rc<MinimalSoftwareWindow>,
    slint_app: Option<AppUi>,
    cursor_pos: Option<LogicalPosition>,
    slint_receiver: ChannelEventLoopReceiver,
}

impl App {
    fn new(proxy: ChannelEventLoopProxy, receiver: ChannelEventLoopReceiver) -> Self {
        let slint_window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);

        slint::platform::set_platform(Box::new(CustomPlatform {
            proxy,
            window: slint_window.clone(),
        }))
        .expect("platform already set");

        let slint_app = AppUi::new().expect("failed to create UI");
        configure_callbacks(&slint_app);
        slint_app.window().show().expect("failed to show window");

        Self {
            window: None,
            surface: None,
            slint_window,
            slint_app: Some(slint_app),
            cursor_pos: None,
            slint_receiver: receiver,
        }
    }

    fn dispatch(&self, event: WindowEvent) {
        if let Some(app) = &self.slint_app {
            app.window().dispatch_event(event);
        }
    }

    fn render(&mut self) {
        let (Some(window), Some(surface)) = (&self.window, &mut self.surface) else { return };
        self.slint_window.draw_if_needed(|renderer| {
            blit(renderer, surface, window);
        });
    }
}

fn configure_callbacks(app: &AppUi) {
    app.on_start_worker({
        let app_weak = app.as_weak();
        move || {
            let Some(app) = app_weak.upgrade() else { return };
            app.set_worker_running(true);
            app.set_worker_progress(0);
            app.set_worker_status(SharedString::from("Worker started"));

            std::thread::spawn({
                let app_weak = app_weak.clone();
                move || {
                    for step in 1..=5 {
                        std::thread::sleep(Duration::from_millis(300));
                        let progress = step * 20;
                        let app_weak = app_weak.clone();
                        slint::invoke_from_event_loop(move || {
                            if let Some(app) = app_weak.upgrade() {
                                app.set_worker_progress(progress);
                                app.set_worker_status(SharedString::from(format!(
                                    "Worker progress: {progress}%"
                                )));
                                if progress == 100 {
                                    app.set_worker_running(false);
                                    app.set_worker_status(SharedString::from(
                                        "Worker finished on the UI thread",
                                    ));
                                }
                            }
                        })
                        .ok();
                    }
                }
            });
        }
    });

    app.on_quit_from_worker(move || {
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            slint::quit_event_loop().ok();
        });
    });
}

// --- winit ApplicationHandler ------------------------------------------------

impl ApplicationHandler<()> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = winit::window::WindowAttributes::default()
            .with_title("Custom Event Loop — ChannelEventLoopProxy")
            .with_inner_size(winit::dpi::LogicalSize::new(640.0, 480.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("failed to create window"));

        let ctx = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        let surface = softbuffer::Surface::new(&ctx, window.clone()).expect("softbuffer surface");

        let size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;
        self.dispatch(WindowEvent::ScaleFactorChanged { scale_factor });
        self.slint_window
            .set_size(WindowSize::Physical(PhysicalSize::new(size.width, size.height)));
        self.dispatch(WindowEvent::WindowActiveChanged(true));

        self.surface = Some(surface);
        self.window = Some(window);
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: ()) {
        // Slint sent a wakeup via the proxy; request a redraw so about_to_wait runs promptly.
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.slint_receiver.drain() == ControlFlow::Break(()) {
            event_loop.exit();
            return;
        }
        self.render();
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WinitWindowEvent) {
        match event {
            WinitWindowEvent::CloseRequested => event_loop.exit(),
            WinitWindowEvent::Resized(size) => {
                self.slint_window
                    .set_size(WindowSize::Physical(PhysicalSize::new(size.width, size.height)));
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WinitWindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let size = self.window.as_ref().map(|w| w.inner_size()).unwrap_or_default();
                self.slint_window
                    .set_size(WindowSize::Physical(PhysicalSize::new(size.width, size.height)));
                self.dispatch(WindowEvent::ScaleFactorChanged {
                    scale_factor: scale_factor as f32,
                });
            }
            WinitWindowEvent::RedrawRequested => {
                self.render();
            }
            WinitWindowEvent::CursorMoved { position, .. } => {
                let scale = self.window.as_ref().map(|w| w.scale_factor()).unwrap_or(1.0) as f32;
                let pos =
                    LogicalPosition::new(position.x as f32 / scale, position.y as f32 / scale);
                self.cursor_pos = Some(pos);
                self.dispatch(WindowEvent::PointerMoved { position: pos });
            }
            WinitWindowEvent::CursorLeft { .. } => {
                self.cursor_pos = None;
                self.dispatch(WindowEvent::PointerExited);
            }
            WinitWindowEvent::MouseInput { state, button, .. } => {
                if let Some(pos) = self.cursor_pos {
                    let btn = match button {
                        MouseButton::Left => slint::platform::PointerEventButton::Left,
                        MouseButton::Right => slint::platform::PointerEventButton::Right,
                        _ => slint::platform::PointerEventButton::Other,
                    };
                    self.dispatch(match state {
                        ElementState::Pressed => {
                            WindowEvent::PointerPressed { button: btn, position: pos }
                        }
                        ElementState::Released => {
                            WindowEvent::PointerReleased { button: btn, position: pos }
                        }
                    });
                }
            }
            _ => {}
        }
    }
}

// --- Rendering ---------------------------------------------------------------

fn blit(
    renderer: &SoftwareRenderer,
    surface: &mut Surface<Arc<WinitWindow>, Arc<WinitWindow>>,
    window: &Arc<WinitWindow>,
) {
    let size = window.inner_size();
    let (width, height) = (size.width, size.height);
    if width == 0 || height == 0 {
        return;
    }
    surface
        .resize(width.try_into().unwrap(), height.try_into().unwrap())
        .expect("softbuffer resize");

    let mut sb_buffer = surface.buffer_mut().expect("softbuffer buffer_mut");
    let mut pixels = vec![slint::Rgb8Pixel::default(); (width * height) as usize];
    renderer.render(&mut pixels, width as usize);

    for (dst, src) in sb_buffer.iter_mut().zip(pixels.iter()) {
        *dst = (src.r as u32) << 16 | (src.g as u32) << 8 | src.b as u32;
    }
    sb_buffer.present().expect("softbuffer present");
}

// --- Entry point -------------------------------------------------------------

fn main() {
    let event_loop =
        EventLoop::<()>::with_user_event().build().expect("failed to build event loop");
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

    let winit_proxy = event_loop.create_proxy();
    let (slint_proxy, slint_receiver) = slint::platform::channel_event_loop_proxy(Some(
        // Wakeup callback: unblock the winit event loop when Slint has pending work.
        Box::new(move || {
            let _ = winit_proxy.send_event(());
        }),
    ));

    let mut app = App::new(slint_proxy, slint_receiver);
    event_loop.run_app(&mut app).expect("event loop failed");
}
