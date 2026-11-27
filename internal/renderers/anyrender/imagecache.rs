// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Slint images converted to [`peniko::ImageData`], shared through the image cache.
//!
//! Converting a Slint image for anyrender means copying pixels (RGB
//! expansion, SVG rasterization, tile cropping) and wrapping them in a
//! [`peniko::Blob`]. Doing that per frame is wasteful in itself, but the
//! bigger cost is downstream: anyrender backends key their own image
//! resource caches on [`peniko::Blob::id`], so handing out a fresh `Blob`
//! every frame forces them to re-convert (and, for vello_cpu,
//! re-premultiply) the image on every fill. The image cache returns the
//! *same* `ImageData` for the same source image across frames, making those
//! downstream caches effective.

use std::rc::Rc;

use i_slint_core::SlintContext;
use i_slint_core::graphics::{ImageCacheKey, cache};

/// A cached, converted image. Cloning is cheap and shares the underlying
/// [`peniko::Blob`], keeping its id stable.
pub type SharedImageData = Rc<peniko::ImageData>;

/// Identifies which derived form of a source image an entry holds.
#[derive(PartialEq, Eq, Hash)]
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

/// Returns the `variant` of the image identified by `key`, made with `convert`
/// unless it's in the image cache of `context`. Without a key, nothing is cached.
pub(crate) fn converted(
    context: &SlintContext,
    key: Option<ImageCacheKey>,
    variant: ImageVariant,
    convert: impl FnOnce() -> Option<peniko::ImageData>,
) -> Option<SharedImageData> {
    let convert = || {
        let image_data = convert()?;
        let weight = image_data.data.len();
        Some((image_data, weight))
    };
    match key {
        Some(key) => cache::cached_variant(context, &key, variant, convert),
        None => convert().map(|(image_data, _)| Rc::new(image_data)),
    }
}
