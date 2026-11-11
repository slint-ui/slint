// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use base64::Engine;
use i_slint_core::api::{PhysicalSize, Window, WindowSize};
use i_slint_core::platform::{Platform, Renderer, WindowAdapter, WindowEvent};
use i_slint_renderer_software::{RepaintBufferType, SoftwareRenderer};
use slint_interpreter::ComponentHandle;
use wasm_bindgen::prelude::*;

use crate::{ResourceLoaderFunction, WrappedCompiledComp};

struct SoftwarePlatform;

impl Platform for SoftwarePlatform {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn WindowAdapter>, i_slint_core::api::PlatformError> {
        Ok(Rc::new_cyclic(|window| SoftwareWindow {
            window: Window::new(window.clone()),
            renderer: SoftwareRenderer::new_with_repaint_buffer_type(RepaintBufferType::NewBuffer),
            size: Cell::new(PhysicalSize::default()),
        }))
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::ZERO
    }
}

struct SoftwareWindow {
    window: Window,
    renderer: SoftwareRenderer,
    size: Cell<PhysicalSize>,
}

impl WindowAdapter for SoftwareWindow {
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
    i_slint_core::platform::set_platform(Box::new(SoftwarePlatform))
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

pub(super) fn set_image_loader(
    compiler: &mut slint_interpreter::Compiler,
    loader: Option<&ResourceLoaderFunction>,
) -> Rc<RefCell<Vec<(String, String)>>> {
    let errors = Rc::new(RefCell::new(Vec::new()));
    let loader = loader.map(|loader| js_sys::Function::from(JsValue::from(loader)));
    let load_errors = errors.clone();
    compiler.compiler_configuration(i_slint_core::InternalToken).resource_url_mapper =
        Some(Rc::new(move |url| {
            if matches!(url.scheme(), "builtin" | "data") {
                return Box::pin(async { None });
            }
            let url = url.clone();
            let loader = loader.clone();
            let errors = load_errors.clone();
            Box::pin(async move {
                let result = async {
                    let loader = loader.as_ref().ok_or_else(|| {
                        JsValue::from_str("Provide a resource loader for external images")
                    })?;
                    let promise =
                        loader.call1(&JsValue::UNDEFINED, &JsValue::from_str(url.as_str()))?;
                    let bytes =
                        wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise))
                            .await?;
                    let bytes = js_sys::Uint8Array::new(&bytes).to_vec();
                    let extension = url.path().rsplit('.').next().unwrap_or_default();
                    i_slint_core::graphics::Image::load_from_data(&bytes, Some(extension))
                        .map_err(|error| JsValue::from_str(&error.to_string()))?;
                    let mime_type =
                        i_slint_core::graphics::image_mime_type_from_extension(extension)
                            .unwrap_or("image/png");
                    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
                    format!("data:{mime_type};base64,{encoded}")
                        .parse()
                        .map_err(|error| JsValue::from_str(&format!("{error}")))
                }
                .await;
                match result {
                    Ok(mapped) => Some(mapped),
                    Err(error) => {
                        let detail = error
                            .as_string()
                            .or_else(|| {
                                error.dyn_ref::<js_sys::Error>().map(|error| error.message().into())
                            })
                            .unwrap_or_else(|| format!("{error:?}"));
                        let message = format!("Cannot load image {url}: {detail}");
                        errors.borrow_mut().push((url.to_string(), message));
                        None
                    }
                }
            })
        }));
    errors
}
