// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use i_slint_core::graphics::{
    cache as core_cache, Image, ImageCacheKey, ImageInner, IntSize, OpaqueImage, OpaqueImageVTable,
    SharedImageBuffer,
};
use i_slint_core::items::ImageFit;
use i_slint_core::lengths::{LogicalLength, LogicalSize, ScaleFactor};

struct SkiaCachedImage {
    image: skia_safe::Image,
    cache_key: ImageCacheKey,
}

i_slint_core::OpaqueImageVTable_static! {
    static SKIA_CACHED_IMAGE_VT for SkiaCachedImage
}

impl OpaqueImage for SkiaCachedImage {
    fn size(&self) -> IntSize {
        IntSize::new(self.image.width() as u32, self.image.height() as u32)
    }

    fn cache_key(&self) -> ImageCacheKey {
        self.cache_key.clone()
    }
}

pub(crate) fn as_skia_image(
    image: Image,
    target_width: std::pin::Pin<&i_slint_core::Property<LogicalLength>>,
    target_height: std::pin::Pin<&i_slint_core::Property<LogicalLength>>,
    image_fit: ImageFit,
    scale_factor: ScaleFactor,
) -> Option<skia_safe::Image> {
    let image_inner: &ImageInner = (&image).into();
    match image_inner {
        ImageInner::None => None,
        ImageInner::EmbeddedImage { buffer, cache_key } => {
            let result = image_buffer_to_skia_image(buffer);
            if let Some(img) = result.as_ref() {
                core_cache::replace_cached_image(
                    cache_key.clone(),
                    ImageInner::BackendStorage(vtable::VRc::into_dyn(vtable::VRc::new(
                        SkiaCachedImage { image: img.clone(), cache_key: cache_key.clone() },
                    ))),
                )
            }
            result
        }
        ImageInner::Svg(svg) => {
            // Query target_width/height here again to ensure that changes will invalidate the item rendering cache.
            let target_size =
                LogicalSize::from_lengths(target_width.get(), target_height.get()) * scale_factor;
            let target_size = i_slint_core::graphics::fit_size(image_fit, target_size, svg.size());
            let pixels = match svg.render(target_size.cast()).ok()? {
                SharedImageBuffer::RGB8(_) => unreachable!(),
                SharedImageBuffer::RGBA8(_) => unreachable!(),
                SharedImageBuffer::RGBA8Premultiplied(pixels) => pixels,
            };

            let image_info = skia_safe::ImageInfo::new(
                skia_safe::ISize::new(pixels.width() as i32, pixels.height() as i32),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Premul,
                None,
            );

            skia_safe::image::Image::from_raster_data(
                &image_info,
                skia_safe::Data::new_copy(pixels.as_bytes()),
                pixels.width() as usize * 4,
            )
        }
        ImageInner::StaticTextures(_) => todo!(),
        ImageInner::BackendStorage(x) => {
            vtable::VRc::borrow(x).downcast::<SkiaCachedImage>().map(|x| x.image.clone())
        }
    }
}

fn image_buffer_to_skia_image(buffer: &SharedImageBuffer) -> Option<skia_safe::Image> {
    let (data, bpl, size, color_type, alpha_type) = match buffer {
        SharedImageBuffer::RGB8(pixels) => {
            // RGB888 with one byte per component is not supported by Skia right now. Convert once to RGBA8 :-(
            let rgba = pixels
                .as_bytes()
                .chunks(3)
                .flat_map(|rgb| IntoIterator::into_iter([rgb[0], rgb[1], rgb[2], 255]))
                .collect::<Vec<u8>>();
            (
                skia_safe::Data::new_copy(&*rgba),
                pixels.width() as usize * 4,
                pixels.size(),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
            )
        }
        SharedImageBuffer::RGBA8(pixels) => (
            skia_safe::Data::new_copy(pixels.as_bytes()),
            pixels.width() as usize * 4,
            pixels.size(),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Unpremul,
        ),
        SharedImageBuffer::RGBA8Premultiplied(pixels) => (
            skia_safe::Data::new_copy(pixels.as_bytes()),
            pixels.width() as usize * 4,
            pixels.size(),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Premul,
        ),
    };
    let image_info = skia_safe::ImageInfo::new(
        skia_safe::ISize::new(size.width as i32, size.height as i32),
        color_type,
        alpha_type,
        None,
    );
    skia_safe::image::Image::from_raster_data(&image_info, data, bpl)
}
