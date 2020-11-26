/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use std::rc::Rc;

use super::FontRequest;

pub struct Font {
    pub pixel_size: f32,
    request: FontRequest,
    text_canvas: web_sys::HtmlCanvasElement,
    canvas_context: web_sys::CanvasRenderingContext2d,
}

trait CSSFontShortHand {
    fn to_css_font_shorthand_string(&self, pixel_size: f32) -> String;
}

impl CSSFontShortHand for FontRequest {
    fn to_css_font_shorthand_string(&self, pixel_size: f32) -> String {
        let mut result = format!("{} {}px ", self.weight, pixel_size);

        if self.family.is_empty() {
            result.push_str("system-ui, -apple-system, sans-serif")
        } else {
            result.push('"');
            result.push_str(&self.family);
            result.push('"');
        }

        result
    }
}

impl Font {
    pub fn text_width(&self, text: &str) -> f32 {
        let text_metrics = self.canvas_context.measure_text(text).unwrap();
        text_metrics.width() as _
    }

    pub fn text_offset_for_x_position(&self, text: &str, x: f32) -> usize {
        // This is pretty cruel ...
        let mut last_width = 0.;
        for offset in text.char_indices().map(|(offset, _)| offset) {
            let new_width = self.text_width(&text[0..offset]);

            if new_width > last_width {
                let advance = new_width - last_width;

                if last_width + advance / 2. >= x {
                    return offset;
                }

                last_width = new_width;
            }
        }
        text.len()
    }

    pub fn height(&self) -> f32 {
        self.pixel_size
    }

    pub fn render_text<'a>(&'a self, text: &str) -> &'a web_sys::HtmlCanvasElement {
        self.canvas_context.set_font(&self.request.to_css_font_shorthand_string(self.pixel_size));
        let text_metrics = self.canvas_context.measure_text(text).unwrap();

        // ### HACK: Add padding to the canvas as web-sys doesn't have bindings to the font ascent/descent
        // properties and even then according to caniuse.com those aren't very well supported to begin with.
        // So this creates a slightly bigger texture that's wasting transparent pixels.
        let height = (1.5 * self.pixel_size) as u32;

        self.text_canvas.set_width(text_metrics.width() as _);
        self.text_canvas.set_height(height);
        self.text_canvas
            .style()
            .set_property("width", &format!("{}px", text_metrics.width()))
            .unwrap();
        self.text_canvas.style().set_property("height", &format!("{}px", self.pixel_size)).unwrap();

        // Re-apply after resize :(
        self.canvas_context.set_font(&self.request.to_css_font_shorthand_string(self.pixel_size));

        self.canvas_context.set_text_align("center");
        self.canvas_context.set_text_baseline("middle");
        self.canvas_context.set_fill_style(&wasm_bindgen::JsValue::from_str("transparent"));
        self.canvas_context.fill_rect(
            0.,
            0.,
            self.text_canvas.width() as _,
            self.text_canvas.height() as _,
        );

        self.canvas_context.set_fill_style(&wasm_bindgen::JsValue::from_str("rgb(0, 0, 0)"));
        self.canvas_context
            .fill_text(
                text,
                (self.text_canvas.width() / 2) as _,
                (self.text_canvas.height() / 2) as _,
            )
            .unwrap();

        &self.text_canvas
    }
}

pub struct PlatformFont(FontRequest);

impl PlatformFont {
    pub fn load(self: &Rc<Self>, pixel_size: f32) -> Font {
        let text_canvas = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .create_element("canvas")
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap();

        use wasm_bindgen::JsCast;
        let canvas_context = text_canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .unwrap();

        canvas_context.set_font(&self.0.to_css_font_shorthand_string(pixel_size));

        Font { pixel_size, request: self.0.clone(), text_canvas, canvas_context }
    }

    pub fn new_from_request(request: &FontRequest) -> Result<Rc<Self>, ()> {
        Ok(Rc::new(Self(request.clone())))
    }
}
