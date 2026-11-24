// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
The cache of what's loaded, parsed, or decoded from images, one per [`SlintContext`].

[`Image`](super::Image) values only say where their encoded data is.
Loading, parsing an SVG, and decoding pixels go through [`cached`],
keyed by that location and the type of what's made from it.
Renderers can keep what they make from the pixels there too.
*/

use super::{ImageCacheKey, SharedImageBuffer};
use crate::SlintContext;
use alloc::rc::Rc;
use core::any::{Any, TypeId};
use std::collections::HashMap;

/// Bytes of entries that nothing outside the cache holds, that the cache keeps
/// so that an image shown again isn't loaded again.
const UNUSED_BUDGET: usize = 5 * 1024 * 1024;

/// What's cached when making an entry failed, so that it isn't tried again on every use.
struct Failed;

struct Entry {
    /// The `T` of the key's `TypeId`, or [`Failed`].
    data: Rc<dyn Any>,
    weight: usize,
    last_use: u64,
}

#[derive(Default)]
struct Entries {
    entries: HashMap<(ImageCacheKey, TypeId), Entry>,
    total_weight: usize,
    clock: u64,
}

impl Entries {
    fn insert(&mut self, key: (ImageCacheKey, TypeId), entry: Entry) {
        self.total_weight += entry.weight;
        if let Some(old) = self.entries.insert(key, entry) {
            self.total_weight -= old.weight;
        }
    }

    fn trim(&mut self) {
        if self.total_weight <= UNUSED_BUDGET {
            return;
        }
        let is_unused = |entry: &Entry| Rc::strong_count(&entry.data) == 1;
        let mut unused_weight: usize =
            self.entries.values().filter(|entry| is_unused(entry)).map(|entry| entry.weight).sum();
        if unused_weight <= UNUSED_BUDGET {
            return;
        }
        let mut unused: alloc::vec::Vec<_> = self
            .entries
            .iter()
            .filter(|(_, entry)| is_unused(entry))
            .map(|(key, entry)| (entry.last_use, entry.weight, key.clone()))
            .collect();
        unused.sort_unstable_by_key(|(last_use, ..)| *last_use);
        for (_, weight, key) in unused {
            self.entries.remove(&key);
            self.total_weight -= weight;
            unused_weight -= weight;
            if unused_weight <= UNUSED_BUDGET {
                break;
            }
        }
    }
}

/// The image cache of a [`SlintContext`].
#[derive(Default)]
pub(crate) struct ImageCache(core::cell::RefCell<Entries>);

impl ImageCache {
    fn cached<T: 'static>(
        &self,
        key: &ImageCacheKey,
        create: impl FnOnce() -> Option<(T, usize)>,
    ) -> Option<Rc<T>> {
        let cache_key = (key.clone(), TypeId::of::<T>());
        let cached = {
            let mut entries = self.0.borrow_mut();
            entries.clock += 1;
            let clock = entries.clock;
            entries.entries.get_mut(&cache_key).map(|entry| {
                entry.last_use = clock;
                entry.data.clone().downcast::<T>().ok()
            })
        };
        if let Some(cached) = cached {
            return cached;
        }
        // `create` may use the cache for other entries, so the cache isn't borrowed while it runs.
        let created = create().map(|(data, weight)| (Rc::new(data), weight));
        let (data, weight): (Rc<dyn Any>, _) = match &created {
            Some((data, weight)) => (data.clone(), *weight),
            None => (Rc::new(Failed), core::mem::size_of::<Entry>()),
        };
        let weight = weight + held_bytes(key);
        let mut entries = self.0.borrow_mut();
        entries.clock += 1;
        let last_use = entries.clock;
        entries.insert(cache_key, Entry { data, weight, last_use });
        entries.trim();
        created.map(|(data, _)| data)
    }

    /// Drops the least recently used entries that nothing holds, beyond the budget.
    pub(crate) fn trim(&self) {
        self.0.borrow_mut().trim();
    }
}

/// Returns the `T` made from the image at `key`,
/// or makes it with `create`, which returns it with its size in bytes.
///
/// The result is shared through the cache of `context` for as long as anything holds it.
/// Afterwards, the cache keeps the most recently used entries within a budget.
/// A failure is cached too. Without a context, nothing is cached.
pub fn cached<T: 'static>(
    context: Option<&SlintContext>,
    key: &ImageCacheKey,
    create: impl FnOnce() -> Option<(T, usize)>,
) -> Option<Rc<T>> {
    match context {
        Some(context) => context.0.image_cache.cached(key, create),
        None => create().map(|(data, _)| Rc::new(data)),
    }
}

/// Returns the bytes of encoded data that `key` holds.
fn held_bytes(key: &ImageCacheKey) -> usize {
    match key {
        ImageCacheKey::Data(data) => data.len(),
        #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
        ImageCacheKey::URL(url) => url.len(),
        _ => 0,
    }
}

