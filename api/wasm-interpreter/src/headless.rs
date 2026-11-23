// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{cell::Cell, rc::Rc};

use i_slint_core::api::{PhysicalSize, Window, WindowSize};
use i_slint_core::platform::{Platform, Renderer, WindowAdapter, WindowEvent};
use i_slint_renderer_anyrender::VelloCpuRenderer;
use slint_interpreter::ComponentHandle;
use wasm_bindgen::prelude::*;

use crate::WrappedCompiledComp;

struct HeadlessPlatform;

impl Platform for HeadlessPlatform {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn WindowAdapter>, i_slint_core::api::PlatformError> {
        Ok(Rc::new_cyclic(|window| HeadlessWindow {
            window: Window::new(window.clone()),
            renderer: VelloCpuRenderer::new_vello_cpu(),
            size: Cell::new(PhysicalSize::default()),
        }))
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::ZERO
    }
}

struct HeadlessWindow {
    window: Window,
    renderer: VelloCpuRenderer,
    size: Cell<PhysicalSize>,
}

impl WindowAdapter for HeadlessWindow {
    fn window(&self) -> &Window {
        &self.window
    }

    fn renderer(&self) -> &dyn Renderer {
        &self.renderer
    }

    fn size(&self) -> PhysicalSize {
        self.size.get()
    }

    fn set_size(&self, size: WindowSize) {
        self.size.set(size.to_physical(self.window.scale_factor()));
        self.window.dispatch_event(WindowEvent::Resized {
            size: size.to_logical(self.window.scale_factor()),
        });
    }

    fn update_window_properties(&self, properties: i_slint_core::window::WindowProperties<'_>) {
        if self.size.get().width == 0 {
            self.set_size(properties.layout_constraints().preferred.into());
        }
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn log_error(message: &str);
}

#[wasm_bindgen(start)]
pub fn init() -> Result<(), JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
    i_slint_core::platform::set_platform(Box::new(HeadlessPlatform))
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    i_slint_core::with_existing_context(|context| {
        context.set_log_message_handler(Some(Box::new(|message| {
            log_error(&message.message_arguments().to_string());
        })));
    })?;
    Ok(())
}

#[wasm_bindgen]
pub struct RenderedFrame {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[wasm_bindgen]
impl RenderedFrame {
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[wasm_bindgen(getter)]
    pub fn pixels(&self) -> Vec<u8> {
        self.pixels.clone()
    }
}

#[wasm_bindgen]
impl WrappedCompiledComp {
    pub fn render(&self) -> Result<RenderedFrame, JsValue> {
        let instance = self.0.create().map_err(|error| JsValue::from_str(&error.to_string()))?;
        let snapshot = instance.show().and_then(|()| instance.window().take_snapshot());
        let hidden = instance.hide();
        let snapshot = snapshot.map_err(|error| JsValue::from_str(&error.to_string()))?;
        hidden.map_err(|error| JsValue::from_str(&error.to_string()))?;
        Ok(RenderedFrame {
            width: snapshot.width(),
            height: snapshot.height(),
            pixels: snapshot.as_bytes().to_vec(),
        })
    }
}
