// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::PhysicalSize;
use i_slint_core::graphics::euclid::{Point2D, Size2D};
use i_slint_core::item_rendering::HasFont;
use i_slint_core::lengths::{LogicalLength, LogicalPoint, LogicalRect, LogicalSize};
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::{Renderer, RendererSealed};
use i_slint_core::textlayout::sharedparley;
use i_slint_core::window::{
    InputMethodRequest, WindowAdapter, WindowAdapterInternal, WindowInner, WindowKind,
};

use i_slint_core::SharedString;
use i_slint_core::api::LogicalPosition;
use i_slint_core::input::MouseEvent;
use i_slint_core::items::{AllowedDragActions, DragAction, DropEvent, TextWrap};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::sync::Mutex;

std::thread_local! {
    /// Live windows targeted by [`ensure_all_tracked_trees_instantiated`].
    static ALL_TESTING_WINDOWS: RefCell<Vec<Weak<TestingWindow>>> =
        const { RefCell::new(Vec::new()) }
}

/// Run the `ensure_instantiated` repeater instantiation pass on every live
/// testing window.
fn ensure_all_tracked_trees_instantiated() {
    let live: Vec<Rc<TestingWindow>> = ALL_TESTING_WINDOWS.with(|list| {
        let mut list = list.borrow_mut();
        list.retain(|w| w.upgrade().is_some());
        list.iter().filter_map(|w| w.upgrade()).collect()
    });
    for tw in live {
        WindowInner::from_pub(&tw.window).ensure_tree_instantiated();
    }
}

/// Advance the mocked time by the given number of milliseconds, updating
/// animations, firing timers, running change handlers, and instantiating
/// pending repeaters and conditionals on every live testing window.
pub fn mock_elapsed_time(time_in_ms: u64) {
    let tick = i_slint_core::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
        let mut tick = driver.current_tick();
        tick += core::time::Duration::from_millis(time_in_ms);
        driver.update_animations(tick);
        tick
    });
    i_slint_core::timers::TimerList::maybe_activate_timers(tick);
    i_slint_core::properties::ChangeTracker::run_change_handlers();
    // Happens after change handlers so changing the running prop happens on the same frame
    i_slint_core::animations::update_animation_objects();
    ensure_all_tracked_trees_instantiated();
}

/// Return the current mocked time in milliseconds.
pub fn get_mocked_time() -> u64 {
    i_slint_core::animations::CURRENT_ANIMATION_DRIVER
        .with(|driver| driver.current_tick())
        .as_millis()
}

#[cfg(any(feature = "internal", feature = "ffi"))]
/// Simulate a click at (`x`, `y`) and release after 50 ms of mock time.
pub fn send_mouse_click(x: f32, y: f32, window_adapter: &i_slint_core::window::WindowAdapterRc) {
    use i_slint_core::api::LogicalPosition;
    use i_slint_core::items::PointerEventButton;
    use i_slint_core::platform::WindowEvent;

    let position = LogicalPosition::new(x, y);
    let button = PointerEventButton::Left;

    window_adapter.window().dispatch_event(WindowEvent::PointerMoved { position });
    window_adapter.window().dispatch_event(WindowEvent::PointerPressed { position, button });
    mock_elapsed_time(50);
    window_adapter.window().dispatch_event(WindowEvent::PointerReleased { position, button });
}

#[cfg(any(feature = "internal", feature = "ffi"))]
/// Dispatch a single key press or release event.
pub fn send_keyboard_key_text(
    text: &i_slint_core::SharedString,
    pressed: bool,
    window_adapter: &i_slint_core::window::WindowAdapterRc,
) {
    use i_slint_core::platform::WindowEvent;
    window_adapter.window().dispatch_event(if pressed {
        WindowEvent::KeyPressed { text: text.clone() }
    } else {
        WindowEvent::KeyReleased { text: text.clone() }
    })
}

