// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

pub use fontique;

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
