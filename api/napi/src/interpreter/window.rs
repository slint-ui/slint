// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use crate::types::{JsPoint, JsSize};
use i_slint_core::window::WindowAdapterRc;
use slint_interpreter::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};

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
    #[napi(constructor)]
    pub fn new() -> napi::Result<Self> {
        Err(napi::Error::from_reason(
            "Window can only be created by using a Component.".to_string(),
        ))
    }

    #[napi]
    pub fn show(&self) -> napi::Result<()> {
        self.inner
            .window()
            .show()
            .map_err(|_| napi::Error::from_reason("Cannot show window.".to_string()))
    }

    #[napi]
    pub fn hide(&self) -> napi::Result<()> {
        self.inner
            .window()
            .hide()
            .map_err(|_| napi::Error::from_reason("Cannot hide window.".to_string()))
    }

    #[napi(getter)]
    pub fn is_visible(&self) -> bool {
        self.inner.window().is_visible()
    }

    #[napi(getter)]
    pub fn get_logical_position(&self) -> JsPoint {
        let pos = self.inner.window().position().to_logical(self.inner.window().scale_factor());
        JsPoint { x: pos.x as f64, y: pos.y as f64 }
    }

    #[napi(setter)]
    pub fn set_logical_position(&self, position: JsPoint) {
        self.inner
            .window()
            .set_position(LogicalPosition { x: position.x as f32, y: position.y as f32 });
    }

    #[napi(getter)]
    pub fn get_physical_position(&self) -> JsPoint {
        let pos = self.inner.window().position();
        JsPoint { x: pos.x as f64, y: pos.y as f64 }
    }

    #[napi(setter)]
    pub fn set_physical_position(&self, position: JsPoint) {
        self.inner.window().set_position(PhysicalPosition {
            x: position.x.floor() as i32,
            y: position.y.floor() as i32,
        });
    }

    #[napi(getter)]
    pub fn get_logical_size(&self) -> JsSize {
        let size = self.inner.window().size().to_logical(self.inner.window().scale_factor());
        JsSize { width: size.width as f64, height: size.height as f64 }
    }

    #[napi(setter)]
    pub fn set_logical_size(&self, size: JsSize) {
        self.inner.window().set_size(LogicalSize::from_physical(
            PhysicalSize { width: size.width.floor() as u32, height: size.height.floor() as u32 },
            self.inner.window().scale_factor(),
        ));
    }

    #[napi(getter)]
    pub fn get_physical_size(&self) -> JsSize {
        let size = self.inner.window().size();
        JsSize { width: size.width as f64, height: size.height as f64 }
    }

    #[napi(setter)]
    pub fn set_physical_size(&self, size: JsSize) {
        self.inner.window().set_size(PhysicalSize {
            width: size.width.floor() as u32,
            height: size.height.floor() as u32,
        });
    }
}
