// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::PhysicalSize;
use i_slint_core::graphics::euclid::{Point2D, Size2D};
use i_slint_core::lengths::{LogicalLength, LogicalPoint, LogicalRect, LogicalSize};
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::{Renderer, RendererSealed};
use i_slint_core::textlayout::sharedparley;
use i_slint_core::window::{InputMethodRequest, WindowAdapter, WindowAdapterInternal, WindowInner};

use i_slint_core::SharedString;
use i_slint_core::items::TextWrap;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Mutex;

const FIXED_TEST_FONT: &str = "FixedTestFont";

fn is_fixed_test_font(family: &Option<SharedString>) -> bool {
    family.as_ref().is_some_and(|f| f == FIXED_TEST_FONT)
}

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
            all_item_trees: Default::default(),
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

#[derive(Default)]
struct CheckAllItemTreesUnregistered(RefCell<HashMap<*const u8, SharedString>>);

impl Drop for CheckAllItemTreesUnregistered {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            assert!(
                self.0.borrow().is_empty(),
                "Some item trees were not unregistered: {:?}",
                self.0.borrow().values()
            );
        }
    }
}

pub struct TestingWindow {
    window: i_slint_core::api::Window,
    size: Cell<PhysicalSize>,
    pub ime_requests: RefCell<Vec<InputMethodRequest>>,
    mouse_cursor: Cell<i_slint_core::items::MouseCursor>,
    all_item_trees: CheckAllItemTreesUnregistered,
}

impl TestingWindow {
    #[allow(dead_code)] // Used by various tests
    pub fn mouse_cursor(&self) -> i_slint_core::items::MouseCursor {
        self.mouse_cursor.get()
    }
}

impl WindowAdapterInternal for TestingWindow {
    fn input_method_request(&self, request: i_slint_core::window::InputMethodRequest) {
        self.ime_requests.borrow_mut().push(request)
    }

    fn set_mouse_cursor(&self, cursor: i_slint_core::items::MouseCursor) {
        self.mouse_cursor.set(cursor);
    }

    fn register_item_tree(&self, item_tree: i_slint_core::item_tree::ItemTreeRefPin) {
        let mut debug = SharedString::new();
        item_tree.as_ref().item_element_infos(0, &mut debug);
        assert_eq!(
            self.all_item_trees.0.borrow_mut().insert(item_tree.as_ptr(), debug.clone()),
            None,
            "Item tree already registered {debug:?}"
        );
    }

    fn unregister_item_tree(
        &self,
        item_tree: i_slint_core::item_tree::ItemTreeRef,
        _items: &mut dyn Iterator<Item = Pin<i_slint_core::items::ItemRef<'_>>>,
    ) {
        self.all_item_trees.0.borrow_mut().remove(&item_tree.as_ptr());
    }
}

