// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore ltrb xywh

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

/// An integer rectangle, with the accessors of `tiny_skia::IntRect`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

impl Rect {
    /// `None` if the rectangle is empty.
    pub fn from_xywh(x: i32, y: i32, width: u32, height: u32) -> Option<Self> {
        (width > 0 && height > 0).then_some(Self { x, y, width, height })
    }

    /// `None` if the rectangle is empty.
    pub fn from_ltrb(left: i32, top: i32, right: i32, bottom: i32) -> Option<Self> {
        Self::from_xywh(
            left,
            top,
            u32::try_from(right - left).ok()?,
            u32::try_from(bottom - top).ok()?,
        )
    }

    pub fn x(&self) -> i32 {
        self.x
    }

    pub fn y(&self) -> i32 {
        self.y
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

#[derive(Clone, Copy, Debug, strum::Display)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Texture {
    pub total_size: Size,
    pub original_size: Size,
    pub rect: Rect,
    pub data: Vec<u8>,
    pub format: PixelFormat,
}

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

use i_slint_common::bitmapfont::Owned;
/// A pre-rendered glyph, see [`i_slint_common::bitmapfont::BitmapGlyph`].
pub type BitmapGlyph = i_slint_common::bitmapfont::BitmapGlyph<Owned>;
/// A set of pre-rendered glyphs, see [`i_slint_common::bitmapfont::BitmapGlyphs`].
pub type BitmapGlyphs = i_slint_common::bitmapfont::BitmapGlyphs<Owned>;
/// A font pre-rendered for the software renderer, see [`i_slint_common::bitmapfont::BitmapFont`].
pub type BitmapFont = i_slint_common::bitmapfont::BitmapFont<Owned>;

/// An embedded resource, as the compilation unit carries it for the interpreter.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EmbeddedResource {
    /// A resource the code generators embed from the file, which the unit doesn't carry.
    #[default]
    NotInUnit,
    Texture(Texture),
    BitmapFont(BitmapFont),
    /// A resource that the interpreter holds under this key, for a unit read with
    /// [`crate::llr::serialize::from_bytes_with_registered_resources()`].
    Registered(u64),
}

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
