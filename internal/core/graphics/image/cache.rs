// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*!
This module contains image and caching related types for the run-time library.
*/

use std::collections::HashMap;

use super::{Image, ImageCacheKey, ImageInner, SharedImageBuffer, SharedPixelBuffer};
use crate::{slice::Slice, SharedString};

// Cache used to avoid repeatedly decoding images from disk.
#[derive(Default)]
pub(crate) struct ImageCache(HashMap<ImageCacheKey, ImageInner>);

thread_local!(pub(crate) static IMAGE_CACHE: core::cell::RefCell<ImageCache>  = Default::default());

impl ImageCache {
    // Look up the given image cache key in the image cache and upgrade the weak reference to a strong one if found,
    // otherwise a new image is created/loaded from the given callback.
    fn lookup_image_in_cache_or_create(
        &mut self,
        cache_key: ImageCacheKey,
        image_create_fn: impl Fn(ImageCacheKey) -> Option<ImageInner>,
    ) -> Option<Image> {
        Some(Image(match self.0.entry(cache_key.clone()) {
            std::collections::hash_map::Entry::Occupied(existing_entry) => {
                existing_entry.get().clone()
            }
            std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                let new_image = image_create_fn(cache_key)?;
                vacant_entry.insert(new_image.clone());
                new_image
            }
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
            if cfg!(feature = "svg") {
                if path.ends_with(".svg") || path.ends_with(".svgz") {
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
        format: Slice<'static, u8>,
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
