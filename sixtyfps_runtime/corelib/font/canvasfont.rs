/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use std::hash::Hash;

#[derive(Clone)]
struct GlyphMetrics {
    advance: f32,
}

pub struct Font {
    pub pixel_size: f32,
    font_family: String,
    text_canvas: web_sys::HtmlCanvasElement,
    canvas_context: web_sys::CanvasRenderingContext2d,
}

impl Font {
    pub fn text_width(&self, text: &str) -> f32 {
        let text_metrics = self.canvas_context.measure_text(text).unwrap();
        text_metrics.width() as _
    }

    pub fn font_height(&self) -> f32 {
        self.pixel_size
    }

    pub fn render_text<'a>(&'a self, text: &str) -> &'a web_sys::HtmlCanvasElement {
        let text_metrics = self.canvas_context.measure_text(text).unwrap();

        self.text_canvas.set_width(text_metrics.width() as _);
        self.text_canvas.set_height(self.pixel_size as _);
        self.text_canvas
            .style()
            .set_property("width", &format!("{}px", text_metrics.width()))
            .unwrap();
        self.text_canvas.style().set_property("height", &format!("{}px", self.pixel_size)).unwrap();

        // Re-apply after resize :(
        self.canvas_context.set_font(&format!("{}px \"{}\"", self.pixel_size, self.font_family));

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

#[derive(Clone)]
pub struct FontHandle(String);

impl Hash for FontHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

impl PartialEq for FontHandle {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl Eq for FontHandle {}

impl FontHandle {
    pub fn load(&self, pixel_size: f32) -> Result<Font, ()> {
        let font_family = &self.0;

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

        canvas_context.set_font(&format!("{}px \"{}\"", pixel_size, font_family));

        Ok(Font { pixel_size, font_family: font_family.clone(), text_canvas, canvas_context })
    }

    pub fn new_from_match(family: &str) -> Self {
        Self(family.to_owned())
    }
}