#[cfg(feature = "ffi")]
/// Dispatch each character in the string as a separate key event.
pub fn send_keyboard_char(
    string: &i_slint_core::SharedString,
    pressed: bool,
    window_adapter: &i_slint_core::window::WindowAdapterRc,
) {
    for ch in string.chars() {
        send_keyboard_key_text(&ch.into(), pressed, window_adapter);
    }
}

#[cfg(any(feature = "internal", feature = "ffi"))]
/// Simulate typing a string, with automatic Shift handling for uppercase letters.
pub fn send_keyboard_string_sequence(
    sequence: &i_slint_core::SharedString,
    window_adapter: &i_slint_core::window::WindowAdapterRc,
) {
    use i_slint_core::input::key_codes::Key;
    use i_slint_core::platform::WindowEvent;

    for ch in sequence.chars() {
        if ch.is_ascii_uppercase() {
            window_adapter
                .window()
                .dispatch_event(WindowEvent::KeyPressed { text: Key::Shift.into() });
        }

        let text: i_slint_core::SharedString = ch.into();
        window_adapter.window().dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        window_adapter.window().dispatch_event(WindowEvent::KeyReleased { text });

        if ch.is_ascii_uppercase() {
            window_adapter
                .window()
                .dispatch_event(WindowEvent::KeyReleased { text: Key::Shift.into() });
        }
    }
}

const FIXED_TEST_FONT: &str = "FixedTestFont";

fn is_fixed_test_font(family: &Option<SharedString>) -> bool {
    family.as_ref().is_some_and(|f| f == FIXED_TEST_FONT)
}

#[derive(Default)]
pub struct TestingBackendOptions {
    pub mock_time: bool,
    pub threading: bool,
    /// When set, windows embed a real rasterizer so headless rendering
    /// (e.g. `Window::take_snapshot`) works. Recognized names: `software`,
    /// `skia`; an empty string or `default` picks the best available.
    /// When `None`, the backend keeps its mock renderer with fixed font
    /// metrics.
    #[cfg(supports_headless)]
    pub renderer_name: Option<SharedString>,
}

pub struct TestingBackend {
    clipboard: Mutex<Option<String>>,
    queue: Option<Queue>,
    mock_time: bool,
    pub open_url: Rc<RefCell<Option<SharedString>>>,
    pub debug_logs: Rc<RefCell<Vec<String>>>,
    #[cfg(supports_headless)]
    renderer_name: Option<SharedString>,
}

impl TestingBackend {
    pub fn new(options: TestingBackendOptions) -> Self {
        Self {
            clipboard: Mutex::default(),
            queue: options.threading.then(|| Queue(Default::default(), std::thread::current())),
            mock_time: options.mock_time,
            open_url: Default::default(),
            debug_logs: Default::default(),
            #[cfg(supports_headless)]
            renderer_name: options.renderer_name,
        }
    }
}

impl i_slint_core::platform::Platform for TestingBackend {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn WindowAdapter>, i_slint_core::platform::PlatformError> {
        #[cfg(supports_headless)]
        let renderer =
            self.renderer_name.as_ref().map(|name| create_headless_renderer(name)).transpose()?;
        let window = Rc::new_cyclic(|self_weak| TestingWindow {
            window: i_slint_core::api::Window::new(self_weak.clone() as _),
            size: Default::default(),
            ime_requests: Default::default(),
            mouse_cursor: Default::default(),
            all_item_trees: Default::default(),
            open_url: self.open_url.clone(),
            debug_logs: self.debug_logs.clone(),
            native_popup: Cell::new(false),
            simulate_native_drag: Cell::new(false),
            native_drag: Default::default(),
            #[cfg(supports_headless)]
            renderer_name: self.renderer_name.clone(),
            #[cfg(supports_headless)]
            renderer,
        });
        ALL_TESTING_WINDOWS.with(|list| list.borrow_mut().push(Rc::downgrade(&window)));
        Ok(window)
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

