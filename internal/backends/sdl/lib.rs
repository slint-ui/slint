// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! SDL3 backend for Slint.
//!
//! This backend uses SDL3 for window management and event handling, SDL_Renderer
//! for 2D rendering of Slint UI elements, and SDL_ttf 3.x for text rendering.
//!
//! It is designed for integration with C++ games that already use SDL3: the game
//! can register a pre-render callback to draw its content before the Slint UI is
//! rendered on top.
//!
//! # Game integration
//!
//! ```rust,ignore
//! let backend = i_slint_backend_sdl::Backend::new().unwrap();
//!
//! // Optional: set a callback that runs before Slint renders, so the game
//! // can draw its own content using the same SDL_Renderer.
//! backend.set_pre_render_callback(|renderer_ptr| {
//!     // raw SDL_Renderer* — use it with SDL3 calls to draw the game scene
//! });
//!
//! i_slint_core::platform::set_platform(Box::new(backend)).unwrap();
//! ```
//!
//! # Supported features
//!
//! - Solid-color rectangles and bordered rectangles
//! - Text rendering (via SDL_ttf)
//! - Image rendering
//! - Rectangular clipping
//! - Opacity
//! - Keyboard and mouse input
//! - Clipboard
//!
//! # Not yet supported
//!
//! See `renderer.rs` module-level docs for the full list. In short: gradients,
//! paths, box shadows, rotation/scale transforms, rounded clipping, and layer
//! compositing are not implemented because SDL_Renderer lacks the required
//! primitives. Each can be added incrementally — see the doc comments for what
//! would be needed.

mod fonts;
mod renderer;
mod sdl3_bindings;

use fonts::FontManager;
use renderer::{CachedTexture, SdlItemRenderer};
use sdl3_bindings::*;

use i_slint_core::api::PhysicalSize;
use i_slint_core::graphics::FontRequest;
use i_slint_core::lengths::{LogicalLength, LogicalPoint, LogicalRect, LogicalSize};
use i_slint_core::platform::{Platform, PlatformError, WindowEvent};
use i_slint_core::renderer::RendererSealed;
use i_slint_core::window::{WindowAdapter, WindowInner};
use i_slint_core::SharedString;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::{c_int, c_void};
use std::rc::{Rc, Weak};
use std::time::Instant;

// ---------------------------------------------------------------------------
// Custom SDL event for cross-thread wake-up
// ---------------------------------------------------------------------------

/// We use SDL_EVENT_USER + 0 to signal "invoke callback" from another thread.
const SLINT_SDL_EVENT_INVOKE: u32 = SDL_EVENT_USER.0;
/// We use SDL_EVENT_USER + 1 to signal "request redraw".
const SLINT_SDL_EVENT_REDRAW: u32 = SDL_EVENT_USER.0 + 1;
/// We use SDL_EVENT_USER + 2 to signal "quit event loop".
const SLINT_SDL_EVENT_QUIT: u32 = SDL_EVENT_USER.0 + 2;

// ---------------------------------------------------------------------------
// Backend
// ---------------------------------------------------------------------------

/// The SDL3 backend for Slint.
///
/// Create with [`Backend::new()`], optionally configure a pre-render callback,
/// then pass to [`i_slint_core::platform::set_platform`].
pub struct Backend {
    start_time: Instant,
    window_adapter: RefCell<Option<Rc<SdlWindowAdapter>>>,
    pre_render_callback: RefCell<Option<Box<dyn Fn(*mut c_void)>>>,
}

impl Backend {
    /// Create a new SDL backend. Initializes SDL3 video and SDL_ttf.
    pub fn new() -> Result<Self, PlatformError> {
        log::debug!("[slint-sdl] Initializing SDL3 backend");
        unsafe {
            if !SDL_Init(SDL_INIT_VIDEO | SDL_INIT_EVENTS) {
                return Err(format!("SDL_Init failed: {}", sdl_error()).into());
            }
            if !TTF_Init() {
                return Err(format!("TTF_Init failed: {}", sdl_error()).into());
            }
        }
        log::debug!("[slint-sdl] SDL3 and SDL_ttf initialized successfully");

        Ok(Self {
            start_time: Instant::now(),
            window_adapter: RefCell::new(None),
            pre_render_callback: RefCell::new(None),
        })
    }

