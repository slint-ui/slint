use std::hash::Hash;

#[derive(Clone)]
struct GlyphMetrics {
    advance: f32,
}

pub struct Font {
    pub pixel_size: f32,
    canvas_context: web_sys::CanvasRenderingContext2d,
}

impl Font {
    pub fn string_to_glyphs(&self, text: &str) -> Vec<u32> {
        text.chars().map(|ch| ch as u32).collect()
    }

    pub fn text_width(&self, text: &str) -> f32 {
        let text_metrics = self.canvas_context.measure_text(text).unwrap();
        text_metrics.width() as _
    }

    pub fn font_height(&self) -> f32 {
        self.pixel_size
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

        Ok(Font { pixel_size, canvas_context })
    }

    pub fn new_from_match(family: &str) -> Self {
        Self(family.to_owned())
    }
}
