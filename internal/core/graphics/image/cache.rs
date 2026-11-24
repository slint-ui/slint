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
use core::hash::{Hash, Hasher};
use std::collections::HashMap;

/// How many windows' worth of pixels the cache keeps of what nothing shows,
/// so that an image shown again isn't loaded again.
const UNUSED_WINDOWS: usize = 2;

/// What's cached when making an entry failed, so that it isn't tried again on every use.
struct Failed;

/// Tells apart what's made of the same type from the same image,
/// such as the sizes an SVG is rasterized at.
trait Variant: Any {
    fn dyn_eq(&self, other: &dyn Variant) -> bool;
    fn dyn_hash(&self, state: &mut dyn Hasher);
}

impl<V: Hash + Eq + 'static> Variant for V {
    fn dyn_eq(&self, other: &dyn Variant) -> bool {
        (other as &dyn Any).downcast_ref::<V>() == Some(self)
    }
    fn dyn_hash(&self, mut state: &mut dyn Hasher) {
        self.hash(&mut state);
    }
}

#[derive(Clone)]
struct Key {
    source: ImageCacheKey,
    /// The type of what's made from the source.
    data_type: TypeId,
    variant: Option<Rc<dyn Variant>>,
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
            && self.data_type == other.data_type
            && match (&self.variant, &other.variant) {
                (None, None) => true,
                (Some(l), Some(r)) => l.dyn_eq(&**r),
                _ => false,
            }
    }
}

impl Eq for Key {}

impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.source.hash(state);
        self.data_type.hash(state);
        if let Some(variant) = &self.variant {
            variant.dyn_hash(state);
        }
    }
}

struct Entry {
    /// The `T` of the key's `data_type`, or [`Failed`].
    data: Rc<dyn Any>,
    weight: usize,
    last_use: u64,
}

#[derive(Default)]
struct Entries {
    entries: HashMap<Key, Entry>,
    total_weight: usize,
    clock: u64,
    fixed_budget: Option<usize>,
    /// The size of the pixels of the largest window drawn, in bytes.
    largest_window: usize,
}

impl Entries {
    fn insert(&mut self, key: Key, entry: Entry) {
        self.total_weight += entry.weight;
        if let Some(old) = self.entries.insert(key, entry) {
            self.total_weight -= old.weight;
        }
    }

    /// Returns the bytes of entries that nothing holds that are kept,
    /// or `None` to keep them all until a window is drawn.
    fn budget(&self) -> Option<usize> {
        self.fixed_budget
            .or((self.largest_window > 0).then(|| self.largest_window * UNUSED_WINDOWS))
    }

    /// Drops the least recently used entries that nothing holds and that weren't used after
    /// `kept_since`, beyond the budget.
    fn trim(&mut self, kept_since: u64) {
        let Some(budget) = self.budget() else { return };
        if self.total_weight <= budget {
            return;
        }
        let is_unused =
            |entry: &Entry| Rc::strong_count(&entry.data) == 1 && entry.last_use <= kept_since;
        let mut unused_weight: usize =
            self.entries.values().filter(|entry| is_unused(entry)).map(|entry| entry.weight).sum();
        if unused_weight <= budget {
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
            if unused_weight <= budget {
                break;
            }
        }
    }
}

/// The image cache of a [`SlintContext`].
pub(crate) struct ImageCache(core::cell::RefCell<Entries>);

impl Default for ImageCache {
    fn default() -> Self {
        Self::with_size(size_from_env())
    }
}

/// Returns the size in bytes that `SLINT_IMAGE_CACHE_SIZE` sets in MB.
fn size_from_env() -> Option<usize> {
    let value = std::env::var("SLINT_IMAGE_CACHE_SIZE").ok()?;
    match value.trim().parse::<usize>() {
        Ok(megabytes) => Some(megabytes.saturating_mul(1024 * 1024)),
        Err(_) => {
            crate::debug_log!("Ignoring SLINT_IMAGE_CACHE_SIZE={value}: expected a size in MB");
            None
        }
    }
}

