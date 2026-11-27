// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
The cache of what's loaded, parsed, or decoded from images, shared by everything on a thread.

Entries are keyed by where the image's data is and by the type of what's made from it,
so renderers can keep what they make from an image next to the loaded image.
*/

#[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
use super::CachedPath;
use super::{Image, ImageCacheKey, ImageData, ImageInner, SharedImageBuffer};
use crate::{SharedString, slice::Slice};
use alloc::rc::Rc;
use core::any::{Any, TypeId};
use std::collections::HashMap;

/// Bytes of entries that the cache keeps.
const BUDGET: usize = 5 * 1024 * 1024;

struct Entry {
    data: Rc<dyn Any>,
    weight: usize,
    last_use: u64,
}

#[derive(Default)]
struct ImageCache {
    entries: HashMap<(ImageCacheKey, TypeId), Entry>,
    total_weight: usize,
    clock: u64,
}

impl ImageCache {
    fn trim(&mut self) {
        while self.total_weight > BUDGET {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_use)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            let entry = self.entries.remove(&oldest).unwrap();
            self.total_weight -= entry.weight;
        }
    }
}

crate::thread_local!(static IMAGE_CACHE: core::cell::RefCell<ImageCache> = Default::default());

/// Returns the `T` made from the image at `key`,
/// or makes it with `create`, which returns it with its size in bytes.
///
/// The cache keeps the most recently used entries within a budget.
/// Without a valid `key`, or beyond the budget on its own, the result isn't cached.
pub fn cached<T: 'static>(
    key: &ImageCacheKey,
    create: impl FnOnce() -> Option<(T, usize)>,
) -> Option<Rc<T>> {
    if *key == ImageCacheKey::Invalid {
        return create().map(|(data, _)| Rc::new(data));
    }
    let cache_key = (key.clone(), TypeId::of::<T>());
    let cached = IMAGE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.clock += 1;
        let clock = cache.clock;
        let entry = cache.entries.get_mut(&cache_key)?;
        entry.last_use = clock;
        entry.data.clone().downcast::<T>().ok()
    });
    if cached.is_some() {
        return cached;
    }
    // `create` may use the cache for other entries, so the cache isn't borrowed while it runs.
    let (data, weight) = create()?;
    let data = Rc::new(data);
    let weight = weight.max(core::mem::size_of::<Entry>());
    if weight <= BUDGET {
        IMAGE_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            cache.clock += 1;
            let last_use = cache.clock;
            cache.total_weight += weight;
            if let Some(old) =
                cache.entries.insert(cache_key, Entry { data: data.clone(), weight, last_use })
            {
                cache.total_weight -= old.weight;
            }
            cache.trim();
        });
    }
    Some(data)
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

fn image_weight(image: &ImageInner) -> usize {
    match image {
        ImageInner::None => 0,
        ImageInner::EmbeddedImage { buffer, .. } => buffer_weight(buffer),
        #[cfg(feature = "svg")]
        ImageInner::Svg(svg) => svg.weight_in_bytes(),
        #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
        ImageInner::HTMLImage(_) => 512, // Something... the web browser maintainers its own cache. The purpose of this cache is to reduce the amount of DOM elements.
        ImageInner::StaticTextures(_) => 0,
        #[cfg(feature = "owned-pre-rendered-resources")]
        ImageInner::OwnedTextures(textures) => textures.data.len(),
        ImageInner::BorrowedOpenGLTexture(..) => 0, // Assume storage in GPU memory
        ImageInner::NineSlice(nine) => image_weight(&nine.0),
        #[cfg(any(feature = "unstable-wgpu-29", feature = "unstable-wgpu-30"))]
        ImageInner::WGPUTexture(..) => 0, // The texture is imported from the application and will never reside in our cache.
        ImageInner::AnimatedImage(animated) => animated.weight_in_bytes(),
    }
}

/// Returns the image loaded from `key`, or loads it with `load`.
fn load(
    key: ImageCacheKey,
    load: impl FnOnce(ImageCacheKey) -> Option<ImageInner>,
) -> Option<Image> {
    let image = cached(&key, || {
        let image = load(key.clone())?;
        let weight = image_weight(&image);
        Some((image, weight))
    })?;
    Some(Image(ImageInner::clone(&image)))
}

