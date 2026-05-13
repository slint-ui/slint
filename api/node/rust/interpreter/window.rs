// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::types::{SlintPoint, SlintSize};
use i_slint_core::window::WindowAdapterRc;
use slint_interpreter::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};

/// This type represents a window towards the windowing system, that's used to render the
/// scene of a component. It provides API to control windowing system specific aspects such
/// as the position on the screen.
#[napi(js_name = "Window")]
pub struct JsWindow {
    pub(crate) inner: WindowAdapterRc,
}

impl From<WindowAdapterRc> for JsWindow {
    fn from(instance: WindowAdapterRc) -> Self {
        Self { inner: instance }
    }
}

#[napi]
impl JsWindow {
    /// @hidden
    #[napi(constructor)]
    pub fn new() -> napi::Result<Self> {
        Err(napi::Error::from_reason(
            "Window can only be created by using a Component.".to_string(),
        ))
    }

    /// Shows the window on the screen. An additional strong reference on the
    /// associated component is maintained while the window is visible.
    #[napi]
    pub fn show(&self) -> napi::Result<()> {
        self.inner
            .window()
            .show()
            .map_err(|_| napi::Error::from_reason("Cannot show window.".to_string()))
    }

    /// Hides the window, so that it is not visible anymore.
    #[napi]
    pub fn hide(&self) -> napi::Result<()> {
        self.inner
            .window()
            .hide()
            .map_err(|_| napi::Error::from_reason("Cannot hide window.".to_string()))
    }

    /// Returns the visibility state of the window. This function can return false even if you previously called show()
    /// on it, for example if the user minimized the window.
    #[napi(getter, js_name = "visible")]
    pub fn is_visible(&self) -> bool {
        self.inner.window().is_visible()
    }

    /// Returns the logical position of the window on the screen.
    #[napi(getter)]
    pub fn get_logical_position(&self) -> SlintPoint {
        let pos = self.inner.window().position().to_logical(self.inner.window().scale_factor());
        SlintPoint { x: pos.x as f64, y: pos.y as f64 }
    }

    /// Sets the logical position of the window on the screen.
    #[napi(setter)]
    pub fn set_logical_position(&self, position: SlintPoint) {
        self.inner
            .window()
            .set_position(LogicalPosition { x: position.x as f32, y: position.y as f32 });
    }

    /// Returns the physical position of the window on the screen.
    #[napi(getter)]
    pub fn get_physical_position(&self) -> SlintPoint {
        let pos = self.inner.window().position();
        SlintPoint { x: pos.x as f64, y: pos.y as f64 }
    }

    /// Sets the physical position of the window on the screen.
    #[napi(setter)]
    pub fn set_physical_position(&self, position: SlintPoint) {
        self.inner.window().set_position(PhysicalPosition {
            x: position.x.floor() as i32,
            y: position.y.floor() as i32,
        });
    }

    /// Returns the logical size of the window on the screen,
    #[napi(getter)]
    pub fn get_logical_size(&self) -> SlintSize {
        let size = self.inner.window().size().to_logical(self.inner.window().scale_factor());
        SlintSize { width: size.width as f64, height: size.height as f64 }
    }

    /// Sets the logical size of the window on the screen,
    #[napi(setter)]
    pub fn set_logical_size(&self, size: SlintSize) {
        self.inner.window().set_size(LogicalSize::from_physical(
            PhysicalSize { width: size.width.floor() as u32, height: size.height.floor() as u32 },
            self.inner.window().scale_factor(),
        ));
    }

    /// Returns the physical size of the window on the screen,
    #[napi(getter)]
    pub fn get_physical_size(&self) -> SlintSize {
        let size = self.inner.window().size();
        SlintSize { width: size.width as f64, height: size.height as f64 }
    }

    /// Sets the logical size of the window on the screen,
    #[napi(setter)]
    pub fn set_physical_size(&self, size: SlintSize) {
        self.inner.window().set_size(PhysicalSize {
            width: size.width.floor() as u32,
            height: size.height.floor() as u32,
        });
    }

    /// Issues a request to the windowing system to re-render the contents of the window.
    #[napi(js_name = "requestRedraw")]
    pub fn request_redraw(&self) {
        self.inner.request_redraw();
    }

    /// Returns if the window is currently fullscreen
    #[napi(getter)]
    pub fn get_fullscreen(&self) -> bool {
        self.inner.window().is_fullscreen()
    }

    /// Set or unset the window to display fullscreen.
    #[napi(setter)]
    pub fn set_fullscreen(&self, enable: bool) {
        self.inner.window().set_fullscreen(enable)
    }

    /// Returns if the window is currently maximized
    #[napi(getter)]
    pub fn get_maximized(&self) -> bool {
        self.inner.window().is_maximized()
    }

    /// Maximize or unmaximize the window.
    #[napi(setter)]
    pub fn set_maximized(&self, maximized: bool) {
        self.inner.window().set_maximized(maximized)
    }

    /// Returns if the window is currently minimized
    #[napi(getter)]
    pub fn get_minimized(&self) -> bool {
        self.inner.window().is_minimized()
    }

    /// Minimize or unminimze the window.
    #[napi(setter)]
    pub fn set_minimized(&self, minimized: bool) {
        self.inner.window().set_minimized(minimized)
    }
}