impl ImageCache {
    /// Makes a cache that keeps `size` bytes of what nothing holds,
    /// or a size that follows the windows drawn.
    fn with_size(size: Option<usize>) -> Self {
        Self(core::cell::RefCell::new(Entries { fixed_budget: size, ..Default::default() }))
    }

    fn cached<T: 'static>(
        &self,
        cache_key: Key,
        create: impl FnOnce() -> Option<(T, usize)>,
    ) -> Option<Rc<T>> {
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
        let weight = weight + held_bytes(&cache_key.source);
        let mut entries = self.0.borrow_mut();
        entries.clock += 1;
        let last_use = entries.clock;
        entries.insert(cache_key, Entry { data, weight, last_use });
        created.map(|(data, _)| data)
    }

    /// Drops the least recently used entries that nothing holds, beyond the budget.
    /// It runs after a window of `window_size` is drawn, since that's when renderers release
    /// images. The largest window sets the budget.
    /// What was used since the window's previous drawing, at `window_clock`, is kept,
    /// since the window may draw it again without holding it.
    pub(crate) fn trim_after_drawing(
        &self,
        window_size: crate::api::PhysicalSize,
        window_clock: &core::cell::Cell<u64>,
    ) {
        let mut entries = self.0.borrow_mut();
        let window = window_size.width as usize * window_size.height as usize * 4;
        entries.largest_window = entries.largest_window.max(window);
        entries.trim(window_clock.get());
        window_clock.set(entries.clock);
    }
}

/// Like [`cached`], for one of several `T` made from the same image, told apart by `variant`.
pub fn cached_variant<T: 'static, V: Hash + Eq + 'static>(
    context: &SlintContext,
    key: &ImageCacheKey,
    variant: V,
    create: impl FnOnce() -> Option<(T, usize)>,
) -> Option<Rc<T>> {
    cached_with(Some(context), key, Some(Rc::new(variant)), create)
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
    cached_with(context, key, None, create)
}

