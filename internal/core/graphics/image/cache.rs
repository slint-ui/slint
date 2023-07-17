// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

/*!
This module contains image and caching related types for the run-time library.
*/

use super::{Image, ImageCacheKey, ImageInner, SharedImageBuffer, SharedPixelBuffer};
use crate::{slice::Slice, SharedString};

struct ImageWeightInBytes;

impl clru::WeightScale<ImageCacheKey, ImageInner> for ImageWeightInBytes {
    fn weight(&self, _: &ImageCacheKey, value: &ImageInner) -> usize {
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
        let cache_key = ImageCacheKey::Path(path.clone());
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
                            log::error!("Error loading SVG from {}: {}", &path, err);
                            None
                        },
                        Some,
                    )?,
                )));
            }

            image::open(std::path::Path::new(&path.as_str())).map_or_else(
                |decode_err| {
                    log::error!("Error loading image from {}: {}", &path, decode_err);
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
                            log::error!("Error loading SVG: {}", svg_err);
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
                    log::error!("Error decoding embedded image: {}", decode_err);
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