    /// Set a callback that is invoked before Slint renders its UI. The callback
    /// receives a raw `SDL_Renderer*` pointer (as `*mut c_void`) so the game can
    /// draw its own content first.
    ///
    /// # Safety
    ///
    /// The pointer passed to the callback is a valid `SDL_Renderer*` for the
    /// duration of the call. Do not store it beyond the callback invocation.
    pub fn set_pre_render_callback(&self, callback: impl Fn(*mut c_void) + 'static) {
        *self.pre_render_callback.borrow_mut() = Some(Box::new(callback));
    }

    /// Returns a raw pointer to the SDL_Renderer, or null if no window has been
    /// created yet. Useful for C++ interop.
    pub fn sdl_renderer_ptr(&self) -> *mut c_void {
        self.window_adapter
            .borrow()
            .as_ref()
            .map_or(std::ptr::null_mut(), |wa| wa.sdl_renderer as *mut c_void)
    }
}

impl Platform for Backend {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let adapter = SdlWindowAdapter::new()?;
        *self.window_adapter.borrow_mut() = Some(adapter.clone());
        // Store weak ref for C FFI access
        SDL_WINDOW_ADAPTER.with(|cell| {
            *cell.borrow_mut() = Some(Rc::downgrade(&adapter));
        });
        Ok(adapter)
    }

    fn run_event_loop(&self) -> Result<(), PlatformError> {
        loop {
            match self.process_events(
                core::time::Duration::from_millis(16),
                i_slint_core::InternalToken,
            )? {
                core::ops::ControlFlow::Break(()) => return Ok(()),
                core::ops::ControlFlow::Continue(()) => {}
            }
        }
    }

    fn process_events(
        &self,
        timeout: core::time::Duration,
        _: i_slint_core::InternalToken,
    ) -> Result<core::ops::ControlFlow<()>, PlatformError> {
        let mut event = SDL_Event::default();
        let timeout_ms = timeout.as_millis().min(i32::MAX as u128) as i32;

        // Process all pending events
        loop {
            let has_event = unsafe { SDL_PollEvent(&mut event) };
            if !has_event {
                break;
            }

            let event_type = unsafe { event.r#type };

            match event_type {
                x if x == SDL_EVENT_QUIT.0 || x == SLINT_SDL_EVENT_QUIT => {
                    return Ok(core::ops::ControlFlow::Break(()));
                }

                x if x == SLINT_SDL_EVENT_INVOKE => {
                    // Retrieve and execute the callback
                    let user = unsafe { event.user };
                    if !user.data1.is_null() {
                        let callback: Box<Box<dyn FnOnce()>> =
                            unsafe { Box::from_raw(user.data1 as *mut Box<dyn FnOnce()>) };
                        callback();
                    }
                }

                x if x == SLINT_SDL_EVENT_REDRAW => {
                    // Handled below in the render phase
                }

                _ => {
                    if let Some(adapter) = self.window_adapter.borrow().as_ref() {
                        adapter.handle_sdl_event(&event);
                    }
                }
            }
        }

        // Update timers and animations
        i_slint_core::platform::update_timers_and_animations();

        // Render if needed
        if let Some(adapter) = self.window_adapter.borrow().as_ref() {
            if adapter.needs_redraw.get() {
                adapter.needs_redraw.set(false);

                // Call pre-render callback (for game rendering).
                // Check both the Rust API callback and the C FFI callback.
                if let Some(ref callback) = *self.pre_render_callback.borrow() {
                    callback(adapter.sdl_renderer as *mut c_void);
                }
                PRE_RENDER_CB.with(|cell| {
                    if let Some(ref cb) = *cell.borrow() {
                        cb(adapter.sdl_renderer as *mut c_void);
                    }
                });

                adapter.render()?;
            }
        }

        // If there's a pending timer, wait for the shorter of timeout or next timer
        if let Some(next_timer) =
            i_slint_core::platform::duration_until_next_timer_update()
        {
            let wait = timeout.min(next_timer);
            if !wait.is_zero() {
                unsafe {
                    SDL_WaitEventTimeout(&mut event, wait.as_millis().min(i32::MAX as u128) as i32);
                    // If we got an event, push it back so it's processed next iteration
                    if event.r#type != 0 {
                        SDL_PushEvent(&mut event);
                    }
                }
            }
        } else if !timeout.is_zero() {
            unsafe {
                SDL_WaitEventTimeout(&mut event, timeout_ms);
                if event.r#type != 0 {
                    SDL_PushEvent(&mut event);
                }
            }
        }

        Ok(core::ops::ControlFlow::Continue(()))
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn i_slint_core::platform::EventLoopProxy>> {
        Some(Box::new(SdlEventLoopProxy))
    }

    fn duration_since_start(&self) -> core::time::Duration {
        self.start_time.elapsed()
    }

    fn set_clipboard_text(&self, text: &str, _clipboard: i_slint_core::platform::Clipboard) {
        if let Ok(c_text) = CString::new(text) {
            unsafe {
                SDL_SetClipboardText(c_text.as_ptr());
            }
        }
    }

    fn clipboard_text(
        &self,
        _clipboard: i_slint_core::platform::Clipboard,
    ) -> Option<String> {
        unsafe {
            let ptr = SDL_GetClipboardText();
            if ptr.is_null() {
                return None;
            }
            let s = std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned();
            SDL_free(ptr as *mut c_void);
            if s.is_empty() { None } else { Some(s) }
        }
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        // Ensure window adapter is dropped before SDL_Quit
        *self.window_adapter.borrow_mut() = None;
        unsafe {
            TTF_Quit();
            SDL_Quit();
        }
    }
}

