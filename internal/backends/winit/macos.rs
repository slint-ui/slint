// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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
