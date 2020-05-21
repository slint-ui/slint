use super::text::GLFont;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

#[derive(Default)]
struct FontMatch {
    fonts_per_pixel_size: Vec<Rc<RefCell<GLFont>>>,
}

#[derive(Clone)]
struct FontHandle(font_kit::handle::Handle);

impl Hash for FontHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match &self.0 {
            font_kit::handle::Handle::Path { path, font_index } => {
                path.hash(state);
                font_index.hash(state);
            }
            font_kit::handle::Handle::Memory { bytes, font_index } => {
                bytes.hash(state);
                font_index.hash(state);
            }
        }
    }
}

impl PartialEq for FontHandle {
    fn eq(&self, other: &Self) -> bool {
        match &self.0 {
            font_kit::handle::Handle::Path { path, font_index } => match &other.0 {
                font_kit::handle::Handle::Path {
                    path: other_path,
                    font_index: other_font_index,
                } => path.eq(other_path) && font_index.eq(other_font_index),
                _ => false,
            },
            font_kit::handle::Handle::Memory { bytes, font_index } => match &other.0 {
                font_kit::handle::Handle::Memory {
                    bytes: other_bytes,
                    font_index: other_font_index,
                } => bytes.eq(other_bytes) && font_index.eq(other_font_index),
                _ => false,
            },
        }
    }
}

impl Eq for FontHandle {}

impl FontHandle {
    fn load(&self) -> Result<font_kit::font::Font, font_kit::error::FontLoadingError> {
        self.0.load()
    }
}

impl From<font_kit::handle::Handle> for FontHandle {
    fn from(h: font_kit::handle::Handle) -> Self {
        Self(h)
    }
}

#[derive(Default)]
pub(crate) struct FontCache {
    loaded_fonts: HashMap<FontHandle, FontMatch>,
}

impl FontCache {
    pub fn find_font(&mut self, family: &str, pixel_size: f32) -> Rc<RefCell<GLFont>> {
        let family_name = if family.len() == 0 {
            font_kit::family_name::FamilyName::SansSerif
        } else {
            font_kit::family_name::FamilyName::Title(family.into())
        };

        let handle: FontHandle = font_kit::source::SystemSource::new()
            .select_best_match(
                &[family_name, font_kit::family_name::FamilyName::SansSerif],
                &font_kit::properties::Properties::new(),
            )
            .unwrap()
            .into();

        let font_match = self.loaded_fonts.entry(handle.clone()).or_insert(FontMatch::default());

        font_match
            .fonts_per_pixel_size
            .iter()
            .find_map(|gl_font| {
                if gl_font.borrow().pixel_size == pixel_size {
                    Some(gl_font.clone())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                let fnt = Rc::new(RefCell::new(GLFont::new(handle.load().unwrap(), pixel_size)));
                font_match.fonts_per_pixel_size.push(fnt.clone());
                fnt
            })
    }
}