// ---------------------------------------------------------------------------
// EventLoopProxy — allows posting events from other threads
// ---------------------------------------------------------------------------

struct SdlEventLoopProxy;

impl i_slint_core::platform::EventLoopProxy for SdlEventLoopProxy {
    fn quit_event_loop(&self) -> Result<(), i_slint_core::api::EventLoopError> {
        let mut event = SDL_Event::default();
        event.user = SDL_UserEvent {
            r#type: SLINT_SDL_EVENT_QUIT,
            reserved: 0,
            timestamp: 0,
            windowID: sdl3_sys::video::SDL_WindowID(0),
            code: 0,
            data1: std::ptr::null_mut(),
            data2: std::ptr::null_mut(),
        };
        unsafe {
            SDL_PushEvent(&mut event);
        }
        Ok(())
    }

    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), i_slint_core::api::EventLoopError> {
        // Box the closure twice so we get a thin pointer for the void*
        let boxed: Box<Box<dyn FnOnce()>> = Box::new(event);
        let ptr = Box::into_raw(boxed);

        let mut sdl_event = SDL_Event::default();
        sdl_event.user = SDL_UserEvent {
            r#type: SLINT_SDL_EVENT_INVOKE,
            reserved: 0,
            timestamp: 0,
            windowID: sdl3_sys::video::SDL_WindowID(0),
            code: 0,
            data1: ptr as *mut c_void,
            data2: std::ptr::null_mut(),
        };
        unsafe {
            SDL_PushEvent(&mut sdl_event);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// WindowAdapter
// ---------------------------------------------------------------------------

/// The SDL window adapter. Wraps an SDL_Window and SDL_Renderer.
#[allow(dead_code)]
struct SdlWindowAdapter {
    window: i_slint_core::api::Window,
    sdl_window: *mut SDL_Window,
    sdl_renderer: *mut SDL_Renderer,
    text_engine: *mut sdl3_ttf_sys::ttf::TTF_TextEngine,
    font_manager: FontManager,
    needs_redraw: Cell<bool>,
    visible: Cell<bool>,
    texture_cache: RefCell<HashMap<(usize, u32), CachedTexture>>,
    self_weak: RefCell<Weak<Self>>,
}

impl SdlWindowAdapter {
    fn new() -> Result<Rc<Self>, PlatformError> {
        let title = CString::new("Slint Window").unwrap();

        let sdl_window = unsafe {
            SDL_CreateWindow(
                title.as_ptr(),
                800,
                600,
                SDL_WINDOW_RESIZABLE | SDL_WINDOW_HIGH_PIXEL_DENSITY,
            )
        };
        if sdl_window.is_null() {
            return Err(format!("SDL_CreateWindow failed: {}", sdl_error()).into());
        }

        let sdl_renderer = unsafe { SDL_CreateRenderer(sdl_window, std::ptr::null()) };
        if sdl_renderer.is_null() {
            unsafe { SDL_DestroyWindow(sdl_window) };
            return Err(format!("SDL_CreateRenderer failed: {}", sdl_error()).into());
        }

        // Enable blending by default
        unsafe {
            SDL_SetRenderDrawBlendMode(sdl_renderer, SDL_BLENDMODE_BLEND);
        }

        // Create the SDL_ttf renderer text engine for efficient text rendering.
        // This caches glyph textures internally so repeated draws are fast.
        let text_engine = unsafe { TTF_CreateRendererTextEngine(sdl_renderer) };
        if text_engine.is_null() {
            unsafe {
                SDL_DestroyRenderer(sdl_renderer);
                SDL_DestroyWindow(sdl_window);
            }
            return Err(format!("TTF_CreateRendererTextEngine failed: {}", sdl_error()).into());
        }

        let font_manager = FontManager::new();

        let adapter = Rc::new_cyclic(|weak| Self {
            window: i_slint_core::api::Window::new(weak.clone() as Weak<dyn WindowAdapter>),
            sdl_window,
            sdl_renderer,
            text_engine,
            font_manager,
            needs_redraw: Cell::new(true),
            visible: Cell::new(false),
            texture_cache: RefCell::new(HashMap::new()),
            self_weak: RefCell::new(weak.clone()),
        });

        // Set initial scale factor
        let scale = unsafe { SDL_GetWindowDisplayScale(sdl_window) };
        if scale > 0.0 {
            adapter.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: scale });
        }

        // Set initial size
        let mut w: c_int = 0;
        let mut h: c_int = 0;
        unsafe { SDL_GetWindowSize(sdl_window, &mut w, &mut h) };
        adapter.window.dispatch_event(WindowEvent::Resized {
            size: i_slint_core::api::LogicalSize::new(w as f32, h as f32),
        });

        Ok(adapter)
    }

    /// Render the Slint UI using the SDL_Renderer.
    fn render(&self) -> Result<(), PlatformError> {
        log::debug!("[slint-sdl] Rendering frame");
        let window_inner = WindowInner::from_pub(&self.window);

        // Clear the renderer (transparent so the game's content shows through if
        // the pre-render callback was used)
        unsafe {
            SDL_SetRenderDrawColor(self.sdl_renderer, 0, 0, 0, 0);
            SDL_RenderClear(self.sdl_renderer);
        }

        let component_rc = window_inner.component();
        let window_adapter_rc =
            self.self_weak.borrow().upgrade().unwrap() as Rc<dyn WindowAdapter>;
        let window_adapter_rc = i_slint_core::window::WindowAdapterRc::from(window_adapter_rc);

        let size = self.window.size();
        let sf = self.window.scale_factor();
        let logical_size = LogicalSize::new(size.width as f32 / sf, size.height as f32 / sf);

        // Reset clip to full window
        unsafe {
            SDL_SetRenderClipRect(self.sdl_renderer, std::ptr::null());
        }

        let mut item_renderer = SdlItemRenderer::new(
            self.sdl_renderer,
            self.text_engine,
            &self.font_manager,
            sf,
            window_inner,
            logical_size,
            &self.texture_cache,
        );

        i_slint_core::item_rendering::render_component_items(
            &component_rc,
            &mut item_renderer,
            LogicalPoint::default(),
            &window_adapter_rc,
        );

        unsafe {
            SDL_RenderPresent(self.sdl_renderer);
        }

        Ok(())
    }

    /// Translate an SDL event into Slint WindowEvents and dispatch them.
    fn handle_sdl_event(&self, event: &SDL_Event) {
        let event_type = unsafe { event.r#type };

        match event_type {
            x if x == SDL_EVENT_WINDOW_RESIZED.0 => {
                let we = unsafe { event.window };
                self.window.dispatch_event(WindowEvent::Resized {
                    size: i_slint_core::api::LogicalSize::new(we.data1 as f32, we.data2 as f32),
                });
                self.request_redraw();
            }

            x if x == SDL_EVENT_WINDOW_EXPOSED.0 => {
                // Mark that we need a redraw without pushing an event (to avoid loops)
                self.needs_redraw.set(true);
            }

            x if x == SDL_EVENT_WINDOW_DISPLAY_SCALE_CHANGED.0 => {
                let scale = unsafe { SDL_GetWindowDisplayScale(self.sdl_window) };
                if scale > 0.0 {
                    self.window.dispatch_event(WindowEvent::ScaleFactorChanged {
                        scale_factor: scale,
                    });
                    self.request_redraw();
                }
            }

            x if x == SDL_EVENT_WINDOW_CLOSE_REQUESTED.0 => {
                self.window
                    .dispatch_event(WindowEvent::CloseRequested);
            }

            x if x == SDL_EVENT_MOUSE_MOTION.0 => {
                let me = unsafe { event.motion };
                self.window.dispatch_event(WindowEvent::PointerMoved {
                    position: i_slint_core::api::LogicalPosition::new(me.x, me.y),
                });
            }

            x if x == SDL_EVENT_MOUSE_BUTTON_DOWN.0 => {
                let be = unsafe { event.button };
                let button = sdl_mouse_button(be.button);
                self.window.dispatch_event(WindowEvent::PointerPressed {
                    position: i_slint_core::api::LogicalPosition::new(be.x, be.y),
                    button,
                });
            }

            x if x == SDL_EVENT_MOUSE_BUTTON_UP.0 => {
                let be = unsafe { event.button };
                let button = sdl_mouse_button(be.button);
                self.window.dispatch_event(WindowEvent::PointerReleased {
                    position: i_slint_core::api::LogicalPosition::new(be.x, be.y),
                    button,
                });
            }

            x if x == SDL_EVENT_MOUSE_WHEEL.0 => {
                let we = unsafe { event.wheel };
                self.window.dispatch_event(WindowEvent::PointerScrolled {
                    position: i_slint_core::api::LogicalPosition::new(we.mouse_x, we.mouse_y),
                    delta_x: we.x * 120.0,
                    delta_y: we.y * 120.0,
                });
            }

            x if x == SDL_EVENT_KEY_DOWN.0 => {
                let ke = unsafe { event.key };
                if let Some(text) = sdl_key_to_slint_key(ke.key) {
                    let event = if ke.repeat {
                        WindowEvent::KeyPressRepeated { text }
                    } else {
                        WindowEvent::KeyPressed { text }
                    };
                    self.window.dispatch_event(event);
                }
            }

            x if x == SDL_EVENT_KEY_UP.0 => {
                let ke = unsafe { event.key };
                if let Some(text) = sdl_key_to_slint_key(ke.key) {
                    self.window.dispatch_event(WindowEvent::KeyReleased { text });
                }
            }

            x if x == SDL_EVENT_TEXT_INPUT.0 => {
                let te = unsafe { event.text };
                if !te.text.is_null() {
                    let text_str =
                        unsafe { std::ffi::CStr::from_ptr(te.text) }.to_string_lossy();
                    // Dispatch each character as a key press + release
                    for ch in text_str.chars() {
                        let text = SharedString::from(ch.to_string().as_str());
                        self.window
                            .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
                        self.window
                            .dispatch_event(WindowEvent::KeyReleased { text });
                    }
                }
            }

            _ => {}
        }
    }
}

impl WindowAdapter for SdlWindowAdapter {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }

    fn set_visible(&self, visible: bool) -> Result<(), PlatformError> {
        self.visible.set(visible);
        unsafe {
            if visible {
                SDL_ShowWindow(self.sdl_window);
            } else {
                SDL_HideWindow(self.sdl_window);
            }
        }
        if visible {
            self.request_redraw();
        }
        Ok(())
    }

    fn position(&self) -> Option<i_slint_core::api::PhysicalPosition> {
        let mut x: c_int = 0;
        let mut y: c_int = 0;
        unsafe { SDL_GetWindowPosition(self.sdl_window, &mut x, &mut y) };
        Some(i_slint_core::api::PhysicalPosition::new(x, y))
    }

    fn set_position(&self, position: i_slint_core::api::WindowPosition) {
        let phys = position.to_physical(self.window.scale_factor());
        unsafe {
            SDL_SetWindowPosition(self.sdl_window, phys.x, phys.y);
        }
    }

    fn set_size(&self, size: i_slint_core::api::WindowSize) {
        let phys = size.to_physical(self.window.scale_factor());
        unsafe {
            SDL_SetWindowSize(self.sdl_window, phys.width as c_int, phys.height as c_int);
        }
    }

    fn size(&self) -> PhysicalSize {
        let mut w: c_int = 0;
        let mut h: c_int = 0;
        unsafe { SDL_GetWindowSizeInPixels(self.sdl_window, &mut w, &mut h) };
        PhysicalSize::new(w as u32, h as u32)
    }

    fn request_redraw(&self) {
        self.needs_redraw.set(true);
        // Also push an SDL event to wake up the event loop if it's waiting
        let mut event = SDL_Event::default();
        event.user = SDL_UserEvent {
            r#type: SLINT_SDL_EVENT_REDRAW,
            reserved: 0,
            timestamp: 0,
            windowID: sdl3_sys::video::SDL_WindowID(0),
            code: 0,
            data1: std::ptr::null_mut(),
            data2: std::ptr::null_mut(),
        };
        unsafe {
            SDL_PushEvent(&mut event);
        }
    }

    fn renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        self
    }

    fn update_window_properties(&self, properties: i_slint_core::window::WindowProperties<'_>) {
        let title = properties.title();
        if let Ok(c_title) = CString::new(title.as_str()) {
            unsafe {
                SDL_SetWindowTitle(self.sdl_window, c_title.as_ptr());
            }
        }
    }
}

