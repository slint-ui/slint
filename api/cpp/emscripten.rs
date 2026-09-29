// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore desynchronized

//! Platform for `wasm32-unknown-emscripten`: renders a single window into an HTML canvas,
//! with FemtoVG on WebGL 2 or with the software renderer.
//!
//! The browser glue lives in `slint_emscripten.js`, which must be passed to the Emscripten
//! linker with `--js-library`.

use std::boxed::Box;
use std::cell::{Cell, RefCell};
use std::ffi::{CStr, CString, c_char, c_int};
use std::rc::{Rc, Weak};
use std::string::ToString;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::vec::Vec;
use std::{eprintln, format, thread_local};

use i_slint_core::api::{LogicalPosition, LogicalSize, PhysicalSize};
use i_slint_core::cursor::MouseCursorInner;
use i_slint_core::platform::{
    EventLoopProxy, Platform, PlatformError, PointerEventButton, WindowAdapter, WindowEvent,
};
use i_slint_core::window::{WindowAdapterInternal, WindowProperties};
use i_slint_core::{SharedString, api::WindowSize};

#[allow(unsafe_code)]
mod ffi {
    use std::ffi::{c_char, c_int};

    unsafe extern "C-unwind" {
        /// With `simulate_infinite_loop`, this unwinds with a JavaScript exception to leave `main()`.
        pub fn emscripten_set_main_loop(
            func: extern "C" fn(),
            fps: c_int,
            simulate_infinite_loop: bool,
        );
    }

    unsafe extern "C" {
        pub fn emscripten_cancel_main_loop();

        // Implemented in slint_emscripten.js
        pub fn slint_em_attach_canvas() -> c_int;
        pub fn slint_em_canvas_css_size(width: *mut f32, height: *mut f32, ratio: *mut f32);
        pub fn slint_em_set_canvas_css_size(width: f32, height: f32);
        pub fn slint_em_set_canvas_buffer_size(width: u32, height: u32);
        pub fn slint_em_set_cursor(name: *const c_char);
        pub fn slint_em_set_title(title: *const c_char);
        #[cfg(feature = "renderer-software")]
        pub fn slint_em_put_image_data(pixels: *const u8, width: u32, height: u32);
    }
}

/// How `slint_em_attach_canvas` found the canvas.
#[derive(Clone, Copy, PartialEq)]
enum CanvasSizing {
    /// The page gave the canvas a size in its `style` attribute, so Slint follows it.
    Page,
    /// Slint sizes the canvas to fit the window.
    Window,
}

#[allow(unsafe_code)]
fn attach_canvas() -> Result<CanvasSizing, PlatformError> {
    match unsafe { ffi::slint_em_attach_canvas() } {
        1 => Ok(CanvasSizing::Window),
        2 => Ok(CanvasSizing::Page),
        _ => Err(PlatformError::Other(
            "Could not find a canvas: set Module.canvas, or add a <canvas id=\"canvas\"> to the page"
                .into(),
        )),
    }
}

#[allow(unsafe_code)]
fn canvas_css_size() -> (LogicalSize, f32) {
    let (mut width, mut height, mut ratio) = (0f32, 0f32, 1f32);
    unsafe { ffi::slint_em_canvas_css_size(&mut width, &mut height, &mut ratio) };
    (LogicalSize::new(width, height), ratio)
}

#[allow(unsafe_code)]
fn set_canvas_css_size(size: LogicalSize) {
    unsafe { ffi::slint_em_set_canvas_css_size(size.width, size.height) };
}

#[allow(unsafe_code)]
fn set_canvas_buffer_size(size: PhysicalSize) {
    unsafe { ffi::slint_em_set_canvas_buffer_size(size.width, size.height) };
}

#[allow(unsafe_code)]
fn set_cursor(name: &str) {
    let name = CString::new(name).unwrap_or_default();
    unsafe { ffi::slint_em_set_cursor(name.as_ptr()) };
}

#[allow(unsafe_code)]
fn set_title(title: &str) {
    let title = CString::new(title).unwrap_or_default();
    unsafe { ffi::slint_em_set_title(title.as_ptr()) };
}

#[cfg(feature = "renderer-femtovg")]
mod webgl {
    #![allow(unsafe_code)]

    use std::boxed::Box;
    use std::ffi::{CStr, c_char, c_int, c_void};
    use std::format;
    use std::num::NonZeroU32;

    use i_slint_core::platform::PlatformError;

