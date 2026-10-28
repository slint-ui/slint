// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Fonts pre-rendered for the software renderer.
//!
//! The same [`BitmapFont`] holds its data in statics when generated code embeds it,
//! or in `Vec`s when the compiler builds it, see [`Storage`].

use alloc::vec::Vec;
use core::fmt::Debug;
use core::ops::Deref;

/// Where a [`BitmapFont`] keeps its data.
pub trait Storage: Clone + Debug + 'static {
    /// The container of a sequence of `T`, which reads as a slice.
    type Slice<T: Clone + Debug + 'static>: Deref<Target = [T]> + Clone + Debug + Default;
}

/// The data in `Vec`s, as the compiler builds a font.
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OwnedStorage;

impl Storage for OwnedStorage {
    type Slice<T: Clone + Debug + 'static> = Vec<T>;
}

/// A pre-rendered glyph with the alpha map and associated metrics
#[repr(C)]
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BitmapGlyph<S: Storage> {
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
    pub data: S::Slice<u8>,
}

/// A set of pre-rendered glyphs at a given pixel size
#[repr(C)]
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BitmapGlyphs<S: Storage> {
    /// The font size in pixels at which the glyphs were pre-rendered. The boundaries of glyphs may exceed this
    /// size, if the font designer has chosen so. This is only used for matching.
    pub pixel_size: i16,
    /// The data of the pre-rendered glyphs
    pub glyph_data: S::Slice<BitmapGlyph<S>>,
}

/// A subset of an entire font that is pre-rendered at selected sizes.
#[repr(C)]
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BitmapFont<S: Storage> {
    /// The family name of the font, in UTF-8
    pub family_name: S::Slice<u8>,
    /// The code points that have a glyph, sorted.
    /// A code point's position in this slice is its index in the `glyph_data` of [`BitmapGlyphs`].
    pub character_map: S::Slice<char>,
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
    pub glyphs: S::Slice<BitmapGlyphs<S>>,
    /// The weight of the font in CSS units (400 is normal).
    pub weight: u16,
    /// Whether the type-face is rendered italic.
    pub italic: bool,
    /// Whether the format of the font is a signed distance field
    pub sdf: bool,
}