impl Drop for SdlWindowAdapter {
    fn drop(&mut self) {
        // Clear texture cache before destroying renderer
        self.texture_cache.borrow_mut().clear();

        // Destroy text engine before the renderer (it holds internal textures)
        if !self.text_engine.is_null() {
            unsafe { TTF_DestroyRendererTextEngine(self.text_engine) };
        }
        if !self.sdl_renderer.is_null() {
            unsafe { SDL_DestroyRenderer(self.sdl_renderer) };
        }
        if !self.sdl_window.is_null() {
            unsafe { SDL_DestroyWindow(self.sdl_window) };
        }
    }
}

// ---------------------------------------------------------------------------
// RendererSealed implementation — text measurement and font management
// ---------------------------------------------------------------------------

impl RendererSealed for SdlWindowAdapter {
    fn text_size(
        &self,
        text_item: std::pin::Pin<&dyn i_slint_core::item_rendering::RenderString>,
        item_rc: &i_slint_core::item_tree::ItemRc,
        max_width: Option<LogicalLength>,
        text_wrap: i_slint_core::items::TextWrap,
    ) -> LogicalSize {
        let font_request = text_item.font_request(item_rc);
        let sf = self.window.scale_factor();
        let font = self.font_manager.font_for_request(&font_request, sf);

        let text = match text_item.text() {
            i_slint_core::item_rendering::PlainOrStyledText::Plain(s) => s.to_string(),
            i_slint_core::item_rendering::PlainOrStyledText::Styled(s) => {
                i_slint_core::styled_text::get_raw_text(&s).into_owned()
            }
        };

        let max_width_phys = if text_wrap != i_slint_core::items::TextWrap::NoWrap {
            max_width.map(|w| w.get() * sf)
        } else {
            None
        };

        let (w, h) = self.font_manager.text_size(font, &text, max_width_phys);
        LogicalSize::new(w / sf, h / sf)
    }

