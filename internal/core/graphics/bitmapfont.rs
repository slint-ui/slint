// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::slice::Slice;
pub use i_slint_common::bitmapfont::{OwnedStorage, Storage};

/// The data in statics, as generated code embeds a font.
#[derive(Clone, Copy, Debug, Default)]
pub struct StaticStorage;

impl Storage for StaticStorage {
    type Slice<T: Clone + core::fmt::Debug + 'static> = Slice<'static, T>;
}

/// A pre-rendered glyph with the alpha map and associated metrics
///
/// cbindgen:ignore
pub type BitmapGlyph<S = StaticStorage> = i_slint_common::bitmapfont::BitmapGlyph<S>;
/// A set of pre-rendered glyphs at a given pixel size
///
/// cbindgen:ignore
pub type BitmapGlyphs<S = StaticStorage> = i_slint_common::bitmapfont::BitmapGlyphs<S>;
/// A subset of an entire font that is pre-rendered at selected sizes.
///
/// cbindgen:ignore
pub type BitmapFont<S = StaticStorage> = i_slint_common::bitmapfont::BitmapFont<S>;
