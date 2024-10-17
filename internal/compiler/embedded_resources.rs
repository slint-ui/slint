// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[cfg(feature = "software-renderer")]
pub use resvg::tiny_skia::IntRect as Rect;

#[derive(Debug, Clone, Copy, Default)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, strum::Display)]
pub enum PixelFormat {
    // 24 bit RGB
    Rgb,
    // 32 bit RGBA
    Rgba,
    // 32 bit RGBA, but the RGB values are pre-multiplied by the alpha
    RgbaPremultiplied,
    // 8bit alpha map with a given color
    AlphaMap([u8; 3]),
}

#[cfg(feature = "software-renderer")]
#[derive(Debug, Clone)]
pub struct Texture {
    pub total_size: Size,
    pub original_size: Size,
    pub rect: Rect,
    pub data: Vec<u8>,
    pub format: PixelFormat,
}

#[cfg(feature = "software-renderer")]
impl Texture {
    pub fn new_empty() -> Self {
        Self {
            total_size: Size::default(),
            original_size: Size::default(),
            rect: Rect::from_xywh(0, 0, 1, 1).unwrap(),
            data: vec![0, 0, 0, 0],
            format: PixelFormat::Rgba,
        }
    }
}

#[cfg(feature = "software-renderer")]
#[derive(Debug, Clone, Default)]
pub struct BitmapGlyph {
    pub x: i16,
    pub y: i16,
    pub width: i16,
    pub height: i16,
    pub x_advance: i16,
    /// 8bit alpha map or SDF if `BitMapGlyphs`'s `sdf` is `true`.
    pub data: Vec<u8>,
}

#[cfg(feature = "software-renderer")]
#[derive(Debug, Clone)]
pub struct BitmapGlyphs {
    pub pixel_size: i16,
    pub glyph_data: Vec<BitmapGlyph>,
}

#[cfg(feature = "software-renderer")]
#[derive(Debug, Clone)]
pub struct CharacterMapEntry {
    pub code_point: char,
    pub glyph_index: u16,
}

#[cfg(feature = "software-renderer")]
#[derive(Debug, Clone)]
pub struct BitmapFont {
    pub family_name: String,
    /// map of available glyphs, sorted by char
    pub character_map: Vec<CharacterMapEntry>,
    pub units_per_em: f32,
    pub ascent: f32,
    pub descent: f32,
    pub x_height: f32,
    pub cap_height: f32,
    pub glyphs: Vec<BitmapGlyphs>,
    pub weight: u16,
    pub italic: bool,
    /// true when the font is represented as a signed distance field
    pub sdf: bool,
}

#[derive(Debug, Clone)]
pub enum EmbeddedResourcesKind {
    /// Only List the resource, do not actually embed it
    ListOnly,
    /// Just put the file content as a resource
    RawData,
    /// The data has been processed in a texture
    #[cfg(feature = "software-renderer")]
    TextureData(Texture),
    /// A set of pre-rendered glyphs of a TrueType font
    #[cfg(feature = "software-renderer")]
    BitmapFontData(BitmapFont),
}

#[derive(Debug, Clone)]
pub struct EmbeddedResources {
    /// unique integer id, that can be used by the generator for symbol generation.
    pub id: usize,

    pub kind: EmbeddedResourcesKind,
}
