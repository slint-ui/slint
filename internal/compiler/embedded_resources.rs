// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use alloc::string::String;
use alloc::vec::Vec;

#[cfg(feature = "renderer-software")]
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

#[cfg(feature = "renderer-software")]
#[derive(Debug, Clone)]
pub struct Texture {
    pub total_size: Size,
    pub original_size: Size,
    pub rect: Rect,
    pub data: Vec<u8>,
    pub format: PixelFormat,
}

#[cfg(feature = "renderer-software")]
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

#[cfg(feature = "renderer-software")]
use i_slint_common::bitmapfont::Owned;
/// A pre-rendered glyph, see [`i_slint_common::bitmapfont::BitmapGlyph`].
#[cfg(feature = "renderer-software")]
pub type BitmapGlyph = i_slint_common::bitmapfont::BitmapGlyph<Owned>;
/// A set of pre-rendered glyphs, see [`i_slint_common::bitmapfont::BitmapGlyphs`].
#[cfg(feature = "renderer-software")]
pub type BitmapGlyphs = i_slint_common::bitmapfont::BitmapGlyphs<Owned>;
/// A font pre-rendered for the software renderer, see [`i_slint_common::bitmapfont::BitmapFont`].
#[cfg(feature = "renderer-software")]
pub type BitmapFont = i_slint_common::bitmapfont::BitmapFont<Owned>;

#[derive(Debug, Clone)]
pub enum EmbeddedResourcesKind {
    /// Only List the resource, do not actually embed it
    ListOnly,
    /// Just put the file content as a resource
    FileData,
    /// Encoded payload from a data URI (bytes, extension)
    DataUriPayload(Vec<u8>, String),
    /// The data has been processed in a texture
    #[cfg(feature = "renderer-software")]
    TextureData(Texture),
    /// A set of pre-rendered glyphs of a TrueType font
    #[cfg(feature = "renderer-software")]
    BitmapFontData(BitmapFont),
    /// The image of a Slint SC `@image-url()`, decoded at compile time. The
    /// Slint SC generator embeds its pixels in the generated code.
    #[cfg(feature = "slint-sc")]
    StaticPixels(image::RgbaImage),
}

#[derive(Debug, Clone)]
pub struct EmbeddedResources {
    /// `None` for in-memory payloads such as data URIs.
    pub path: Option<crate::source_path::SourcePath>,

    pub kind: EmbeddedResourcesKind,
}

/// Index of an [`EmbeddedResources`] entry in [`crate::object_tree::Document::embedded_file_resources`].
#[derive(
    Debug,
    Clone,
    Copy,
    Hash,
    PartialEq,
    Eq,
    derive_more::Into,
    derive_more::From,
    derive_more::Display,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EmbeddedResourcesIdx(pub usize);
