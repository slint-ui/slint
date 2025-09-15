// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

pub use fontique;
pub use ttf_parser;

static COLLECTION: std::sync::LazyLock<Collection> = std::sync::LazyLock::new(|| Collection {
    inner: fontique::Collection::new(fontique::CollectionOptions {
        shared: true,
        ..Default::default()
    }),
    source_cache: fontique::SourceCache::new_shared(),
});

pub fn get_collection() -> Collection {
    COLLECTION.clone()
}

#[derive(Clone)]
pub struct Collection {
    inner: fontique::Collection,
    source_cache: fontique::SourceCache,
}

impl Collection {
    pub fn query<'a>(&'a mut self) -> fontique::Query<'a> {
        self.inner.query(&mut self.source_cache)
    }

    pub fn register_fonts(&mut self, data: impl Into<fontique::Blob<u8>>) -> usize {
        self.inner.register_fonts(data.into(), None).len()
    }
}

impl std::ops::Deref for Collection {
    type Target = fontique::Collection;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for Collection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// Font metrics in design space. Scale with desired pixel size and divided by units_per_em
/// to obtain pixel metrics.
#[derive(Clone)]
pub struct DesignFontMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub x_height: f32,
    pub cap_height: f32,
    pub units_per_em: f32,
}

impl DesignFontMetrics {
    pub fn new(font: &fontique::QueryFont) -> Self {
        let face = ttf_parser::Face::parse(font.blob.data(), font.index).unwrap();
        Self {
            ascent: face.ascender() as f32,
            descent: face.descender() as f32,
            x_height: face.x_height().unwrap_or_default() as f32,
            cap_height: face.capital_height().unwrap_or_default() as f32,
            units_per_em: face.units_per_em() as f32,
        }
    }
}