    fn open_url(&self, url: &str) -> Result<(), PlatformError> {
        *self.open_url.borrow_mut() = Some(url.into());
        Ok(())
    }

    fn debug_log(&self, arguments: core::fmt::Arguments) {
        self.debug_logs.borrow_mut().push(arguments.to_string());
        i_slint_core::debug_log::default_log_message(arguments);
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
    mouse_cursor: RefCell<i_slint_core::cursor::MouseCursorInner>,
    all_item_trees: CheckAllItemTreesUnregistered,
    pub open_url: Rc<RefCell<Option<SharedString>>>,
    pub debug_logs: Rc<RefCell<Vec<String>>>,
    native_popup: Cell<bool>,
    simulate_native_drag: Cell<bool>,
    /// Payload and allowed actions recorded by `start_drag` while simulating a native drag,
    /// so the receive-side helpers can build the drop they deliver to a target window.
    native_drag: RefCell<Option<(i_slint_core::data_transfer::DataTransfer, AllowedDragActions)>>,
    /// Remembered for child popups, so they pick the same rasterizer.
    #[cfg(supports_headless)]
    renderer_name: Option<SharedString>,
    /// Rasterizer returned by `WindowAdapter::renderer` when headless
    /// rendering was requested, so every `RendererSealed` call routes through
    /// it. `None` keeps the mock renderer with its fixed test font metrics.
    #[cfg(supports_headless)]
    renderer: Option<Box<dyn Renderer>>,
}

impl TestingWindow {
    pub fn use_native_popup(&self, native: bool) {
        self.native_popup.set(native);
    }

    #[allow(dead_code)] // Used by various tests
    pub fn mouse_cursor(&self) -> i_slint_core::cursor::MouseCursorInner {
        self.mouse_cursor.borrow().clone()
    }

    #[allow(dead_code)]
    pub fn open_url(&self) -> Option<SharedString> {
        self.open_url.borrow().clone()
    }

    /// Drain and return all debug_log messages captured since the last call.
    pub fn take_debug_log(&self) -> Vec<String> {
        self.debug_logs.borrow_mut().drain(..).collect()
    }

    /// Enable simulating native (OS-level) drag-and-drop. Once enabled, `start_drag` takes the
    /// drag over as a real backend does (instead of declining it and using the in-window
    /// fallback), recording the payload so [`Self::simulate_native_drag_move`] and
    /// [`Self::simulate_native_drop`] can drive the receive side.
    pub fn set_simulate_native_drag(&self, enabled: bool) {
        self.simulate_native_drag.set(enabled);
    }

    /// Move the in-flight simulated native drag over `target` at `position`, as a backend does
    /// when the OS drags across a window. Returns the action the target's `DropArea` proposes.
    pub fn simulate_native_drag_move(
        &self,
        target: &i_slint_core::api::Window,
        position: LogicalPosition,
    ) -> DragAction {
        self.deliver_native_drag(target, position, false)
    }

    /// Drop the in-flight simulated native drag onto `target` at `position`, then report
    /// completion back to this source window, as a backend does when the OS drag ends. Returns
    /// the final negotiated action.
    pub fn simulate_native_drop(
        &self,
        target: &i_slint_core::api::Window,
        position: LogicalPosition,
    ) -> DragAction {
        let action = self.deliver_native_drag(target, position, true);
        WindowInner::from_pub(&self.window).report_drag_finished(action);
        action
    }

    fn deliver_native_drag(
        &self,
        target: &i_slint_core::api::Window,
        position: LogicalPosition,
        drop: bool,
    ) -> DragAction {
        let (data, allowed) =
            self.native_drag.borrow().clone().expect("no simulated native drag in flight");
        let mut event = DropEvent::default();
        event.data = data;
        event.position = position;
        event.proposed_action =
            i_slint_core::items::compute_proposed_action(Default::default(), allowed);
        let event = if drop {
            MouseEvent::Drop { event, allowed }
        } else {
            MouseEvent::DragMove { event, allowed }
        };
        WindowInner::from_pub(target)
            .process_mouse_input(event)
            .and_then(|r| r.drag_action)
            .unwrap_or(DragAction::None)
    }
}

impl WindowAdapterInternal for TestingWindow {
    fn input_method_request(&self, request: i_slint_core::window::InputMethodRequest) {
        self.ime_requests.borrow_mut().push(request)
    }