#[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
pub(crate) fn load_image_from_path(_path: &SharedString) -> Option<Image> {
    None
}

#[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
pub(crate) fn load_image_from_path(path: &SharedString) -> Option<Image> {
    if path.is_empty() {
        return None;
    }
    let cache_key = ImageCacheKey::Path(CachedPath::new(path.as_str()));
    load(cache_key, |cache_key| {
        #[cfg(feature = "svg")]
        if path.ends_with(".svg") || path.ends_with(".svgz") {
            return Some(ImageInner::Svg(vtable::VRc::new(
                super::svg::load_from_path(path, cache_key).map_or_else(
                    |err| {
                        crate::debug_log!("Error loading SVG from {}: {}", &path, err);
                        None
                    },
                    Some,
                )?,
            )));
        }

        #[cfg(not(feature = "svg"))]
        if path.ends_with(".svg") || path.ends_with(".svgz") {
            crate::debug_log!("Error loading SVG from {}: {}", &path, super::SVG_DISABLED_MESSAGE);
            return None;
        }

        let format = image::ImageFormat::from_path(path.as_str()).ok();
        let loaded = if format.is_some_and(|format| !super::animated::can_animate(format)) {
            image::open(path.as_str()).map(|image| ImageInner::EmbeddedImage {
                cache_key,
                buffer: super::dynamic_image_to_shared_image_buffer(image),
            })
        } else {
            std::fs::read(path.as_str()).map_err(image::ImageError::IoError).and_then(|data| {
                let data = crate::SharedVector::from_slice(&data);
                ImageInner::decode_with_cache_key(cache_key, ImageData::Shared(&data), format)
            })
        };
        loaded.map_err(|err| crate::debug_log!("Error loading image from {}: {}", &path, err)).ok()
    })
}

/// Load an image by handing its URL to an `<img>` element for the browser to
/// fetch. This is a web-only slintpad mechanism, not general network loading.
#[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
pub(crate) fn load_as_html_image(url: &str) -> Option<Image> {
    if url.is_empty() {
        return None;
    }
    load(ImageCacheKey::URL(url.into()), |_| {
        Some(ImageInner::HTMLImage(vtable::VRc::new(super::htmlimage::HTMLImage::new(url))))
    })
}

pub(crate) fn load_image_from_embedded_data(
    data: Slice<'static, u8>,
    format: Slice<'_, u8>,
) -> Option<Image> {
    load(ImageCacheKey::from_embedded_image_data(data.as_slice()), |cache_key| {
        ImageInner::load_from_data_with_cache_key(
            cache_key,
            ImageData::Static(data.as_slice()),
            format,
        )
    })
}

#[cfg(all(target_arch = "wasm32", not(target_os = "emscripten"), not(feature = "image-decoders")))]
pub(crate) fn load_image_from_data_uri(uri: &str, data: &[u8], format: &str) -> Option<Image> {
    load(ImageCacheKey::URL(uri.into()), |cache_key| {
        ImageInner::load_from_data_with_cache_key(
            cache_key,
            ImageData::Borrowed(data),
            format.as_bytes().into(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: usize) -> ImageCacheKey {
        ImageCacheKey::EmbeddedData(n)
    }

    fn is_cached(n: usize) -> bool {
        IMAGE_CACHE
            .with(|cache| cache.borrow().entries.contains_key(&(key(n), TypeId::of::<u32>())))
    }

    #[test]
    fn shares_and_evicts_beyond_budget() {
        let third = BUDGET / 3 + 1;
        let first = cached(&key(1), || Some((1u32, third))).unwrap();
        cached(&key(2), || Some((2u32, third))).unwrap();
        assert!(Rc::ptr_eq(&first, &cached(&key(1), || None::<(u32, _)>).unwrap()));
        cached(&key(3), || Some((3u32, third))).unwrap();
        assert!(is_cached(1) && !is_cached(2) && is_cached(3));
        cached(&key(4), || Some((4u32, BUDGET + 1))).unwrap();
        assert!(is_cached(1) && is_cached(3) && !is_cached(4));
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
}