    fn char_size(
        &self,
        text_item: std::pin::Pin<&dyn i_slint_core::item_rendering::HasFont>,
        item_rc: &i_slint_core::item_tree::ItemRc,
        ch: char,
    ) -> LogicalSize {
        let font_request = text_item.font_request(item_rc);
        let sf = self.window.scale_factor();
        let font = self.font_manager.font_for_request(&font_request, sf);

        let s = ch.to_string();
        let (w, h) = self.font_manager.text_size(font, &s, None);
        LogicalSize::new(w / sf, h / sf)
    }

    fn font_metrics(
        &self,
        font_request: FontRequest,
    ) -> i_slint_core::items::FontMetrics {
        let sf = self.window.scale_factor();
        let font = self.font_manager.font_for_request(&font_request, sf);
        let (ascent, descent, x_height, cap_height) = self.font_manager.font_metrics(font);

        i_slint_core::items::FontMetrics {
            ascent: ascent / sf,
            descent: descent / sf,
            x_height: x_height / sf,
            cap_height: cap_height / sf,
        }
    }

    fn text_input_byte_offset_for_position(
        &self,
        text_input: std::pin::Pin<&i_slint_core::items::TextInput>,
        item_rc: &i_slint_core::items::ItemRc,
        pos: LogicalPoint,
    ) -> usize {
        let font_request = text_input.font_request(item_rc);
        let sf = self.window.scale_factor();
        let font = self.font_manager.font_for_request(&font_request, sf);
        if font.is_null() {
            return 0;
        }

        let text = text_input.text();

        // Simple single-line hit testing
        let phys_x = pos.x * sf;
        let phys_y = pos.y * sf;

        // Find which line the click is on
        let line_height = unsafe { TTF_GetFontLineSkip(font) } as f32;
        if line_height <= 0.0 {
            return 0;
        }
        let line_idx = ((phys_y / line_height).max(0.0)) as usize;

        let mut byte_offset = 0;
        for (i, line) in text.split('\n').enumerate() {
            if i == line_idx {
                return byte_offset
                    + self.font_manager.byte_offset_for_x(font, line, phys_x);
            }
            byte_offset += line.len() + 1; // +1 for newline
        }

        text.len()
    }

