// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Embedded mode: Slint draws into a buffer the caller owns.
//!
//! The rest of this crate lets Slint open its own window, which is what the
//! Python and Node.js bindings do. Dart cannot always take that route: the Dart
//! VM runs `main()` on a worker thread, and on macOS a native event loop has to
//! be on the process main thread. Inside Flutter the situation is the same, and
//! a second native window would not compose with the Flutter widget tree
//! anyway.
//!
//! So this module installs the software renderer as the Slint platform and
//! hands the frame over as pixels: the caller resizes, feeds input, ticks
//! timers, and asks for a render whenever it wants a frame. No event loop, no
//! thread requirements. Flutter turns the buffer into a `ui.Image` and delivers
//! pointer and keyboard events back.
//!
//! Call [`slint_dart_embedded_init`] before compiling anything — the platform
//! has to be in place before the first window is created.

use i_slint_core::api::{LogicalPosition, PhysicalSize};
use i_slint_core::platform::{
    Platform, PlatformError, PointerEventButton, WindowEvent, update_timers_and_animations,
};
use i_slint_core::window::WindowAdapter;
use i_slint_renderer_software::{MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType};
use std::cell::RefCell;
use std::ffi::c_char;
use std::rc::Rc;
use std::time::Instant;

use crate::{err, guard, ok, ok_void, str_or_empty};

thread_local! {
    static WINDOW: RefCell<Option<Rc<MinimalSoftwareWindow>>> = const { RefCell::new(None) };
}

struct EmbeddedPlatform {
    window: Rc<MinimalSoftwareWindow>,
    start: Instant,
}

impl Platform for EmbeddedPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }

    fn duration_since_start(&self) -> core::time::Duration {
        self.start.elapsed()
    }
}

fn with_window<T>(f: impl FnOnce(&Rc<MinimalSoftwareWindow>) -> T) -> Option<T> {
    WINDOW.with_borrow(|w| w.as_ref().map(f))
}

/// Install the software renderer as the Slint platform.
///
/// Must be called before the first component is instantiated, and only makes
/// sense once per thread; calling it again is a no-op.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_embedded_init() -> *mut c_char {
    guard(|| {
        if WINDOW.with_borrow(|w| w.is_some()) {
            return ok_void();
        }
        let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
        let platform = EmbeddedPlatform { window: window.clone(), start: Instant::now() };
        match i_slint_core::platform::set_platform(Box::new(platform)) {
            Ok(()) => {
                WINDOW.replace(Some(window));
                ok_void()
            }
            Err(_) => err("a Slint platform is already in place; \
                 call the embedded init before creating any component"),
        }
    })
}

/// Resize the surface. `width` and `height` are physical pixels; `scale_factor`
/// maps them to the logical pixels the `.slint` code sizes itself in.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_embedded_resize(
    width: u32,
    height: u32,
    scale_factor: f32,
) -> *mut c_char {
    guard(|| {
        let resized = with_window(|window| {
            if window.window().scale_factor() != scale_factor {
                window.window().dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor });
            }
            WindowAdapter::set_size(&**window, PhysicalSize::new(width, height).into());
            window.request_redraw();
        });
        resized.map_or_else(uninitialized, |()| ok_void())
    })
}

/// Draw the current frame into `buffer`, which must hold `width * height`
/// RGBA pixels with premultiplied alpha — the layout Flutter's
/// `PixelFormat.rgba8888` expects.
///
/// Returns true when something was drawn. The renderer only repaints what
/// changed, so pass the same buffer every frame and keep its contents between
/// calls.
///
/// # Safety
/// `buffer` must point to at least `width * height * 4` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_embedded_render(
    buffer: *mut u8,
    width: u32,
    height: u32,
) -> bool {
    if buffer.is_null() || width == 0 || height == 0 {
        return false;
    }
    let pixels = unsafe {
        core::slice::from_raw_parts_mut(
            buffer as *mut PremultipliedRgbaColor,
            width as usize * height as usize,
        )
    };
    let drawn = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_window(|window| {
            window.draw_if_needed(|renderer| {
                renderer.render(pixels, width as usize);
            })
        })
    }));
    match drawn {
        Ok(drawn) => drawn.unwrap_or(false),
        Err(panic) => {
            eprintln!("{}", crate::panic_message(&panic));
            false
        }
    }
}

