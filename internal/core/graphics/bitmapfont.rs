// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::SharedString;
use crate::slice::Slice;
use alloc::rc::Rc;
use alloc::vec::Vec;

#[repr(C)]
#[derive(Debug)]
/// A pre-rendered glyph with the alpha map and associated metrics
pub struct BitmapGlyph {
    /// The starting x-coordinate for the glyph, relative to the base line
    /// This is a fixed point number that is shifted by 6 bits
    pub x: i16,
    /// The starting y-coordinate for the glyph, relative to the base line
    /// This is a fixed point number that is shifted by 6 bits
    pub y: i16,
    /// The width of the glyph in pixels
    pub width: i16,
    /// The height of the glyph in pixels
    pub height: i16,
    /// The horizontal distance to the next glyph
    /// This is a fixed point number that is shifted by 6 bits
    pub x_advance: i16,
    /// The 8-bit alpha map that's to be blended with the current text color
    /// or 8-bit signed distance field depending on `BitmapFont::sdf`
    pub data: Slice<'static, u8>,
}

#[repr(C)]
#[derive(Debug)]
/// A set of pre-rendered bitmap glyphs at a fixed pixel size
pub struct BitmapGlyphs {
    /// The font size in pixels at which the glyphs were pre-rendered. The boundaries of glyphs may exceed this
    /// size, if the font designer has chosen so. This is only used for matching.
    pub pixel_size: i16,
    /// The data of the pre-rendered glyphs
    pub glyph_data: Slice<'static, BitmapGlyph>,
}

#[repr(C)]
#[derive(Debug)]
/// A subset of an originally scalable font that's rendered ahead of time.
pub struct BitmapFont {
    /// The family name of the font
    pub family_name: Slice<'static, u8>,
    /// The code points that have a glyph, sorted.
    /// A code point's position in this slice is its index in the `glyph_data` of [`BitmapGlyphs`].
    pub character_map: Slice<'static, char>,
    /// The font supplied size of the em square.
    pub units_per_em: f32,
    /// The font ascent in design metrics (typically positive)
    pub ascent: f32,
    /// The font descent in design metrics (typically negative)
    pub descent: f32,
    /// The font's x-height.
    pub x_height: f32,
    /// The font's cap-height.
    pub cap_height: f32,
    /// A vector of pre-rendered glyph sets. Each glyph set must have the same number of glyphs
    /// as the character map.
    pub glyphs: Slice<'static, BitmapGlyphs>,
    /// The weight of the font in CSS units (400 is normal).
    pub weight: u16,
    /// Whether the type-face is rendered italic.
    pub italic: bool,
    /// Whether the format of the font is a signed distance field
    pub sdf: bool,
}

/// A [`BitmapGlyph`], but owned, see [`SharedBitmapFont`].
#[derive(Debug, Clone)]
pub struct SharedBitmapGlyph {
    /// See [`BitmapGlyph::x`]
    pub x: i16,
    /// See [`BitmapGlyph::y`]
    pub y: i16,
    /// See [`BitmapGlyph::width`]
    pub width: i16,
    /// See [`BitmapGlyph::height`]
    pub height: i16,
    /// See [`BitmapGlyph::x_advance`]
    pub x_advance: i16,
    /// See [`BitmapGlyph::data`]
    pub data: Rc<[u8]>,
}

/// [`BitmapGlyphs`], but owned, see [`SharedBitmapFont`].
#[derive(Debug, Clone)]
pub struct SharedBitmapGlyphs {
    /// See [`BitmapGlyphs::pixel_size`]
    pub pixel_size: i16,
    /// See [`BitmapGlyphs::glyph_data`]
    pub glyph_data: Vec<SharedBitmapGlyph>,
}

/// A [`BitmapFont`], but owned, for fonts that only exist at run time.
/// A renderer only keeps a weak reference to it, so the font goes away with its last owner.
#[derive(Debug, Clone)]
pub struct SharedBitmapFont {
    /// See [`BitmapFont::family_name`]
    pub family_name: SharedString,
    /// See [`BitmapFont::character_map`]
    pub character_map: Vec<char>,
    /// See [`BitmapFont::units_per_em`]
    pub units_per_em: f32,
    /// See [`BitmapFont::ascent`]
    pub ascent: f32,
    /// See [`BitmapFont::descent`]
    pub descent: f32,
    /// See [`BitmapFont::x_height`]
    pub x_height: f32,
    /// See [`BitmapFont::cap_height`]
    pub cap_height: f32,
    /// See [`BitmapFont::glyphs`]
    pub glyphs: Vec<SharedBitmapGlyphs>,
    /// See [`BitmapFont::weight`]
    pub weight: u16,
    /// See [`BitmapFont::italic`]
    pub italic: bool,
    /// See [`BitmapFont::sdf`]
    pub sdf: bool,
}
