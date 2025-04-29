// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::PhysicalSize;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::Renderer;
use i_slint_core::window::{InputMethodRequest, WindowAdapter, WindowAdapterInternal};
use i_slint_renderer_skia::{SkiaRenderer, SkiaSharedContext};

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Mutex;

pub struct HeadlessBackend {
    clipboard: Mutex<Option<String>>,
    queue: Option<Queue>,
}

impl HeadlessBackend {
    pub fn new() -> Self {
        eprintln!("Running headless!");
        Self {
            clipboard: Mutex::default(),
            queue: Some(Queue(Default::default(), std::thread::current())),
        }
    }
}

impl Default for HeadlessBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl i_slint_core::platform::Platform for HeadlessBackend {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn WindowAdapter>, i_slint_core::platform::PlatformError> {
        Ok(Rc::new_cyclic(|self_weak| HeadlessWindow {
            window: i_slint_core::api::Window::new(self_weak.clone() as _),
            size: Default::default(),
            ime_requests: Default::default(),
            mouse_cursor: Default::default(),
            renderer: SkiaRenderer::default_software(&SkiaSharedContext::default()),
        }))
    }

    fn duration_since_start(&self) -> core::time::Duration {
        static INITIAL_INSTANT: std::sync::OnceLock<std::time::Instant> =
            std::sync::OnceLock::new();
        let the_beginning = *INITIAL_INSTANT.get_or_init(std::time::Instant::now);
        std::time::Instant::now() - the_beginning
    }

    fn set_clipboard_text(&self, text: &str, clipboard: i_slint_core::platform::Clipboard) {
        if clipboard == i_slint_core::platform::Clipboard::DefaultClipboard {
            *self.clipboard.lock().unwrap() = Some(text.into());
        }
    }

    fn clipboard_text(&self, clipboard: i_slint_core::platform::Clipboard) -> Option<String> {
        if clipboard == i_slint_core::platform::Clipboard::DefaultClipboard {
            self.clipboard.lock().unwrap().clone()
        } else {
            None
        }
    }

    fn run_event_loop(&self) -> Result<(), PlatformError> {
        let queue = match self.queue.as_ref() {
            Some(queue) => queue.clone(),
            None => return Err(PlatformError::NoEventLoopProvider),
        };

        loop {
            let e = queue.0.lock().unwrap().pop_front();
            i_slint_core::platform::update_timers_and_animations();
            match e {
                Some(Event::Quit) => break Ok(()),
                Some(Event::Event(e)) => e(),
                None => {
                    i_slint_core::platform::duration_until_next_timer_update();
                    std::thread::park();
                }
            }
        }
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn i_slint_core::platform::EventLoopProxy>> {
        self.queue
            .as_ref()
            .map(|q| Box::new(q.clone()) as Box<dyn i_slint_core::platform::EventLoopProxy>)
    }
}

pub struct HeadlessWindow {
    window: i_slint_core::api::Window,
    size: Cell<PhysicalSize>,
    pub ime_requests: RefCell<Vec<InputMethodRequest>>,
    pub mouse_cursor: Cell<i_slint_core::items::MouseCursor>,
    renderer: SkiaRenderer,
}

impl WindowAdapterInternal for HeadlessWindow {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn input_method_request(&self, request: i_slint_core::window::InputMethodRequest) {
        self.ime_requests.borrow_mut().push(request)
    }

    fn set_mouse_cursor(&self, cursor: i_slint_core::items::MouseCursor) {
        self.mouse_cursor.set(cursor);
    }
}

impl WindowAdapter for HeadlessWindow {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        if self.size.get().width == 0 {
            PhysicalSize::new(800, 600)
        } else {
            self.size.get()
        }
    }

    fn set_size(&self, size: i_slint_core::api::WindowSize) {
        self.window.dispatch_event(i_slint_core::platform::WindowEvent::Resized {
            size: size.to_logical(self.window().scale_factor()),
        });
        self.size.set(size.to_physical(self.window().scale_factor()))
    }

    fn renderer(&self) -> &dyn Renderer {
        &self.renderer
    }

    fn update_window_properties(&self, properties: i_slint_core::window::WindowProperties<'_>) {
        if self.size.get().width == 0 {
            let c = properties.layout_constraints();
            self.size.set(c.preferred.to_physical(self.window.scale_factor()));
        }
    }

    fn internal(&self, _: i_slint_core::InternalToken) -> Option<&dyn WindowAdapterInternal> {
        Some(self)
    }
}

enum Event {
    Quit,
    Event(Box<dyn FnOnce() + Send>),
}

#[derive(Clone)]
struct Queue(
    std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<Event>>>,
    std::thread::Thread,
);

impl i_slint_core::platform::EventLoopProxy for Queue {
    fn quit_event_loop(&self) -> Result<(), i_slint_core::api::EventLoopError> {
        self.0.lock().unwrap().push_back(Event::Quit);
        self.1.unpark();
        Ok(())
    }

    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), i_slint_core::api::EventLoopError> {
        self.0.lock().unwrap().push_back(Event::Event(event));
        self.1.unpark();
        Ok(())
    }
}

pub fn init() {
    i_slint_core::platform::set_platform(Box::new(HeadlessBackend::default()))
        .expect("platform already initialized");
}

pub fn set_window_scale_factor(window: &slint_interpreter::Window, factor: f32) {
    window.dispatch_event(i_slint_core::platform::WindowEvent::ScaleFactorChanged {
        scale_factor: factor,
    });
}
