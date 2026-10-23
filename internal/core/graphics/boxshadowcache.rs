// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
This module contains a cache helper for caching box shadow textures.
*/

use alloc::boxed::Box;
use alloc::vec::Vec;
use std::cell::{Cell, RefCell};

use super::boxshadow::BoxShadowOptions;
use crate::items::ItemRc;
use crate::lengths::ScaleFactor;

/// Upper bound on the number of shadow textures kept alive by a [`BoxShadowCache`].
const MAX_CACHED_SHADOWS: usize = 16;

struct CacheEntry<ImageType> {
    image: Option<ImageType>,
    /// Value of the cache's access counter when this entry was last returned, for LRU eviction.
    last_used: u64,
}

/// Cache to hold box textures for given box shadow options.
pub struct BoxShadowCache<ImageType> {
    // Brushes have no total ordering, so the cache uses equality comparisons.
    entries: RefCell<Vec<(BoxShadowOptions, CacheEntry<ImageType>)>>,
    access_counter: Cell<u64>,
    /// Track if the window scale factor changes; used to clear the cache if necessary.
    window_scale_factor_tracker: core::pin::Pin<Box<crate::properties::PropertyTracker>>,
}

impl<ImageType> Default for BoxShadowCache<ImageType> {
    fn default() -> Self {
        Self {
            entries: Default::default(),
            access_counter: Default::default(),
            window_scale_factor_tracker: Box::pin(Default::default()),
        }
    }
}

impl<ImageType> BoxShadowCache<ImageType> {
    /// Removes all cached box shadow textures.
    pub fn clear(&self) {
        self.entries.borrow_mut().clear();
    }

    /// Clears the cache if the window's scale factor has changed since the last call, as the
    /// cached textures are rendered in physical pixels.
    pub fn clear_cache_if_scale_factor_changed(&self, window: &crate::api::Window) {
        if self.window_scale_factor_tracker.is_dirty() {
            self.window_scale_factor_tracker
                .as_ref()
                .evaluate_as_dependency_root(|| window.scale_factor());
            self.clear();
        }
    }
}

impl<ImageType: Clone> BoxShadowCache<ImageType> {
    /// Look up a box shadow texture for a given box shadow item, or create a new one if needed.
    pub fn get_box_shadow(
        &self,
        item_rc: &ItemRc,
        item_cache: &crate::item_rendering::ItemCache<Option<ImageType>>,
        box_shadow: std::pin::Pin<&crate::items::BoxShadow>,
        scale_factor: ScaleFactor,
        shadow_render_fn: impl FnOnce(&BoxShadowOptions) -> Option<ImageType>,
    ) -> Option<ImageType> {
        item_cache.get_or_update_cache_entry(item_rc, || {
            let shadow_options = BoxShadowOptions::new(item_rc, box_shadow, scale_factor)?;
            let mut entries = self.entries.borrow_mut();
            let stamp = self.access_counter.get() + 1;
            self.access_counter.set(stamp);
            if let Some((_, entry)) =
                entries.iter_mut().find(|(options, _)| *options == shadow_options)
            {
                entry.last_used = stamp;
                return entry.image.clone();
            }
            if entries.len() >= MAX_CACHED_SHADOWS {
                let oldest = entries
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, (_, entry))| entry.last_used)
                    .map(|(index, _)| index)
                    .unwrap();
                entries.swap_remove(oldest);
            }
            let image = shadow_render_fn(&shadow_options);
            entries.push((shadow_options, CacheEntry { image: image.clone(), last_used: stamp }));
            image
        })
    }
}
