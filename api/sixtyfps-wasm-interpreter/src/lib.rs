/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! This wasm library can be loaded from JS to load and display the content of .60 files
#![cfg(target_arch = "wasm32")]

use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

/// Compile the content of a string.
///
/// Returns a promise to a compiled component which can be run with ".run()"
#[wasm_bindgen]
pub async fn compile_from_string(
    source: String,
    base_url: String,
    optional_resolve_import_callback: Option<js_sys::Function>,
    optional_import_callback: Option<js_sys::Function>,
) -> Result<WrappedCompiledComp, JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let mut config = sixtyfps_interpreter::new_compiler_configuration();

    if let (Some(resolver_callback), Some(load_callback)) =
        (optional_resolve_import_callback, optional_import_callback)
    {
        config.resolve_import_fallback = Some(Box::new(move |file_name| {
            resolver_callback
                .clone()
                .call1(&JsValue::UNDEFINED, &file_name.into())
                .ok()
                .and_then(|path_value| path_value.as_string())
        }));
        config.open_import_fallback = Some(Box::new(move |file_name| {
            Box::pin({
                let load_callback = load_callback.clone();
                async move {
                    let result = load_callback.call1(&JsValue::UNDEFINED, &file_name.into());
                    let promise: js_sys::Promise = result.unwrap().into();
                    let future = wasm_bindgen_futures::JsFuture::from(promise);
                    match future.await {
                        Ok(js_ok) => Ok(js_ok.as_string().unwrap_or_default()),
                        Err(js_err) => Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            js_err.as_string().unwrap_or_default(),
                        )),
                    }
                }
            })
        }));
    }

    let c = match sixtyfps_interpreter::load(source, base_url.into(), config).await {
        (Ok(c), ..) => {
            //TODO: warnings.print();
            c
        }
        (Err(()), errors) => {
            let line_key = JsValue::from_str("lineNumber");
            let column_key = JsValue::from_str("columnNumber");
            let message_key = JsValue::from_str("message");
            let file_key = JsValue::from_str("fileName");
            let level_key = JsValue::from_str("level");
            let mut error_as_string = String::new();
            let array = js_sys::Array::new();
            for diag in errors.into_iter() {
                let filename_js = JsValue::from_str(&diag.current_path.display().to_string());
                for d in &diag.inner {
                    if !error_as_string.is_empty() {
                        error_as_string.push_str("\n");
                    }
                    use std::fmt::Write;

                    let (line, column) = d.line_column(&diag);
                    write!(&mut error_as_string, "{}:{}:{}", diag.current_path.display(), line, d)
                        .unwrap();
                    let error_obj = js_sys::Object::new();
                    js_sys::Reflect::set(
                        &error_obj,
                        &message_key,
                        &JsValue::from_str(&d.to_string()),
                    )?;
                    js_sys::Reflect::set(&error_obj, &line_key, &JsValue::from_f64(line as f64))?;
                    js_sys::Reflect::set(
                        &error_obj,
                        &column_key,
                        &JsValue::from_f64(column as f64),
                    )?;
                    js_sys::Reflect::set(&error_obj, &file_key, &filename_js)?;
                    js_sys::Reflect::set(
                        &error_obj,
                        &level_key,
                        &JsValue::from_f64(d.level() as i8 as f64),
                    )?;
                    array.push(&error_obj);
                }
            }

            let error = js_sys::Error::new(&error_as_string);
            js_sys::Reflect::set(&error, &JsValue::from_str("errors"), &array)?;
            return Err((**error).clone());
        }
    };

    Ok(WrappedCompiledComp(c))
}

#[wasm_bindgen]
pub struct WrappedCompiledComp(Rc<sixtyfps_interpreter::ComponentDescription>);

#[wasm_bindgen]
impl WrappedCompiledComp {
    /// Run this compiled component in a canvas.
    /// The HTML must contains a <canvas> element with the given `canvas_id`
    /// where the result is gonna be rendered
    #[wasm_bindgen]
    pub fn run(&self, canvas_id: String) {
        let component = self.0.clone().create(canvas_id);
        component.window().show();
        sixtyfps_interpreter::run_event_loop();
    }
}

/// Downloads the font from the specified url and registers it as a font
/// for use in text elements.
#[wasm_bindgen]
pub async fn register_font(url: String) -> Result<(), JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let mut opts = RequestInit::new();
    opts.method("GET");
    opts.mode(RequestMode::Cors);

    let request = Request::new_with_str_and_init(&url, &opts)?;

    let window = web_sys::window().unwrap();
    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;

    let resp: Response = resp_value.dyn_into().unwrap();

    let data = js_sys::Uint8Array::new(&JsFuture::from(resp.array_buffer()?).await?);
    let data = data.to_vec();

    sixtyfps_interpreter::register_font_from_memory(&data).unwrap();

    Ok(())
}
