// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::PhysicalSize;
use i_slint_core::graphics::euclid::{Point2D, Size2D};
use i_slint_core::graphics::FontRequest;
use i_slint_core::lengths::{LogicalLength, LogicalPoint, LogicalRect, LogicalSize, ScaleFactor};
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::{Renderer, RendererSealed};
use i_slint_core::window::{InputMethodRequest, WindowAdapter, WindowAdapterInternal};

use i_slint_core::items::TextWrap;
use std::cell::{Cell, RefCell};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Mutex;

#[derive(Default)]
pub struct TestingBackendOptions {
    pub mock_time: bool,
    pub threading: bool,
}

pub struct TestingBackend {
    clipboard: Mutex<Option<String>>,
    queue: Option<Queue>,
    mock_time: bool,
}

impl TestingBackend {
    pub fn new(options: TestingBackendOptions) -> Self {
        Self {
            clipboard: Mutex::default(),
            queue: options.threading.then(|| Queue(Default::default(), std::thread::current())),
            mock_time: options.mock_time,
        }
    }
}

impl i_slint_core::platform::Platform for TestingBackend {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn WindowAdapter>, i_slint_core::platform::PlatformError> {
        Ok(Rc::new_cyclic(|self_weak| TestingWindow {
            window: i_slint_core::api::Window::new(self_weak.clone() as _),
            size: Default::default(),
            ime_requests: Default::default(),
            mouse_cursor: Default::default(),
        }))
    }

    fn duration_since_start(&self) -> core::time::Duration {
        if self.mock_time {
            // The slint::testing::mock_elapsed_time updates the animation tick directly
            core::time::Duration::from_millis(i_slint_core::animations::current_tick().0)
        } else {
            static INITIAL_INSTANT: std::sync::OnceLock<std::time::Instant> =
                std::sync::OnceLock::new();
            let the_beginning = *INITIAL_INSTANT.get_or_init(std::time::Instant::now);
            std::time::Instant::now() - the_beginning
        }
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
            if !self.mock_time {
                i_slint_core::platform::update_timers_and_animations();
            }
            match e {
                Some(Event::Quit) => break Ok(()),
                Some(Event::Event(e)) => e(),
                None => match i_slint_core::platform::duration_until_next_timer_update() {
                    Some(duration) if !self.mock_time => std::thread::park_timeout(duration),
                    _ => std::thread::park(),
                },
            }
        }
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn i_slint_core::platform::EventLoopProxy>> {
        self.queue
            .as_ref()
            .map(|q| Box::new(q.clone()) as Box<dyn i_slint_core::platform::EventLoopProxy>)
    }
}

pub struct TestingWindow {
    window: i_slint_core::api::Window,
    size: Cell<PhysicalSize>,
    pub ime_requests: RefCell<Vec<InputMethodRequest>>,
    pub mouse_cursor: Cell<i_slint_core::items::MouseCursor>,
}

impl WindowAdapterInternal for TestingWindow {
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

impl WindowAdapter for TestingWindow {
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
            size: size.to_logical(1.),
        });
        self.size.set(size.to_physical(1.))
    }

    fn renderer(&self) -> &dyn Renderer {
        self
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

impl RendererSealed for TestingWindow {
    fn text_size(
        &self,
        _font_request: i_slint_core::graphics::FontRequest,
        text: &str,
        _max_width: Option<LogicalLength>,
        _scale_factor: ScaleFactor,
        _text_wrap: TextWrap,
    ) -> LogicalSize {
        LogicalSize::new(text.len() as f32 * 10., 10.)
    }

    fn font_metrics(
        &self,
        _font_request: i_slint_core::graphics::FontRequest,
        _scale_factor: ScaleFactor,
    ) -> i_slint_core::items::FontMetrics {
        i_slint_core::items::FontMetrics { ascent: 7., descent: 3., x_height: 3., cap_height: 7. }
    }

    // this works only for single line text
    fn text_input_byte_offset_for_position(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        pos: LogicalPoint,
        _font_request: FontRequest,
        _scale_factor: ScaleFactor,
    ) -> usize {
        let text_len = text_input.text().len();
        let result = pos.x / 10.;
        result.min(text_len as f32).max(0.) as usize
    }

    // this works only for single line text
    fn text_input_cursor_rect_for_byte_offset(
        &self,
        _text_input: Pin<&i_slint_core::items::TextInput>,
        byte_offset: usize,
        _font_request: FontRequest,
        _scale_factor: ScaleFactor,
    ) -> LogicalRect {
        LogicalRect::new(Point2D::new(byte_offset as f32 * 10., 0.), Size2D::new(1., 10.))
    }

    fn register_font_from_memory(
        &self,
        _data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn register_font_from_path(
        &self,
        _path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn default_font_size(&self) -> LogicalLength {
        LogicalLength::new(10.)
    }

    fn set_window_adapter(&self, _window_adapter: &Rc<dyn WindowAdapter>) {
        // No-op since TestingWindow is also the WindowAdapter
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
