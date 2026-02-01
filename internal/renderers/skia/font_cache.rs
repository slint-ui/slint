// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use clru::CLruCache;
use i_slint_common::sharedfontique::HashedBlob;
use i_slint_core::textlayout::sharedparley::parley;
use std::cell::RefCell;
use std::num::NonZeroUsize;

const FONT_CACHE_CAPACITY: NonZeroUsize = NonZeroUsize::new(64).unwrap();

pub struct FontCache {
    font_mgr: skia_safe::FontMgr,
    // Use HashedBlob in key to keep strong reference to font data blob,
    // preventing eviction from fontique's shared cache (see commit 30a03cf)
    fonts: CLruCache<(HashedBlob, u32), Option<skia_safe::Typeface>>,
}

impl Default for FontCache {
    fn default() -> Self {
        Self { font_mgr: skia_safe::FontMgr::new(), fonts: CLruCache::new(FONT_CACHE_CAPACITY) }
    }
}

impl FontCache {
    pub fn font(&mut self, font: &parley::FontData) -> Option<skia_safe::Typeface> {
        let key = (font.data.clone().into(), font.index);

        if let Some(cached_option) = self.fonts.peek(&key) {
            return cached_option.clone();
        }

        let typeface = self.load_typeface_internal(font);

        self.fonts.put(key, typeface.clone());

        typeface
    }

    fn load_typeface_internal(&self, font: &parley::FontData) -> Option<skia_safe::Typeface> {
        let typeface = self.font_mgr.new_from_data(
            font.data.as_ref(),
            if font.index > 0 { Some(font.index as _) } else { None },
        );

        // Due to  https://issues.skia.org/issues/310510989, fonts from true type collections
        // with an index > 0 fail to load on macOS. As a workaround, we manually extract the font from the
        // collection and load it as a single font.
        #[cfg(target_vendor = "apple")]
        if font.index > 0 && typeface.is_none() {
            if let Some(typeface) = read_fonts::CollectionRef::new(font.data.as_ref())
                .ok()
                .and_then(|ttc| ttc.get(font.index).ok())
                .map(|ttf| write_fonts::FontBuilder::new().copy_missing_tables(ttf).build())
                .and_then(|new_ttf| self.font_mgr.new_from_data(&new_ttf, None))
            {
                return Some(typeface);
            }
        }

        typeface
    }
}

thread_local! {
    pub static FONT_CACHE: RefCell<FontCache> = RefCell::new(Default::default())
}