    /// Mirrors `EmscriptenWebGLContextAttributes` from `emscripten/html5_webgl.h`.
    #[repr(C)]
    struct ContextAttributes {
        alpha: bool,
        depth: bool,
        stencil: bool,
        antialias: bool,
        premultiplied_alpha: bool,
        preserve_drawing_buffer: bool,
        power_preference: c_int,
        fail_if_major_performance_caveat: bool,
        major_version: c_int,
        minor_version: c_int,
        enable_extensions_by_default: bool,
        explicit_swap_control: bool,
        proxy_context_to_main_thread: c_int,
        render_via_offscreen_back_buffer: bool,
        desynchronized: bool,
    }

    unsafe extern "C" {
        fn emscripten_webgl_init_context_attributes(attributes: *mut ContextAttributes);
        fn emscripten_webgl_create_context(
            target: *const c_char,
            attributes: *const ContextAttributes,
        ) -> usize;
        fn emscripten_webgl_make_context_current(context: usize) -> c_int;
        fn emscripten_webgl_get_proc_address(name: *const c_char) -> *const c_void;
    }

    pub struct WebGLContext(usize);

    impl WebGLContext {
        pub fn new() -> Result<Self, PlatformError> {
            let context = unsafe {
                let mut attributes = std::mem::MaybeUninit::<ContextAttributes>::uninit();
                emscripten_webgl_init_context_attributes(attributes.as_mut_ptr());
                let mut attributes = attributes.assume_init();
                attributes.major_version = 2;
                attributes.stencil = true;
                attributes.antialias = false;
                // Registered by slint_em_attach_canvas
                emscripten_webgl_create_context(c"!slint-canvas".as_ptr(), &attributes)
            };
            if context == 0 {
                return Err(PlatformError::Other("Could not create a WebGL 2 context".into()));
            }
            // The renderer loads the GL functions before it asks for the context to be current.
            if unsafe { emscripten_webgl_make_context_current(context) } != 0 {
                return Err(PlatformError::Other(
                    "Could not make the WebGL context current".into(),
                ));
            }
            Ok(Self(context))
        }
    }

    unsafe impl i_slint_renderer_femtovg::opengl::OpenGLInterface for WebGLContext {
        fn ensure_current(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            match unsafe { emscripten_webgl_make_context_current(self.0) } {
                0 => Ok(()),
                error => Err(format!("Could not make the WebGL context current: {error}").into()),
            }
        }

        fn swap_buffers(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            // The browser presents the canvas when control returns to it.
            Ok(())
        }

        fn resize(
            &self,
            _width: NonZeroU32,
            _height: NonZeroU32,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        fn get_proc_address(&self, name: &CStr) -> *const c_void {
            unsafe { emscripten_webgl_get_proc_address(name.as_ptr()) }
        }
    }
}

enum WindowRenderer {
    #[cfg(feature = "renderer-femtovg")]
    FemtoVG(i_slint_renderer_femtovg::FemtoVGOpenGLRenderer),
    #[cfg(feature = "renderer-software")]
    Software {
        renderer: i_slint_renderer_software::SoftwareRenderer,
        pixels: RefCell<Vec<i_slint_renderer_software::PremultipliedRgbaColor>>,
        rgba: RefCell<Vec<u8>>,
    },
}

#[derive(Clone, Copy, PartialEq)]
enum RendererKind {
    #[cfg(feature = "renderer-femtovg")]
    FemtoVG,
    #[cfg(feature = "renderer-software")]
    Software,
}

impl RendererKind {
    fn from_name(name: Option<&str>) -> Result<Self, PlatformError> {
        match name {
            #[cfg(feature = "renderer-femtovg")]
            None | Some("" | "femtovg" | "gl") => Ok(Self::FemtoVG),
            #[cfg(all(feature = "renderer-software", not(feature = "renderer-femtovg")))]
            None | Some("") => Ok(Self::Software),
            #[cfg(feature = "renderer-software")]
            Some("software" | "sw") => Ok(Self::Software),
            Some(name) => Err(PlatformError::Other(format!(
                "The Emscripten backend doesn't support the renderer '{name}'"
            ))),
            #[allow(unreachable_patterns)]
            None => Err(PlatformError::Other(
                "The Emscripten backend needs the renderer-femtovg or renderer-software feature"
                    .into(),
            )),
        }
    }

