// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author Ian McFarlane <ian.mcfarlane@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::platform::PlatformError;
use wasm_bindgen::JsCast;

const WINIT_EVENT_LOOP_EXCEPTION: &str =
    "Using exceptions for control flow, don't mind me. This isn't actually an error!";

/// Returns whether or not the given value is an exception that looks like one that the winit even
/// loop would throw to yield out of wasm init(), indicating a successful initialization
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn slint_is_event_loop_exception(error: wasm_bindgen::JsValue) -> bool {
    error
        .dyn_ref::<js_sys::Error>()
        .is_some_and(|error| String::from(error.message()) == WINIT_EVENT_LOOP_EXCEPTION)
}

/// Get the current webpage's id="canvas" element if one exists
pub fn find_html_canvas() -> Result<Option<web_sys::HtmlCanvasElement>, PlatformError> {
    Ok(web_sys::window()
        .ok_or_else(|| "winit backend: Could not retrieve DOM window".to_string())?
        .document()
        .ok_or_else(|| "winit backend: Could not retrieve DOM document".to_string())?
        .get_element_by_id("canvas")
        .and_then(|canvas_elem| canvas_elem.dyn_into::<web_sys::HtmlCanvasElement>().ok()))
}

/// Draws the given message to an HTML canvas
pub fn canvas_draw_error_message(canvas: &web_sys::HtmlCanvasElement, message: &str) {
    const LINE_HEIGHT: f64 = 24.;

    // I don't believe that there's a way of disabling the 2D canvas.
    let Some(context_2d) = canvas
        .get_context("2d")
        .ok()
        .flatten()
        .and_then(|context| context.dyn_into::<web_sys::CanvasRenderingContext2d>().ok())
    else {
        return;
    };
    context_2d.set_font("20px serif");
    // We don't know if we're rendering on dark or white background, so choose a "color" in the middle for the text.
    context_2d.set_fill_style_str("red");

    let max_width = canvas.width() as f64;
    let mut y = 30.;
    let mut line = String::new();
    for word in message.split_whitespace() {
        let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
        let too_wide =
            context_2d.measure_text(&candidate).is_ok_and(|metrics| metrics.width() > max_width);
        if too_wide && !line.is_empty() {
            let _ = context_2d.fill_text(&line, 0., y);
            y += LINE_HEIGHT;
            line = word.to_string();
        } else {
            line = candidate;
        }
    }
    let _ = context_2d.fill_text(&line, 0., y);
}
