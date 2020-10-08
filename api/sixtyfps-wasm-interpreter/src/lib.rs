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

use wasm_bindgen::prelude::*;

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

/// Compile and display the content of a string.
/// The HTML must contains a <canvas> element with the given `canvas_id`
/// where the result is gonna be rendered
#[wasm_bindgen]
pub fn instantiate_from_string(
    source: &str,
    base_url: &str,
    canvas_id: String,
) -> Result<(), JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let c = match sixtyfps_interpreter::load(
        source.to_owned(),
        &std::path::Path::new(base_url),
        &Default::default(),
    ) {
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

    let component = c.clone().create(canvas_id);
    component.window().run(component.borrow(), component.root_item());
    Ok(())
}