    fn create(self) -> Result<WindowRenderer, PlatformError> {
        match self {
            #[cfg(feature = "renderer-femtovg")]
            Self::FemtoVG => Ok(WindowRenderer::FemtoVG(
                i_slint_renderer_femtovg::FemtoVGOpenGLRenderer::new(webgl::WebGLContext::new()?)?,
            )),
            #[cfg(feature = "renderer-software")]
            Self::Software => Ok(WindowRenderer::Software {
                renderer: i_slint_renderer_software::SoftwareRenderer::new_with_repaint_buffer_type(
                    i_slint_renderer_software::RepaintBufferType::ReusedBuffer,
                ),
                pixels: Default::default(),
                rgba: Default::default(),
            }),
        }
    }
}

impl WindowRenderer {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        match self {
            #[cfg(feature = "renderer-femtovg")]
            Self::FemtoVG(renderer) => renderer,
            #[cfg(feature = "renderer-software")]
            Self::Software { renderer, .. } => renderer,
        }
    }

    #[allow(unsafe_code)]
    #[cfg_attr(not(feature = "renderer-software"), allow(unused_variables))]
    fn render(&self, size: PhysicalSize) -> Result<(), PlatformError> {
        match self {
            #[cfg(feature = "renderer-femtovg")]
            Self::FemtoVG(renderer) => renderer.render(),
            #[cfg(feature = "renderer-software")]
            Self::Software { renderer, pixels, rgba } => {
                let (width, height) = (size.width as usize, size.height as usize);
                let mut pixels = pixels.borrow_mut();
                pixels.resize(width * height, Default::default());
                renderer.render(&mut pixels, width);

                // putImageData takes straight alpha.
                let mut rgba = rgba.borrow_mut();
                rgba.clear();
                rgba.extend(pixels.iter().flat_map(|pixel| {
                    let alpha = pixel.alpha as u16;
                    let unpremultiply = |channel: u8| match alpha {
                        0 => 0,
                        255 => channel,
                        _ => ((channel as u16 * 255 + alpha / 2) / alpha).min(255) as u8,
                    };
                    [
                        unpremultiply(pixel.red),
                        unpremultiply(pixel.green),
                        unpremultiply(pixel.blue),
                        pixel.alpha,
                    ]
                }));
                unsafe { ffi::slint_em_put_image_data(rgba.as_ptr(), size.width, size.height) };
                Ok(())
            }
        }
    }
}

pub struct EmscriptenWindowAdapter {
    window: i_slint_core::api::Window,
    renderer: WindowRenderer,
    canvas_sizing: CanvasSizing,
    size: Cell<PhysicalSize>,
    scale_factor: Cell<f32>,
    redraw_requested: Cell<bool>,
    /// Whether the canvas has been given the window's preferred size yet.
    sized_to_preferred: Cell<bool>,
    title: RefCell<SharedString>,
}

impl EmscriptenWindowAdapter {
    fn new(renderer: WindowRenderer, canvas_sizing: CanvasSizing) -> Rc<Self> {
        Rc::new_cyclic(|self_weak: &Weak<Self>| Self {
            window: i_slint_core::api::Window::new(self_weak.clone()),
            renderer,
            canvas_sizing,
            size: Default::default(),
            scale_factor: Cell::new(1.),
            redraw_requested: Cell::new(true),
            sized_to_preferred: Cell::new(false),
            title: Default::default(),
        })
    }

    fn dispatch(&self, event: WindowEvent) {
        if let Err(error) = self.window.dispatch_event_with_result(event) {
            eprintln!("Slint: error dispatching an event: {error}");
        }
    }

    fn update_size(&self, css_size: LogicalSize, scale_factor: f32) {
        if scale_factor != self.scale_factor.get() {
            self.scale_factor.set(scale_factor);
            self.dispatch(WindowEvent::ScaleFactorChanged { scale_factor });
        }
        let size = css_size.to_physical(scale_factor);
        set_canvas_buffer_size(size);
        if size != self.size.get() {
            self.size.set(size);
            self.dispatch(WindowEvent::Resized { size: css_size });
        }
        self.request_redraw();
    }

    fn render_if_needed(&self) -> Result<(), PlatformError> {
        let size = self.size.get();
        if size.width == 0 || size.height == 0 || !self.redraw_requested.replace(false) {
            return Ok(());
        }
        self.renderer.render(size)?;
        if self.window.has_active_animations() {
            self.redraw_requested.set(true);
        }
        Ok(())
    }
}

