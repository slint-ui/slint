// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

pub use fontique;

pub static COLLECTION: once_cell::sync::OnceCell<Collection> = once_cell::sync::OnceCell::new();

pub fn get_collection() -> Collection {
    COLLECTION.get_or_init(Default::default).clone()
}

#[derive(Clone)]
pub struct Collection {
    inner: fontique::Collection,
    source_cache: fontique::SourceCache,
}

impl Default for Collection {
    fn default() -> Self {
        Self {
            inner: fontique::Collection::new(fontique::CollectionOptions {
                shared: true,
                ..Default::default()
            }),
            source_cache: fontique::SourceCache::new_shared(),
        }
    }
}

impl Collection {
    pub fn query<'a>(&'a mut self) -> fontique::Query<'a> {
        self.inner.query(&mut self.source_cache)
    }

    pub fn register_fonts(&mut self, data: impl Into<fontique::Blob<u8>>) -> usize {
        self.inner.register_fonts(data.into(), None).len()
    }
}