fn cached_with<T: 'static>(
    context: Option<&SlintContext>,
    key: &ImageCacheKey,
    variant: Option<Rc<dyn Variant>>,
    create: impl FnOnce() -> Option<(T, usize)>,
) -> Option<Rc<T>> {
    match context {
        Some(context) => {
            let key = Key { source: key.clone(), data_type: TypeId::of::<T>(), variant };
            context.0.image_cache.cached(key, create)
        }
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

    fn source(n: usize) -> ImageCacheKey {
        static DATA: [u8; 8] = [0; 8];
        ImageCacheKey::EmbeddedData(by_address::ByAddress(crate::slice::Slice::from_slice(
            &DATA[n..=n],
        )))
    }

    fn key(n: usize) -> Key {
        Key { source: source(n), data_type: TypeId::of::<u32>(), variant: None }
    }

    fn is_cached(cache: &ImageCache, n: usize) -> bool {
        cache.0.borrow().entries.contains_key(&key(n))
    }

    fn window() -> crate::api::PhysicalSize {
        crate::api::PhysicalSize::new(100, 100)
    }

    /// The budget of a cache whose largest window is `window()`.
    const BUDGET: usize = 100 * 100 * 4 * UNUSED_WINDOWS;

    #[test]
    fn shares_and_evicts_unused_beyond_budget() {
        let cache = ImageCache::with_size(None);
        let clock = Default::default();
        let third = BUDGET / 3 + 1;
        let first = cache.cached(key(1), || Some((1u32, third))).unwrap();
        cache.cached(key(2), || Some((2u32, third))).unwrap();
        assert!(Rc::ptr_eq(&first, &cache.cached(key(1), || None::<(u32, _)>).unwrap()));
        drop(first);
        cache.cached(key(3), || Some((3u32, third))).unwrap();
        // Nothing is evicted before a window is drawn, nor what was used since the last drawing.
        assert!(is_cached(&cache, 1) && is_cached(&cache, 2) && is_cached(&cache, 3));
        cache.trim_after_drawing(window(), &clock);
        assert!(is_cached(&cache, 1) && is_cached(&cache, 2) && is_cached(&cache, 3));
        cache.trim_after_drawing(window(), &clock);
        // The least recently used of the three unused images is gone.
        assert!(is_cached(&cache, 1) && !is_cached(&cache, 2) && is_cached(&cache, 3));
        // An image in use doesn't count, even beyond the budget.
        let huge = cache.cached(key(4), || Some((4u32, BUDGET * 2))).unwrap();
        assert!(is_cached(&cache, 1) && is_cached(&cache, 3) && is_cached(&cache, 4));
        drop(huge);
        cache.trim_after_drawing(window(), &clock);
        cache.trim_after_drawing(window(), &clock);
        assert!(!is_cached(&cache, 4));
    }

    #[test]
    fn what_each_frame_uses_is_kept() {
        let cache = ImageCache::with_size(Some(0));
        let clock = Default::default();
        cache.cached(key(0), || Some((0u32, 1))).unwrap();
        cache.trim_after_drawing(window(), &clock);
        assert!(is_cached(&cache, 0));
        cache.cached(key(0), || None::<(u32, usize)>).unwrap();
        cache.trim_after_drawing(window(), &clock);
        assert!(is_cached(&cache, 0));
        // A frame that doesn't use it lets it go.
        cache.trim_after_drawing(window(), &clock);
        assert!(!is_cached(&cache, 0));
    }

    #[test]
    fn drawing_another_window_keeps_what_a_window_draws() {
        let cache = ImageCache::with_size(Some(0));
        let (first, second) = (Default::default(), Default::default());
        cache.cached(key(7), || Some((7u32, 1))).unwrap();
        cache.trim_after_drawing(window(), &first);
        cache.trim_after_drawing(window(), &second);
        assert!(is_cached(&cache, 7));
        cache.trim_after_drawing(window(), &first);
        assert!(!is_cached(&cache, 7));
    }

    #[test]
    fn failures_are_cached() {
        let cache = ImageCache::default();
        assert!(cache.cached(key(5), || None::<(u32, usize)>).is_none());
        assert!(cache.cached::<u32>(key(5), || unreachable!()).is_none());
    }

    #[test]
    fn variants_are_separate_entries() {
        let cache = ImageCache::default();
        let variant = |size: u32| Key { variant: Some(Rc::new(size)), ..key(6) };
        let small = cache.cached(variant(16), || Some((16u32, 0))).unwrap();
        let large = cache.cached(variant(32), || Some((32u32, 0))).unwrap();
        assert_eq!((*small, *large), (16, 32));
        assert!(Rc::ptr_eq(&small, &cache.cached(variant(16), || None::<(u32, _)>).unwrap()));
        assert!(cache.cached(key(6), || None::<(u32, usize)>).is_none());
    }

    #[cfg(feature = "image-decoders")]
    #[test]
    fn only_shared_pixels_are_kept() {
        static PNG: &[u8] =
            include_bytes!("../../../../tests/screenshots/cases/image/tiny-1x1.png");
        let context = SlintContext::new(alloc::boxed::Box::new(crate::testing::NoWindowPlatform));
        let image = crate::graphics::load_image_from_embedded_data(
            &context,
            crate::slice::Slice::from_slice(PNG),
            crate::slice::Slice::from_slice(b"png"),
        );
        let has_pixels = || {
            context
                .0
                .image_cache
                .0
                .borrow()
                .entries
                .keys()
                .any(|key| key.data_type == TypeId::of::<SharedImageBuffer>())
        };
        assert!(image.0.render_to_premultiplied_buffer(None, &context).is_some());
        assert!(image.0.render_to_buffer(None, Some(&context)).is_some());
        assert!(!has_pixels());
        assert!(image.0.decode_shared(&context).is_some());
        assert!(has_pixels());
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