impl WindowAdapter for TestingWindow {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        if self.size.get().width == 0 { PhysicalSize::new(800, 600) } else { self.size.get() }
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
        text_item: Pin<&dyn i_slint_core::item_rendering::RenderString>,
        item_rc: &i_slint_core::item_tree::ItemRc,
        max_width: Option<LogicalLength>,
        text_wrap: TextWrap,
    ) -> LogicalSize {
        let font_request = text_item.font_request(item_rc);
        if is_fixed_test_font(&font_request.family) {
            let pixel_size = font_request.pixel_size.map_or(10., |s| s.get());
            let text: String = match text_item.text() {
                i_slint_core::item_rendering::PlainOrStyledText::Plain(s) => s.to_string(),
                i_slint_core::item_rendering::PlainOrStyledText::Styled(s) => {
                    i_slint_core::styled_text::get_raw_text(&s).into_owned()
                }
            };
            let max_line_len = text.lines().map(|l: &str| l.len()).max().unwrap_or(0);
            let num_lines = text.lines().count().max(1);
            let width = max_line_len as f32 * pixel_size;
            let height = num_lines as f32 * pixel_size;
            LogicalSize::new(width, height)
        } else {
            sharedparley::text_size(self, text_item, item_rc, max_width, text_wrap)
                .unwrap_or_default()
        }
    }

    fn char_size(
        &self,
        text_item: Pin<&dyn i_slint_core::item_rendering::HasFont>,
        item_rc: &i_slint_core::item_tree::ItemRc,
        ch: char,
    ) -> LogicalSize {
        let font_request = text_item.font_request(item_rc);
        if is_fixed_test_font(&font_request.family) {
            let pixel_size = font_request.pixel_size.map_or(10., |s| s.get());
            LogicalSize::new(pixel_size, pixel_size)
        } else {
            let Some(ctx) = self.slint_context() else {
                return LogicalSize::default();
            };
            let mut font_ctx = ctx.font_context().borrow_mut();
            sharedparley::char_size(&mut font_ctx, text_item, item_rc, ch).unwrap_or_default()
        }
    }

    fn font_metrics(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
    ) -> i_slint_core::items::FontMetrics {
        if is_fixed_test_font(&font_request.family) {
            let pixel_size = font_request.pixel_size.map_or(10., |s| s.get());
            i_slint_core::items::FontMetrics {
                ascent: pixel_size * 0.7,
                descent: -pixel_size * 0.3,
                x_height: 3.,
                cap_height: 7.,
            }
        } else {
            let Some(ctx) = self.slint_context() else {
                return Default::default();
            };
            let mut font_ctx = ctx.font_context().borrow_mut();
            sharedparley::font_metrics(&mut font_ctx, font_request)
        }
    }

    fn text_input_byte_offset_for_position(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        item_rc: &i_slint_core::item_tree::ItemRc,
        pos: LogicalPoint,
    ) -> usize {
        let font_request = text_input.font_request(item_rc);
        if is_fixed_test_font(&font_request.family) {
            let pixel_size = font_request.pixel_size.map_or(10., |s| s.get());
            let text = text_input.text();
            if pos.y < 0. {
                return 0;
            }
            let line = (pos.y / pixel_size) as usize;
            let offset = if line >= 1 {
                text.split('\n').take(line - 1).map(|l| l.len() + 1).sum()
            } else {
                0
            };
            let Some(line) = text.split('\n').nth(line) else {
                return text.len();
            };
            let column = ((pos.x / pixel_size).max(0.) as usize).min(line.len());
            offset + column
        } else {
            sharedparley::text_input_byte_offset_for_position(self, text_input, item_rc, pos)
        }
    }

    fn text_input_cursor_rect_for_byte_offset(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        item_rc: &i_slint_core::item_tree::ItemRc,
        byte_offset: usize,
    ) -> LogicalRect {
        let font_request = text_input.font_request(item_rc);
        if is_fixed_test_font(&font_request.family) {
            let pixel_size = font_request.pixel_size.map_or(10., |s| s.get());
            let text = text_input.text();
            let line = text[..byte_offset].chars().filter(|c| *c == '\n').count();
            let column = text[..byte_offset].split('\n').nth(line).unwrap_or("").len();
            LogicalRect::new(
                Point2D::new(column as f32 * pixel_size, line as f32 * pixel_size),
                Size2D::new(1., pixel_size),
            )
        } else {
            sharedparley::text_input_cursor_rect_for_byte_offset(
                self,
                text_input,
                item_rc,
                byte_offset,
            )
        }
    }

    fn register_font_from_memory(
        &self,
        data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let ctx = self.slint_context().ok_or("slint platform not initialized")?;
        ctx.font_context().borrow_mut().collection.register_fonts(data.to_vec().into(), None);
        Ok(())
    }

    fn register_font_from_path(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let requested_path = path.canonicalize().unwrap_or_else(|_| path.into());
        let contents = std::fs::read(requested_path)?;
        let ctx = self.slint_context().ok_or("slint platform not initialized")?;
        ctx.font_context().borrow_mut().collection.register_fonts(contents.into(), None);
        Ok(())
    }

    fn default_font_size(&self) -> LogicalLength {
        sharedparley::DEFAULT_FONT_SIZE
    }

    fn set_window_adapter(&self, _window_adapter: &Rc<dyn WindowAdapter>) {
        // No-op since TestingWindow is also the WindowAdapter
    }

    fn window_adapter(&self) -> Option<Rc<dyn WindowAdapter>> {
        Some(WindowInner::from_pub(&self.window).window_adapter())
    }

    fn supports_transformations(&self) -> bool {
        true
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