    fn text_input_cursor_rect_for_byte_offset(
        &self,
        text_input: std::pin::Pin<&i_slint_core::items::TextInput>,
        item_rc: &i_slint_core::items::ItemRc,
        byte_offset: usize,
    ) -> LogicalRect {
        let font_request = text_input.font_request(item_rc);
        let sf = self.window.scale_factor();
        let font = self.font_manager.font_for_request(&font_request, sf);
        if font.is_null() {
            return LogicalRect::default();
        }

        let text = text_input.text();
        let line_height = unsafe { TTF_GetFontLineSkip(font) } as f32;

        // Find which line and column the byte_offset is on
        let text_before = &text[..byte_offset.min(text.len())];
        let line_idx = text_before.matches('\n').count();
        let last_newline = text_before.rfind('\n').map_or(0, |pos| pos + 1);
        let line_text = &text_before[last_newline..];

        let x = self.font_manager.x_for_byte_offset(font, line_text, line_text.len());
        let y = line_idx as f32 * line_height;
        let cursor_width = text_input.text_cursor_width().get() * sf;

        LogicalRect::new(
            LogicalPoint::new(x / sf, y / sf),
            LogicalSize::new(cursor_width / sf, line_height / sf),
        )
    }

    fn free_graphics_resources(
        &self,
        _component: i_slint_core::item_tree::ItemTreeRef,
        _items: &mut dyn Iterator<Item = std::pin::Pin<i_slint_core::items::ItemRef<'_>>>,
    ) -> Result<(), PlatformError> {
        // Clear texture cache entries for this component
        // A more precise implementation would only remove textures for the specific items
        self.texture_cache.borrow_mut().clear();
        Ok(())
    }