/// Advance Slint's timers and animations, and report how long the caller may
/// idle before the next update is due: milliseconds, or -1 when nothing is
/// pending.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_embedded_tick() -> i64 {
    let ticked = std::panic::catch_unwind(|| {
        update_timers_and_animations();
        i_slint_core::platform::duration_until_next_timer_update()
            .map_or(-1, |d| d.as_millis() as i64)
    });
    ticked.unwrap_or(-1)
}

/// True when an animation is running, so the caller should keep drawing frames.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_embedded_has_active_animations() -> bool {
    with_window(|window| window.window().has_active_animations()).unwrap_or(false)
}

/// Deliver a pointer event at logical position (`x`, `y`).
///
/// `kind` is 0 for pressed, 1 for released, 2 for moved, 3 for scrolled, and 4
/// when the pointer left the surface; `PointerEventKind` on the Dart side names
/// the same numbers. `button` is 0 for left, 1 for right, 2 for middle, and is
/// ignored by the kinds that don't have one. `delta_x` and `delta_y` only apply
/// to scroll events.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_embedded_pointer_event(
    kind: u32,
    x: f32,
    y: f32,
    button: u32,
    delta_x: f32,
    delta_y: f32,
) -> *mut c_char {
    guard(|| {
        let position = LogicalPosition::new(x, y);
        let button = match button {
            1 => PointerEventButton::Right,
            2 => PointerEventButton::Middle,
            _ => PointerEventButton::Left,
        };
        let event = match kind {
            0 => WindowEvent::PointerPressed { position, button },
            1 => WindowEvent::PointerReleased { position, button },
            2 => WindowEvent::PointerMoved { position },
            3 => WindowEvent::PointerScrolled { position, delta_x, delta_y },
            4 => WindowEvent::PointerExited,
            other => return err(format!("unknown pointer event kind: {other}")),
        };
        dispatch(event)
    })
}

/// Deliver a key event. `kind` is 0 for pressed, 1 for repeated, 2 for
/// released. `text` is the key's unicode text; Slint's non-printable keys are
/// the private-use characters listed in its `Key` enum, which the Dart side
/// maps for you.
///
/// # Safety
/// `text` must be a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_embedded_key_event(
    kind: u32,
    text: *const c_char,
) -> *mut c_char {
    guard(|| {
        let text = unsafe { str_or_empty(text) }.into();
        let event = match kind {
            0 => WindowEvent::KeyPressed { text },
            1 => WindowEvent::KeyPressRepeated { text },
            2 => WindowEvent::KeyReleased { text },
            other => return err(format!("unknown key event kind: {other}")),
        };
        dispatch(event)
    })
}

/// Tell Slint whether the surface has keyboard focus, so that text cursors
/// blink and selections render the way the platform expects.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_embedded_focus_event(focused: bool) -> *mut c_char {
    guard(|| dispatch(WindowEvent::WindowActiveChanged(focused)))
}

fn dispatch(event: WindowEvent) -> *mut c_char {
    with_window(|window| {
        window.window().dispatch_event(event);
    })
    .map_or_else(uninitialized, |()| ok_void())
}

fn uninitialized() -> *mut c_char {
    err("embedded mode is not initialized; call the embedded init first")
}

