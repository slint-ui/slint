// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
This module contains image and caching related types for the run-time library.
*/

use super::{CachedPath, Image, ImageCacheKey, ImageInner, SharedImageBuffer, SharedPixelBuffer};
use crate::{slice::Slice, SharedString};

struct ImageWeightInBytes;

impl clru::WeightScale<ImageCacheKey, ImageInner> for ImageWeightInBytes {
    fn weight(&self, _key: &ImageCacheKey, value: &ImageInner) -> usize {
        match value {
            ImageInner::None => 0,
            ImageInner::EmbeddedImage { buffer, .. } => match buffer {
                SharedImageBuffer::RGB8(pixels) => pixels.as_bytes().len(),
                SharedImageBuffer::RGBA8(pixels) => pixels.as_bytes().len(),
                SharedImageBuffer::RGBA8Premultiplied(pixels) => pixels.as_bytes().len(),
            },
            #[cfg(feature = "svg")]
            ImageInner::Svg(_) => 512, // Don't know how to measure the size of the parsed SVG tree...
            #[cfg(target_arch = "wasm32")]
            ImageInner::HTMLImage(_) => 512, // Something... the web browser maintainers its own cache. The purpose of this cache is to reduce the amount of DOM elements.
            ImageInner::StaticTextures(_) => 0,
            ImageInner::BackendStorage(x) => vtable::VRc::borrow(x).size().area() as usize,
            #[cfg(not(target_arch = "wasm32"))]
            ImageInner::BorrowedOpenGLTexture(..) => 0, // Assume storage in GPU memory
            ImageInner::NineSlice(nine) => self.weight(_key, &nine.0),
        }
    }
}

/// Cache used to avoid repeatedly decoding images from disk.
pub(crate) struct ImageCache(
    clru::CLruCache<
        ImageCacheKey,
        ImageInner,
        std::collections::hash_map::RandomState,
        ImageWeightInBytes,
    >,
);

thread_local!(pub(crate) static IMAGE_CACHE: core::cell::RefCell<ImageCache>  =
    core::cell::RefCell::new(
        ImageCache(
            clru::CLruCache::with_config(
                clru::CLruCacheConfig::new(core::num::NonZeroUsize::new(5 * 1024 * 1024).unwrap())
                    .with_scale(ImageWeightInBytes)
            )
        )
    )
);

impl ImageCache {
    // Look up the given image cache key in the image cache and upgrade the weak reference to a strong one if found,
    // otherwise a new image is created/loaded from the given callback.
    fn lookup_image_in_cache_or_create(
        &mut self,
        cache_key: ImageCacheKey,
        image_create_fn: impl Fn(ImageCacheKey) -> Option<ImageInner>,
    ) -> Option<Image> {
        Some(Image(if let Some(entry) = self.0.get(&cache_key) {
            entry.clone()
        } else {
            let new_image = image_create_fn(cache_key.clone())?;
            self.0.put_with_weight(cache_key, new_image.clone()).ok();
            new_image
        }))
    }

    pub(crate) fn load_image_from_path(&mut self, path: &SharedString) -> Option<Image> {
        if path.is_empty() {
            return None;
        }
        let cache_key = ImageCacheKey::Path(CachedPath::new(path.as_str()));
        #[cfg(target_arch = "wasm32")]
        return self.lookup_image_in_cache_or_create(cache_key, |_| {
            return Some(ImageInner::HTMLImage(vtable::VRc::new(
                super::htmlimage::HTMLImage::new(&path),
            )));
        });
        #[cfg(not(target_arch = "wasm32"))]
        return self.lookup_image_in_cache_or_create(cache_key, |cache_key| {
            if cfg!(feature = "svg") && (path.ends_with(".svg") || path.ends_with(".svgz")) {
                return Some(ImageInner::Svg(vtable::VRc::new(
                    super::svg::load_from_path(path, cache_key).map_or_else(
                        |err| {
                            eprintln!("Error loading SVG from {}: {}", &path, err);
                            None
                        },
                        Some,
                    )?,
                )));
            }

            image::open(std::path::Path::new(&path.as_str())).map_or_else(
                |decode_err| {
                    eprintln!("Error loading image from {}: {}", &path, decode_err);
                    None
                },
                |image| {
                    Some(ImageInner::EmbeddedImage {
                        cache_key,
                        buffer: dynamic_image_to_shared_image_buffer(image),
                    })
                },
            )
        });
    }

