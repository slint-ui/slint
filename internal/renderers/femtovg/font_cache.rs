// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cspell:ignore Noto fontconfig

use femtovg::TextContext;
use i_slint_core::textlayout::sharedparley::parley;
use std::cell::RefCell;
use std::collections::HashMap;

pub struct FontCache {
    pub(crate) text_context: femtovg::TextContext,
    fonts: HashMap<(u64, u32), femtovg::FontId>,
}

impl Default for FontCache {
    fn default() -> Self {
        let text_context = TextContext::default();
        Self { text_context, fonts: Default::default() }
    }
}

impl FontCache {
    pub fn font(&mut self, font: &parley::FontData) -> femtovg::FontId {
        let text_context = self.text_context.clone();

        *self.fonts.entry((font.data.id(), font.index)).or_insert_with(move || {
            text_context.add_shared_font_with_index(font.data.clone(), font.index).unwrap()
        })
    }
}

thread_local! {
    pub static FONT_CACHE: RefCell<FontCache> = RefCell::new(Default::default())
}
