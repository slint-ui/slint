// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use crate::PhysicalSize;
#[cfg(skia_backend_opengl)]
use i_slint_core::graphics::BorrowedOpenGLTexture;
use i_slint_core::graphics::{
    cache as core_cache, Image, ImageCacheKey, ImageInner, IntRect, IntSize, OpaqueImage,
    OpaqueImageVTable, SharedImageBuffer,
};
use i_slint_core::items::ImageFit;
use i_slint_core::lengths::{LogicalSize, ScaleFactor};

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
    target_size_fn: &dyn Fn() -> LogicalSize,
    image_fit: ImageFit,
    scale_factor: ScaleFactor,
    canvas: &skia_safe::Canvas,
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
            let svg_size = svg.size();
            let fit = i_slint_core::graphics::fit(
                image_fit,
                target_size_fn() * scale_factor,
                IntRect::from_size(svg_size.cast()),
                scale_factor,
                Default::default(), // We only care about the size, so alignments don't matter
                Default::default(),
            );
            let target_size = PhysicalSize::new(
                svg_size.cast::<f32>().width * fit.source_to_target_x,
                svg_size.cast::<f32>().height * fit.source_to_target_y,
            );
            let pixels = match svg.render(Some(target_size.cast())).ok()? {
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

            skia_safe::images::raster_from_data(
                &image_info,
                skia_safe::Data::new_copy(pixels.as_bytes()),
                pixels.width() as usize * 4,
            )
        }
        ImageInner::StaticTextures(_) => todo!(),
        ImageInner::BackendStorage(x) => {
            vtable::VRc::borrow(x).downcast::<SkiaCachedImage>().map(|x| x.image.clone())
        }
        #[cfg(skia_backend_opengl)]
        ImageInner::BorrowedOpenGLTexture(BorrowedOpenGLTexture {
            texture_id,
            size,
            origin,
            ..
        }) => unsafe {
            let mut texture_info = skia_safe::gpu::gl::TextureInfo::from_target_and_id(
                glow::TEXTURE_2D,
                texture_id.get(),
            );
            texture_info.format = glow::RGBA8;
            let backend_texture = skia_safe::gpu::backend_textures::make_gl(
                (size.width as _, size.height as _),
                skia_safe::gpu::Mipmapped::No,
                texture_info,
                "Borrowed GL texture",
            );
            skia_safe::image::Image::from_texture(
                canvas.recording_context().as_mut().unwrap(),
                &backend_texture,
                match origin {
                    i_slint_core::graphics::BorrowedOpenGLTextureOrigin::TopLeft => {
                        skia_safe::gpu::SurfaceOrigin::TopLeft
                    }
                    i_slint_core::graphics::BorrowedOpenGLTextureOrigin::BottomLeft => {
                        skia_safe::gpu::SurfaceOrigin::BottomLeft
                    }
                    _ => unimplemented!(
                        "internal error: missing implementation for BorrowedOpenGLTextureOrigin"
                    ),
                },
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            )
        },
        #[cfg(not(skia_backend_opengl))]
        ImageInner::BorrowedOpenGLTexture(..) => None,
        ImageInner::NineSlice(n) => {
            as_skia_image(n.image(), target_size_fn, ImageFit::Preserve, scale_factor, canvas)
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
    skia_safe::images::raster_from_data(&image_info, data, bpl)
}