/// The renderer's current buffer size in physical pixels, as `{"width": …,
/// "height": …}`. Useful to check what a resize actually took effect as.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_embedded_size() -> *mut c_char {
    guard(|| {
        with_window(|window| {
            let size = WindowAdapter::size(&**window);
            ok(serde_json::json!({ "width": size.width, "height": size.height }))
        })
        .unwrap_or_else(uninitialized)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{c, unwrap_err, unwrap_ok};
    use std::ffi::{CString, c_void};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// The embedded platform is process-wide and can only be installed once,
    /// so this single test walks the whole cycle: init, size, load, render,
    /// input.
    #[test]
    fn render_a_frame_and_click_it() {
        unwrap_ok(slint_dart_embedded_init());
        // Installing twice is a no-op rather than an error.
        unwrap_ok(slint_dart_embedded_init());

        unwrap_ok(slint_dart_embedded_resize(64, 32, 1.0));
        assert_eq!(
            unwrap_ok(slint_dart_embedded_size()),
            serde_json::json!({ "width": 64, "height": 32 })
        );

        let source = c(r#"
            export component App inherits Window {
                width: 64px;
                height: 32px;
                background: #ff0000;
                in-out property <int> clicks: 0;
                TouchArea {
                    width: 100%;
                    height: 100%;
                    clicked => { root.clicks += 1; }
                }
            }
        "#);
        let path = c("embedded.slint");
        let compiler = crate::slint_dart_compiler_new();
        let result = unsafe {
            crate::slint_dart_compiler_build_from_source(&*compiler, source.as_ptr(), path.as_ptr())
        };
        assert!(!crate::slint_dart_result_has_errors(unsafe { &*result }));
        let definition = unsafe { crate::slint_dart_result_component(&*result, std::ptr::null()) };
        assert!(!definition.is_null());

        let mut error = std::ptr::null_mut();
        let instance = unsafe { crate::slint_dart_definition_create(&*definition, &mut error) };
        assert!(!instance.is_null(), "{:?}", unsafe { CString::from_raw(error) });
        unwrap_ok(crate::slint_dart_instance_show(unsafe { &*instance }, true));

        let mut buffer = vec![0u8; 64 * 32 * 4];
        assert!(
            unsafe { slint_dart_embedded_render(buffer.as_mut_ptr(), 64, 32) },
            "the first frame must be drawn"
        );
        // The window is opaque red, so every pixel is (255, 0, 0, 255).
        assert_eq!(&buffer[0..4], &[255, 0, 0, 255]);
        // Nothing changed, so the next frame is skipped.
        assert!(!unsafe { slint_dart_embedded_render(buffer.as_mut_ptr(), 64, 32) });

        // A press and release inside the touch area counts as a click.
        let clicks = c("clicks");
        unwrap_ok(slint_dart_embedded_pointer_event(0, 10.0, 10.0, 0, 0.0, 0.0));
        unwrap_ok(slint_dart_embedded_pointer_event(1, 10.0, 10.0, 0, 0.0, 0.0));
        let value = unwrap_ok(unsafe {
            crate::slint_dart_instance_get_property(&*instance, std::ptr::null(), clicks.as_ptr())
        });
        assert_eq!(value, 1);

        assert!(
            unwrap_err(slint_dart_embedded_pointer_event(99, 0.0, 0.0, 0, 0.0, 0.0))
                .contains("unknown pointer event kind")
        );
        assert!(
            unwrap_err(unsafe { slint_dart_embedded_key_event(99, c("a").as_ptr()) })
                .contains("unknown key event kind")
        );

        // Ticking without a pending timer reports "nothing due".
        assert_eq!(slint_dart_embedded_tick(), -1);

        unsafe { crate::slint_dart_instance_free(instance) };
        unsafe { crate::slint_dart_definition_free(definition) };
        unsafe { crate::slint_dart_result_free(result) };
        unsafe { crate::slint_dart_compiler_free(compiler) };
    }

    static TIMER_TICKS: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "C" fn count_tick(_user_data: *mut c_void) {
        TIMER_TICKS.fetch_add(1, Ordering::Relaxed);
    }

    /// Drive the timer for at most a second, stopping once `wanted` ticks
    /// have arrived, and answer with how many actually did.
    fn pump_until(wanted: usize) -> usize {
        for _ in 0..100 {
            if TIMER_TICKS.load(Ordering::Relaxed) >= wanted {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            slint_dart_embedded_tick();
        }
        TIMER_TICKS.load(Ordering::Relaxed)
    }

    #[test]
    fn a_repeating_timer_fires_until_it_is_freed() {
        unwrap_ok(slint_dart_embedded_init());
        TIMER_TICKS.store(0, Ordering::Relaxed);

        let timer =
            unsafe { crate::slint_dart_timer_start(true, 5, count_tick, std::ptr::null_mut()) };
        assert!(pump_until(3) >= 3, "the timer never fired");

        unsafe { crate::slint_dart_timer_free(timer) };
        let after_free = TIMER_TICKS.load(Ordering::Relaxed);
        for _ in 0..5 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            slint_dart_embedded_tick();
        }
        assert_eq!(
            TIMER_TICKS.load(Ordering::Relaxed),
            after_free,
            "a freed timer must not fire again"
        );
    }
}