    pub(crate) fn load_image_from_embedded_data(
        &mut self,
        data: Slice<'static, u8>,
        format: Slice<'_, u8>,
    ) -> Option<Image> {
        let cache_key = ImageCacheKey::from_embedded_image_data(data.as_slice());
        self.lookup_image_in_cache_or_create(cache_key, |cache_key| {
            #[cfg(feature = "svg")]
            if format.as_slice() == b"svg" || format.as_slice() == b"svgz" {
                return Some(ImageInner::Svg(vtable::VRc::new(
                    super::svg::load_from_data(data.as_slice(), cache_key).map_or_else(
                        |svg_err| {
                            eprintln!("Error loading SVG: {}", svg_err);
                            None
                        },
                        Some,
                    )?,
                )));
            }

            let format = std::str::from_utf8(format.as_slice())
                .ok()
                .and_then(image::ImageFormat::from_extension);
            let maybe_image = if let Some(format) = format {
                image::load_from_memory_with_format(data.as_slice(), format)
            } else {
                image::load_from_memory(data.as_slice())
            };

            match maybe_image {
                Ok(image) => Some(ImageInner::EmbeddedImage {
                    cache_key,
                    buffer: dynamic_image_to_shared_image_buffer(image),
                }),
                Err(decode_err) => {
                    eprintln!("Error decoding embedded image: {}", decode_err);
                    None
                }
            }
        })
    }
}

fn dynamic_image_to_shared_image_buffer(dynamic_image: image::DynamicImage) -> SharedImageBuffer {
    if dynamic_image.color().has_alpha() {
        let rgba8image = dynamic_image.to_rgba8();
        SharedImageBuffer::RGBA8(SharedPixelBuffer::clone_from_slice(
            rgba8image.as_raw(),
            rgba8image.width(),
            rgba8image.height(),
        ))
    } else {
        let rgb8image = dynamic_image.to_rgb8();
        SharedImageBuffer::RGB8(SharedPixelBuffer::clone_from_slice(
            rgb8image.as_raw(),
            rgb8image.width(),
            rgb8image.height(),
        ))
    }
}

/// Replace the cached image key with the given value
pub fn replace_cached_image(key: ImageCacheKey, value: ImageInner) {
    if key == ImageCacheKey::Invalid {
        return;
    }
    let _ =
        IMAGE_CACHE.with(|global_cache| global_cache.borrow_mut().0.put_with_weight(key, value));
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use crate::graphics::Rgba8Pixel;

    #[test]
    fn test_path_cache_invalidation() {
        let temp_dir = tempfile::tempdir().unwrap();

        let test_path = [temp_dir.path(), std::path::Path::new("testfile.png")]
            .iter()
            .collect::<std::path::PathBuf>();

        let red_image = image::RgbImage::from_pixel(10, 10, image::Rgb([255, 0, 0]));
        red_image.save(&test_path).unwrap();
        let red_slint_image = crate::graphics::Image::load_from_path(&test_path).unwrap();
        let buffer = red_slint_image.to_rgba8().unwrap();
        assert!(buffer
            .as_slice()
            .iter()
            .all(|pixel| *pixel == Rgba8Pixel { r: 255, g: 0, b: 0, a: 255 }));

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
        assert!(buffer
            .as_slice()
            .iter()
            .all(|pixel| *pixel == Rgba8Pixel { r: 0, g: 255, b: 0, a: 255 }));
    }
}