impl WindowAdapter for EmscriptenWindowAdapter {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        self.size.get()
    }

    fn set_size(&self, size: WindowSize) {
        if self.canvas_sizing == CanvasSizing::Window {
            self.sized_to_preferred.set(true);
            set_canvas_css_size(size.to_logical(self.scale_factor.get()));
        }
    }

    fn set_visible(&self, visible: bool) -> Result<(), PlatformError> {
        if visible {
            let (css_size, scale_factor) = canvas_css_size();
            self.update_size(css_size, scale_factor);
        }
        Ok(())
    }

    fn renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        self.renderer.as_core_renderer()
    }

    fn request_redraw(&self) {
        self.redraw_requested.set(true);
    }

    fn update_window_properties(&self, properties: WindowProperties<'_>) {
        let title = properties.title();
        if *self.title.borrow() != title {
            set_title(&title);
            *self.title.borrow_mut() = title;
        }

        if self.canvas_sizing == CanvasSizing::Window {
            let constraints = properties.layout_constraints();
            let (current, _) = canvas_css_size();
            let mut size =
                if self.sized_to_preferred.replace(true) { current } else { constraints.preferred };
            if let Some(min) = constraints.min {
                size.width = size.width.max(min.width);
                size.height = size.height.max(min.height);
            }
            if let Some(max) = constraints.max {
                size.width = size.width.min(max.width);
                size.height = size.height.min(max.height);
            }
            if size != current {
                set_canvas_css_size(size);
            }
        }
    }

    fn internal(&self, _: i_slint_core::InternalToken) -> Option<&dyn WindowAdapterInternal> {
        Some(self)
    }
}

impl WindowAdapterInternal for EmscriptenWindowAdapter {
    fn set_mouse_cursor(&self, cursor: MouseCursorInner) {
        match cursor {
            MouseCursorInner::BuiltIn(cursor) => set_cursor(&cursor.to_string()),
            _ => set_cursor("default"),
        }
    }
}

thread_local! {
    static WINDOW: RefCell<Weak<EmscriptenWindowAdapter>> = const { RefCell::new(Weak::new()) };
    static CANVAS_SIZING: Cell<Option<CanvasSizing>> = const { Cell::new(None) };
}

fn current_window() -> Option<Rc<EmscriptenWindowAdapter>> {
    WINDOW.with(|window| window.borrow().upgrade())
}

static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);
static QUEUED_CALLBACKS: Mutex<Vec<Box<dyn FnOnce() + Send>>> = Mutex::new(Vec::new());

struct Proxy;

impl EventLoopProxy for Proxy {
    fn quit_event_loop(&self) -> Result<(), i_slint_core::api::EventLoopError> {
        QUIT_REQUESTED.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), i_slint_core::api::EventLoopError> {
        QUEUED_CALLBACKS.lock().unwrap().push(event);
        Ok(())
    }
}

#[allow(unsafe_code)]
extern "C" fn run_frame() {
    let callbacks = std::mem::take(&mut *QUEUED_CALLBACKS.lock().unwrap());
    for callback in callbacks {
        callback();
    }
    if QUIT_REQUESTED.swap(false, Ordering::Relaxed) {
        unsafe { ffi::emscripten_cancel_main_loop() };
        return;
    }
    i_slint_core::platform::update_timers_and_animations();
    if let Some(window) = current_window()
        && let Err(error) = window.render_if_needed()
    {
        eprintln!("Slint: error rendering: {error}");
    }
}

struct Backend {
    renderer: RendererKind,
}

/// Creates the platform, with the renderer that `SLINT_BACKEND` names,
/// such as `emscripten-software`, or the default renderer.
pub fn create_platform() -> Result<Box<dyn Platform>, PlatformError> {
    let backend = std::env::var("SLINT_BACKEND").unwrap_or_default().to_lowercase();
    let renderer = match backend.split_once('-') {
        Some(("emscripten", renderer)) => renderer,
        None if backend == "emscripten" => "",
        _ => backend.as_str(),
    };
    let renderer = RendererKind::from_name((!renderer.is_empty()).then_some(renderer))?;
    Ok(Box::new(Backend { renderer }))
}

impl Platform for Backend {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        if current_window().is_some() {
            return Err(PlatformError::Other(
                "The Emscripten backend supports only one window".into(),
            ));
        }
        let canvas_sizing = match CANVAS_SIZING.get() {
            Some(sizing) => sizing,
            None => {
                let sizing = attach_canvas()?;
                CANVAS_SIZING.set(Some(sizing));
                sizing
            }
        };
        let adapter = EmscriptenWindowAdapter::new(self.renderer.create()?, canvas_sizing);
        WINDOW.with(|window| *window.borrow_mut() = Rc::downgrade(&adapter));
        Ok(adapter)
    }

    /// Hands control to the browser and never returns: see `emscripten_set_main_loop`.
    #[allow(unsafe_code)]
    fn run_event_loop(&self) -> Result<(), PlatformError> {
        unsafe { ffi::emscripten_set_main_loop(run_frame, 0, true) };
        Ok(())
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        Some(Box::new(Proxy))
    }
}

