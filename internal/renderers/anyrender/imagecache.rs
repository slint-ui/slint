// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Shared cache of Slint images converted to [`peniko::ImageData`].
//!
//! Converting a Slint image for anyrender means copying pixels (RGB
//! expansion, SVG rasterization, tile cropping) and wrapping them in a
//! [`peniko::Blob`]. Doing that per frame is wasteful in itself, but the
//! bigger cost is downstream: anyrender backends key their own image
//! resource caches on [`peniko::Blob::id`], so handing out a fresh `Blob`
//! every frame forces them to re-convert (and, for vello_cpu,
//! re-premultiply) the image on every fill. This cache returns the *same*
//! `ImageData` for the same source image across frames, making those
//! downstream caches effective.
//!
//! The caching model mirrors the femtovg renderer's texture cache: this
//! shared map deduplicates conversions across items, while the strong
//! references live in the per-item
//! [`ItemCache`](i_slint_core::item_rendering::ItemCache) held by
//! `AnyrenderSlintRenderer` (invalidated automatically when item
//! properties change). [`ImageConversionCache::drain`], called once per
//! frame, drops entries no item holds on to anymore.

use std::collections::HashMap;
use std::rc::Rc;

use i_slint_core::graphics::ImageCacheKey;

/// A cached, converted image. Cloning is cheap and shares the underlying
/// [`peniko::Blob`], keeping its id stable.
pub type SharedImageData = Rc<peniko::ImageData>;

/// Identifies which derived form of a source image an entry holds.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) enum ImageVariant {
    /// The image data as-is.
    Full,
    /// Rasterized at a specific pixel size (SVG).
    Sized { width: u32, height: u32 },
    /// A cropped sub-rectangle used as repeating tile. The source image
    /// dimensions are part of the key because the crop coordinates are in
    /// image-data space, whose content depends on the rasterized size for
    /// scalable sources (SVG).
    Tile { source_width: u32, source_height: u32, x: u32, y: u32, width: u32, height: u32 },
}

/// See the module documentation.
#[derive(Default)]
pub struct ImageConversionCache(HashMap<(ImageCacheKey, ImageVariant), SharedImageData>);

impl ImageConversionCache {
    /// Look up (or convert and store) the `variant` of the image identified
    /// by `key`. A `None` (or [`ImageCacheKey::Invalid`]) key means the
    /// source has no stable identity; the conversion then runs uncached.
    pub(crate) fn get_or_insert(
        &mut self,
        key: Option<ImageCacheKey>,
        variant: ImageVariant,
        convert: impl FnOnce() -> Option<peniko::ImageData>,
    ) -> Option<SharedImageData> {
        let Some(key) = key.filter(|key| *key != ImageCacheKey::Invalid) else {
            return convert().map(Rc::new);
        };
        match self.0.entry((key, variant)) {
            std::collections::hash_map::Entry::Occupied(entry) => Some(entry.get().clone()),
            std::collections::hash_map::Entry::Vacant(slot) => {
                let image_data = Rc::new(convert()?);
                slot.insert(image_data.clone());
                Some(image_data)
            }
        }
    }

    /// Drop entries that are no longer referenced outside the cache. Call
    /// once per rendered frame, like the femtovg renderer's texture cache
    /// drain. Tiles are kept as long as their source image survives: they
    /// are only referenced during the frame (no item holds them), but
    /// re-cropping them every frame would defeat the cache for tiled
    /// images.
    pub(crate) fn drain(&mut self) {
        self.0.retain(|(_, variant), image_data| {
            matches!(variant, ImageVariant::Tile { .. }) || Rc::strong_count(image_data) > 1
        });
        let live_sources: std::collections::HashSet<ImageCacheKey> = self
            .0
            .keys()
            .filter(|(_, variant)| !matches!(variant, ImageVariant::Tile { .. }))
            .map(|(key, _)| key.clone())
            .collect();
        self.0.retain(|(key, variant), _| {
            !matches!(variant, ImageVariant::Tile { .. }) || live_sources.contains(key)
        });
    }
}