    fn register_font_from_memory(
        &self,
        data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Try to extract a family name from the font data (simplified)
        let family_name = "CustomFont".to_string();
        self.font_manager
            .register_font_from_memory(family_name, data.to_vec());
        Ok(())
    }

    fn register_font_from_path(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let family_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("CustomFont")
            .to_string();
        self.font_manager.register_font_from_path(
            family_name,
            path.to_string_lossy().into_owned(),
        );
        Ok(())
    }

    fn set_window_adapter(&self, _window_adapter: &Rc<dyn WindowAdapter>) {
        // Already stored via self_weak
    }

    fn window_adapter(&self) -> Option<Rc<dyn WindowAdapter>> {
        self.self_weak
            .borrow()
            .upgrade()
            .map(|rc| rc as Rc<dyn WindowAdapter>)
    }

    fn default_font_size(&self) -> LogicalLength {
        LogicalLength::new(16.0)
    }

    fn supports_transformations(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// Key mapping helpers
// ---------------------------------------------------------------------------

fn sdl_mouse_button(button: u8) -> i_slint_core::platform::PointerEventButton {
    match button as i32 {
        SDL_BUTTON_LEFT => i_slint_core::platform::PointerEventButton::Left,
        SDL_BUTTON_MIDDLE => i_slint_core::platform::PointerEventButton::Middle,
        SDL_BUTTON_RIGHT => i_slint_core::platform::PointerEventButton::Right,
        _ => i_slint_core::platform::PointerEventButton::Other,
    }
}

fn sdl_key_to_slint_key(sdl_key: SDL_Keycode) -> Option<SharedString> {
    use i_slint_core::input::key_codes::Key;

    let key = match sdl_key {
        SDLK_RETURN => Key::Return,
        SDLK_ESCAPE => Key::Escape,
        SDLK_BACKSPACE => Key::Backspace,
        SDLK_TAB => Key::Tab,
        SDLK_DELETE => Key::Delete,
        SDLK_LEFT => Key::LeftArrow,
        SDLK_RIGHT => Key::RightArrow,
        SDLK_UP => Key::UpArrow,
        SDLK_DOWN => Key::DownArrow,
        SDLK_HOME => Key::Home,
        SDLK_END => Key::End,
        SDLK_PAGEUP => Key::PageUp,
        SDLK_PAGEDOWN => Key::PageDown,
        SDLK_F1 => Key::F1,
        SDLK_F2 => Key::F2,
        SDLK_F3 => Key::F3,
        SDLK_F4 => Key::F4,
        SDLK_F5 => Key::F5,
        SDLK_F6 => Key::F6,
        SDLK_F7 => Key::F7,
        SDLK_F8 => Key::F8,
        SDLK_F9 => Key::F9,
        SDLK_F10 => Key::F10,
        SDLK_F11 => Key::F11,
        SDLK_F12 => Key::F12,
        _ => return None, // Printable characters are handled via SDL_EVENT_TEXT_INPUT
    };

    Some(key.into())
}

// ---------------------------------------------------------------------------
// C FFI — allows C/C++ code to interact with the SDL backend
// ---------------------------------------------------------------------------

/// Thread-local storage for the pre-render callback. This is used by the
/// rendering path (which always runs on the main thread) to call the game's
/// rendering function before Slint draws its UI.
thread_local! {
    static PRE_RENDER_CB: RefCell<Option<Box<dyn Fn(*mut c_void)>>> = const { RefCell::new(None) };
}

/// Set the pre-render callback from C/C++. The callback receives the
/// `SDL_Renderer*` as its first argument and a user-supplied context pointer
/// as its second. It is called once before each frame so the game can draw
/// its own content underneath the Slint UI.
///
/// Pass `callback = NULL` to remove a previously set callback.
///
/// # Safety
/// - Must be called from the main thread after `slint_ensure_backend()`.
/// - `user_data` must remain valid until the callback is replaced or cleared.
/// - `drop_user_data` (if non-null) is called when the callback is replaced
///   or cleared, to allow the caller to free `user_data`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_sdl_set_pre_render_callback(
    callback: Option<unsafe extern "C" fn(renderer: *mut c_void, user_data: *mut c_void)>,
    user_data: *mut c_void,
    drop_user_data: Option<unsafe extern "C" fn(*mut c_void)>,
) {
    struct CbData {
        cb: unsafe extern "C" fn(*mut c_void, *mut c_void),
        data: *mut c_void,
        drop_fn: Option<unsafe extern "C" fn(*mut c_void)>,
    }
    impl Drop for CbData {
        fn drop(&mut self) {
            if let Some(f) = self.drop_fn {
                unsafe { f(self.data) };
            }
        }
    }

    PRE_RENDER_CB.with(|cell| {
        *cell.borrow_mut() = callback.map(|cb| {
            let d = CbData { cb, data: user_data, drop_fn: drop_user_data };
            let closure: Box<dyn Fn(*mut c_void)> = Box::new(move |renderer_ptr| {
                unsafe { (d.cb)(renderer_ptr, d.data) };
            });
            closure
        });
    });
}

/// Returns the `SDL_Renderer*` pointer (as `void*`), or null if no SDL
/// window has been created yet.
///
/// # Safety
/// Must be called from the main thread after a window has been shown.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_sdl_get_renderer() -> *mut c_void {
    get_sdl_ptr(|wa| wa.sdl_renderer as *mut c_void)
}

/// Returns the `SDL_Window*` pointer (as `void*`), or null if no SDL
/// window has been created yet.
///
/// # Safety
/// Must be called from the main thread after a window has been shown.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_sdl_get_window() -> *mut c_void {
    get_sdl_ptr(|wa| wa.sdl_window as *mut c_void)
}

/// Helper: access the SdlWindowAdapter through any active window.
fn get_sdl_ptr(f: impl Fn(&SdlWindowAdapter) -> *mut c_void) -> *mut c_void {
    // The SdlWindowAdapter stores a self_weak. We can find it if we have
    // any active window, since for the SDL backend there's only one.
    // Use the platform's stored window_adapter.
    // Since we can't easily downcast the platform, we use a thread-local.
    SDL_WINDOW_ADAPTER.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .map_or(std::ptr::null_mut(), |rc| f(&rc))
    })
}

/// Thread-local weak reference to the SdlWindowAdapter, set when the
/// window is created.
thread_local! {
    static SDL_WINDOW_ADAPTER: RefCell<Option<Weak<SdlWindowAdapter>>> = const { RefCell::new(None) };
}