// Called from slint_emscripten.js

#[unsafe(no_mangle)]
extern "C" fn slint_em_on_pointer(kind: c_int, x: f32, y: f32, button: c_int, is_touch: bool) {
    let Some(window) = current_window() else { return };
    let position = LogicalPosition::new(x, y);
    let button = match button {
        0 => PointerEventButton::Left,
        1 => PointerEventButton::Middle,
        2 => PointerEventButton::Right,
        3 => PointerEventButton::Back,
        4 => PointerEventButton::Forward,
        _ => PointerEventButton::Other,
    };
    match kind {
        0 => window.dispatch(WindowEvent::PointerPressed { position, button }),
        1 => {
            window.dispatch(WindowEvent::PointerReleased { position, button });
            // A lifted finger leaves nothing hovered.
            if is_touch {
                window.dispatch(WindowEvent::PointerExited);
            }
        }
        2 => window.dispatch(WindowEvent::PointerMoved { position }),
        _ => window.dispatch(WindowEvent::PointerExited),
    }
}

#[unsafe(no_mangle)]
extern "C" fn slint_em_on_wheel(x: f32, y: f32, delta_x: f32, delta_y: f32) {
    let Some(window) = current_window() else { return };
    window.dispatch(WindowEvent::PointerScrolled {
        position: LogicalPosition::new(x, y),
        delta_x,
        delta_y,
    });
}

/// Returns whether the key maps to a Slint key, and so was dispatched.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
extern "C" fn slint_em_on_key(
    pressed: bool,
    key: *const c_char,
    shift: bool,
    repeat: bool,
) -> bool {
    let Some(window) = current_window() else { return false };
    let key = unsafe { CStr::from_ptr(key) }.to_string_lossy();
    let Some(text) = key_text(&key, shift, i_slint_core::is_apple_platform()) else {
        return false;
    };
    window.dispatch(match (pressed, repeat) {
        (true, false) => WindowEvent::KeyPressed { text },
        (true, true) => WindowEvent::KeyPressRepeated { text },
        (false, _) => WindowEvent::KeyReleased { text },
    });
    true
}

#[unsafe(no_mangle)]
extern "C" fn slint_em_on_focus(focused: bool) {
    let Some(window) = current_window() else { return };
    window.dispatch(WindowEvent::WindowActiveChanged(focused));
}

#[unsafe(no_mangle)]
extern "C" fn slint_em_on_resize(width: f32, height: f32, scale_factor: f32) {
    let Some(window) = current_window() else { return };
    window.update_size(LogicalSize::new(width, height), scale_factor);
}

/// Maps a DOM `KeyboardEvent.key` to the text of a Slint key event.
/// See `event_text` in the winit backend's `wasm_input_helper.rs`.
fn key_text(key: &str, shift: bool, is_apple: bool) -> Option<SharedString> {
    use i_slint_core::platform::Key;

    macro_rules! check_non_printable_code {
        ($($char:literal # $name:ident # $($shifted:ident)? $(=> $($_muda:ident)? # $($qt:ident)|* # $($_winit:ident $(($_pos:ident))?)|* # $($_xkb:ident)|* )? ;)*) => {
            match key {
                "Tab" if shift => return Some(Key::Backtab.into()),
                "Meta" if is_apple => return Some(Key::Control.into()),
                "Control" if is_apple => return Some(Key::Meta.into()),
                $($(stringify!($name) => {
                    $(let _ = stringify!($qt);)*
                    return Some($char.into());
                })?)*
                "ArrowLeft" => return Some(Key::LeftArrow.into()),
                "ArrowUp" => return Some(Key::UpArrow.into()),
                "ArrowRight" => return Some(Key::RightArrow.into()),
                "ArrowDown" => return Some(Key::DownArrow.into()),
                "Enter" => return Some(Key::Return.into()),
                _ => (),
            }
        };
    }
    i_slint_common::for_each_keys!(check_non_printable_code);

    let mut chars = key.chars();
    match chars.next() {
        Some(first_char) if chars.next().is_none() => Some(first_char.into()),
        _ => None,
    }
}
