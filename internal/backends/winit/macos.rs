// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Integrates macOS window presentation and application termination.
//!
//! Keeps a freshly shown window invisible until its first frame is submitted.
//!
//! wgpu's Metal surface hands out no drawable while the hosting `NSWindow` reports itself
//! as not visible (gfx-rs/wgpu#8309).
//! A window that hasn't been ordered front yet counts as such.
//! So the frame rendered before showing the window is dropped,
//! and AppKit maps the window with an empty content layer.
//! The title bar shows up alone until `occlusionState` catches up and a redraw lands.
//! Ordering the window front with an alpha of zero hides that gap.
//! The window server still reports such a window as visible, so the redraw goes through,
//! and the alpha is restored once a frame is submitted.
//!
//! wgpu fixes this in <https://github.com/gfx-rs/wgpu/pull/10302>.
//! Remove this module once every wgpu version Slint builds against contains that fix.

use objc2::rc::Retained;
use objc2_app_kit::{NSView, NSWindow};

/// Makes a window fully transparent for as long as this value lives.
/// Drop it when the first frame is submitted.
pub(crate) struct RevealOnFirstFrame {
    ns_window: Retained<NSWindow>,
    alpha: f64,
}

impl RevealOnFirstFrame {
    pub(crate) fn new(winit_window: &winit::window::Window) -> Option<Self> {
        let ns_window = ns_view(winit_window)?.window()?;
        let alpha = ns_window.alphaValue();
        ns_window.setAlphaValue(0.0);
        Some(Self { ns_window, alpha })
    }
}

impl Drop for RevealOnFirstFrame {
    fn drop(&mut self) {
        self.ns_window.setAlphaValue(self.alpha);
    }
}

pub(crate) fn ns_view(winit_window: &winit::window::Window) -> Option<&NSView> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let RawWindowHandle::AppKit(handle) = winit_window.window_handle().ok()?.as_raw() else {
        return None;
    };
    Some(unsafe { handle.ns_view.cast().as_ref() })
}

thread_local! {
    static QUIT_BACKEND: std::cell::RefCell<std::rc::Weak<crate::SharedBackendData>> = Default::default();
}

pub(crate) fn install_termination_handler(backend: std::rc::Weak<crate::SharedBackendData>) {
    use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
    use objc2::sel;

    unsafe extern "C-unwind" fn should_terminate(
        _this: *mut AnyObject,
        _cmd: Sel,
        _app: *mut AnyObject,
    ) -> usize {
        const TERMINATE_CANCEL: usize = 0;
        const TERMINATE_NOW: usize = 1;

        QUIT_BACKEND.with(|slot| {
            let Some(backend) = slot.borrow().upgrade() else { return TERMINATE_NOW };
            if !backend.native_quit_active.get() {
                return TERMINATE_NOW;
            }
            let generation =
                backend.event_loop_generation.load(std::sync::atomic::Ordering::Relaxed);
            match backend
                .event_loop_proxy
                .send_event(crate::SlintEvent(crate::CustomEvent::Exit(generation)))
            {
                Ok(()) => TERMINATE_CANCEL,
                Err(_) => TERMINATE_NOW,
            }
        })
    }

    QUIT_BACKEND.with(|slot| *slot.borrow_mut() = backend);
    // Winit 0.30 does not expose a replaceable application delegate.
    let selector = sel!(applicationShouldTerminate:);
    if let Some(class) = AnyClass::get(c"WinitApplicationDelegate")
        && class.instance_method(selector).is_none()
    {
        unsafe {
            objc2::ffi::class_addMethod(
                (class as *const AnyClass).cast_mut(),
                selector,
                core::mem::transmute::<
                    unsafe extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject) -> usize,
                    Imp,
                >(should_terminate),
                c"Q@:@".as_ptr(),
            );
        }
    }
}