    fn start_drag(&self, request: &i_slint_core::window::DragRequest) -> bool {
        if !self.simulate_native_drag.get() {
            return false;
        }
        *self.native_drag.borrow_mut() = Some((request.data().clone(), request.allowed_actions()));
        true
    }

    fn set_mouse_cursor(&self, cursor: i_slint_core::cursor::MouseCursorInner) {
        self.mouse_cursor.replace(cursor);
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

    fn create_child_window_adapter(&self, _kind: WindowKind) -> Option<Rc<dyn WindowAdapter>> {
        if self.native_popup.get() {
            #[cfg(supports_headless)]
            let renderer = self
                .renderer_name
                .as_ref()
                .map(|name| create_headless_renderer(name))
                .transpose()
                .ok()?;
            let window = Rc::new_cyclic(|self_weak| TestingWindow {
                window: i_slint_core::api::Window::new(self_weak.clone() as _),
                size: Default::default(),
                ime_requests: Default::default(),
                mouse_cursor: Default::default(),
                all_item_trees: Default::default(),
                open_url: self.open_url.clone(),
                debug_logs: self.debug_logs.clone(),
                native_popup: self.native_popup.clone(),
                simulate_native_drag: self.simulate_native_drag.clone(),
                native_drag: Default::default(),
                #[cfg(supports_headless)]
                renderer_name: self.renderer_name.clone(),
                #[cfg(supports_headless)]
                renderer,
            });
            Some(window)
        } else {
            None
        }
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
        #[cfg(supports_headless)]
        if let Some(renderer) = &self.renderer {
            return &**renderer;
        }
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
            let max_lines = text_item.line_limit().unwrap_or(usize::MAX);
            let (max_line_len, num_lines) = text
                .lines()
                .take(max_lines)
                .fold((0, 0), |(len, count), line| (len.max(line.len()), count + 1));
            let width = max_line_len as f32 * pixel_size;
            let height = num_lines.max(1) as f32 * pixel_size;
            LogicalSize::new(width, height)
        } else {
            sharedparley::text_size(self, text_item, item_rc, max_width, text_wrap, None)
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
        ctx.font_context().borrow_mut().register_static_font(data);
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

/// Pick the rasterizer for the headless backend.
/// `""` / `"default"` picks Skia software when compiled in, else the
/// built-in software renderer.
#[cfg(supports_headless)]
fn create_headless_renderer(name: &str) -> Result<Box<dyn Renderer>, PlatformError> {
    match name {
        #[cfg(skia_headless)]
        "" | "default" | "skia" | "skia-software" => {
            std::thread_local! {
                /// Shared across all windows so they reuse Skia resources.
                static SHARED_CONTEXT: i_slint_renderer_skia::SkiaSharedContext =
                    Default::default();
            }
            SHARED_CONTEXT.with(|context| {
                Ok(Box::new(i_slint_renderer_skia::SkiaRenderer::default_software(context)) as _)
            })
        }
        #[cfg(all(feature = "renderer-software", not(skia_headless)))]
        "" | "default" => Ok(Box::new(i_slint_renderer_software::SoftwareRenderer::new())),
        #[cfg(feature = "renderer-software")]
        "sw" | "software" => Ok(Box::new(i_slint_renderer_software::SoftwareRenderer::new())),
        other => {
            let available: &[&str] = &[
                #[cfg(feature = "renderer-software")]
                "software",
                #[cfg(skia_headless)]
                "skia",
            ];
            Err(PlatformError::Other(format!(
                "Unknown headless renderer {other:?} (available: {})",
                available.join(", ")
            )))
        }
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