/// Returns the size of the pixels of `buffer` in bytes.
pub(super) fn buffer_weight(buffer: &SharedImageBuffer) -> usize {
    match buffer {
        SharedImageBuffer::RGB8(pixels) => pixels.as_bytes().len(),
        SharedImageBuffer::RGBA8(pixels) => pixels.as_bytes().len(),
        SharedImageBuffer::RGBA8Premultiplied(pixels) => pixels.as_bytes().len(),
        #[cfg(feature = "image-pixel-format-rgb565")]
        SharedImageBuffer::RGB565(pixels) => pixels.as_bytes().len(),
        #[cfg(feature = "image-pixel-format-gray8")]
        SharedImageBuffer::Gray8(pixels) => pixels.as_bytes().len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: usize) -> ImageCacheKey {
        static DATA: [u8; 8] = [0; 8];
        ImageCacheKey::EmbeddedData(by_address::ByAddress(crate::slice::Slice::from_slice(
            &DATA[n..=n],
        )))
    }

    fn is_cached(cache: &ImageCache, n: usize) -> bool {
        cache.0.borrow().entries.contains_key(&(key(n), TypeId::of::<u32>()))
    }

    #[test]
    fn shares_and_evicts_unused_beyond_budget() {
        let cache = ImageCache::default();
        let third = UNUSED_BUDGET / 3 + 1;
        let first = cache.cached(&key(1), || Some((1u32, third))).unwrap();
        cache.cached(&key(2), || Some((2u32, third))).unwrap();
        assert!(Rc::ptr_eq(&first, &cache.cached(&key(1), || None::<(u32, _)>).unwrap()));
        drop(first);
        cache.cached(&key(3), || Some((3u32, third))).unwrap();
        cache.trim();
        assert!(is_cached(&cache, 1) && !is_cached(&cache, 2) && is_cached(&cache, 3));
        let huge = cache.cached(&key(4), || Some((4u32, UNUSED_BUDGET * 2))).unwrap();
        assert!(is_cached(&cache, 1) && is_cached(&cache, 3) && is_cached(&cache, 4));
        drop(huge);
        cache.trim();
        assert!(!is_cached(&cache, 4));
    }

    #[test]
    fn failures_are_cached() {
        let cache = ImageCache::default();
        assert!(cache.cached(&key(5), || None::<(u32, usize)>).is_none());
        assert!(cache.cached::<u32>(&key(5), || unreachable!()).is_none());
    }

    #[cfg(feature = "image-decoders")]
    #[test]
    fn test_path_cache_invalidation() {
        use crate::graphics::Rgba8Pixel;
        let temp_dir = tempfile::tempdir().unwrap();

        let test_path = [temp_dir.path(), std::path::Path::new("testfile.png")]
            .iter()
            .collect::<std::path::PathBuf>();

        let red_image = image::RgbImage::from_pixel(10, 10, image::Rgb([255, 0, 0]));
        red_image.save(&test_path).unwrap();
        let red_slint_image = crate::graphics::Image::load_from_path(&test_path).unwrap();
        let buffer = red_slint_image.to_rgba8().unwrap();
        assert!(
            buffer
                .as_slice()
                .iter()
                .all(|pixel| *pixel == Rgba8Pixel { r: 255, g: 0, b: 0, a: 255 })
        );

        let green_image = image::RgbImage::from_pixel(10, 10, image::Rgb([0, 255, 0]));

        std::thread::sleep(std::time::Duration::from_secs(2));

        green_image.save(&test_path).unwrap();

        /* Can't use this until we use Rust 1.78
        let mod_time = std::fs::metadata(&test_path).unwrap().modified().unwrap();
        std::fs::File::options()
            .write(true)
            .open(&test_path)
            .unwrap()
            .set_modified(mod_time.checked_add(std::time::Duration::from_secs(2)).unwrap())
            .unwrap();
        */

        let green_slint_image = crate::graphics::Image::load_from_path(&test_path).unwrap();
        let buffer = green_slint_image.to_rgba8().unwrap();
        assert!(
            buffer
                .as_slice()
                .iter()
                .all(|pixel| *pixel == Rgba8Pixel { r: 0, g: 255, b: 0, a: 255 })
        );
    }

    #[cfg(feature = "image-decoders")]
    #[test]
    fn file_of_another_size_draws_nothing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("resized.png");
        image::RgbImage::new(64, 64).save(&path).unwrap();
        let image = crate::graphics::Image::load_from_path(&path).unwrap();
        image::RgbImage::new(10, 10).save(&path).unwrap();
        assert_eq!(image.size(), crate::graphics::IntSize::new(64, 64));
        assert!(image.to_rgba8().is_none());
    }

    #[cfg(feature = "image-decoders")]
    #[test]
    fn truncated_file_has_a_size_but_no_pixels() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("truncated.png");
        image::RgbImage::from_fn(64, 64, |x, y| image::Rgb([x as u8, y as u8, 0]))
            .save(&path)
            .unwrap();
        let data = std::fs::read(&path).unwrap();
        std::fs::write(&path, &data[..data.len() / 2]).unwrap();

        let image = crate::graphics::Image::load_from_path(&path).unwrap();
        assert_eq!(image.size(), crate::graphics::IntSize::new(64, 64));
        assert!(image.to_rgba8().is_none());
    }
}
