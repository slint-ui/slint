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
    canvas_id: String,
) -> Result<(), wasm_bindgen::JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let c = sixtyfps_interpreter::load(source.to_owned(), &std::path::Path::new(""), &[])
        .map_err(|diag| js_sys::Error::new(&diag.to_string_vec().join("\n")))?;

    let component = c.clone().create(canvas_id);
    component.window().run(component.borrow(), component.root_item());
    Ok(())
}
