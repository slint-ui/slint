// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::graphics::{
    Image, ImageCacheKey, ImageInner, IntSize, OpaqueImage, OpaqueImageVTable, SharedImageBuffer,
    SharedPixelBuffer, cache as core_cache,
};
use i_slint_core::items::ImageFit;
use i_slint_core::lengths::{LogicalSize, ScaleFactor};

struct SkiaCachedImage {
    image: skia_safe::Image,
    cache_key: ImageCacheKey,
}

struct PixelBytes<Pixel>(SharedPixelBuffer<Pixel>);

impl<Pixel: Clone + bytemuck::Pod> AsRef<[u8]> for PixelBytes<Pixel> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

/// Wraps the pixels in Skia data that shares the buffer instead of copying it.
fn shared_pixel_data<Pixel: Clone + bytemuck::Pod + Send + Sync>(
    pixels: &SharedPixelBuffer<Pixel>,
) -> skia_safe::Data {
    skia_safe::Data::new_with_owner(PixelBytes(pixels.clone()))
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

/// `transform_scale` is how much the canvas transform magnifies the image,
/// see [`i_slint_core::graphics::transformed_scalable_render_size`].
pub(crate) fn as_skia_image(
    image: Image,
    target_size_fn: &dyn Fn() -> LogicalSize,
    image_fit: ImageFit,
    scale_factor: ScaleFactor,
    transform_scale: f32,
    canvas: &skia_safe::Canvas,
    surface: Option<&dyn crate::Surface>,
) -> Option<skia_safe::Image> {
    let image_inner: &ImageInner = (&image).into();
    match image_inner {
        ImageInner::None => None,
        ImageInner::EmbeddedImage { buffer, cache_key } => {
            if let Some(ImageInner::BackendStorage(cached)) =
                core_cache::lookup_cached_image(cache_key)
                && let Some(img) = skia_cached_image(&cached)
            {
                return Some(img);
            }
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
            let target_size = i_slint_core::graphics::transformed_scalable_render_size(
                svg.size(),
                image_fit,
                target_size_fn(),
                scale_factor,
                Default::default(),
                transform_scale,
            )?;
            let SharedImageBuffer::RGBA8Premultiplied(pixels) =
                svg.render(Some(target_size)).ok()?
            else {
                unreachable!()
            };

            let image_info = crate::image_info(
                skia_safe::ISize::new(pixels.width() as i32, pixels.height() as i32),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Premul,
            );

            skia_safe::images::raster_from_data(
                &image_info,
                shared_pixel_data(&pixels),
                pixels.width() as usize * 4,
            )
        }
        ImageInner::BackendStorage(x) => skia_cached_image(x),
        ImageInner::BorrowedOpenGLTexture(texture) => {
            surface.and_then(|surface| surface.import_opengl_texture(canvas, texture))
        }
        ImageInner::NineSlice(n) => as_skia_image(
            n.image(),
            target_size_fn,
            ImageFit::Preserve,
            scale_factor,
            transform_scale,
            canvas,
            surface,
        ),
        ImageInner::AnimatedImage(animated) => as_skia_image(
            animated.first_frame(),
            target_size_fn,
            image_fit,
            scale_factor,
            transform_scale,
            canvas,
            surface,
        ),
        #[cfg(any(feature = "unstable-wgpu-29", feature = "unstable-wgpu-30"))]
        ImageInner::WGPUTexture(any_wgpu_texture) => {
            surface.and_then(|surface| surface.import_wgpu_texture(canvas, any_wgpu_texture))
        }
        _ => image_inner
            .render_to_buffer(None)
            .and_then(|buffer| image_buffer_to_skia_image(&buffer)),
    }
}

fn skia_cached_image(storage: &vtable::VRc<OpaqueImageVTable>) -> Option<skia_safe::Image> {
    vtable::VRc::borrow(storage).downcast::<SkiaCachedImage>().map(|x| x.image.clone())
}

fn image_buffer_to_skia_image(buffer: &SharedImageBuffer) -> Option<skia_safe::Image> {
    // Report fully opaque images as such to Skia. This allows Skia to skip
    // reading destination pixels back when blending, which is particularly
    // important for the software renderer when it renders directly into
    // write-combined (uncached) frame buffer memory, where CPU reads are an
    // order of magnitude slower than writes. The scan happens once per image;
    // the resulting skia_safe::Image is cached.
    fn opaque_or(rgba_bytes: &[u8], alpha_type: skia_safe::AlphaType) -> skia_safe::AlphaType {
        if rgba_bytes.as_chunks::<4>().0.iter().all(|px| px[3] == 255) {
            skia_safe::AlphaType::Opaque
        } else {
            alpha_type
        }
    }

    let (data, bpl, size, color_type, alpha_type) = match buffer {
        SharedImageBuffer::RGB8(pixels) => {
            // RGB888 with one byte per component is not supported by Skia right now. Convert once to RGBA8 :-(
            let mut rgba = Vec::with_capacity(pixels.as_bytes().len() / 3 * 4);
            rgba.extend(
                pixels.as_bytes().as_chunks::<3>().0.iter().flat_map(|&[r, g, b]| [r, g, b, 255]),
            );
            (
                skia_safe::Data::new_with_owner(rgba),
                pixels.width() as usize * 4,
                pixels.size(),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Opaque,
            )
        }
        SharedImageBuffer::RGBA8(pixels) => (
            shared_pixel_data(pixels),
            pixels.width() as usize * 4,
            pixels.size(),
            skia_safe::ColorType::RGBA8888,
            opaque_or(pixels.as_bytes(), skia_safe::AlphaType::Unpremul),
        ),
        SharedImageBuffer::RGBA8Premultiplied(pixels) => (
            shared_pixel_data(pixels),
            pixels.width() as usize * 4,
            pixels.size(),
            skia_safe::ColorType::RGBA8888,
            opaque_or(pixels.as_bytes(), skia_safe::AlphaType::Premul),
        ),
        #[cfg(feature = "image-pixel-format-rgb565")]
        SharedImageBuffer::RGB565(pixels) => (
            shared_pixel_data(pixels),
            pixels.width() as usize * 2,
            pixels.size(),
            skia_safe::ColorType::RGB565,
            skia_safe::AlphaType::Opaque,
        ),
        #[cfg(feature = "image-pixel-format-gray8")]
        SharedImageBuffer::Gray8(pixels) => (
            shared_pixel_data(pixels),
            pixels.width() as usize,
            pixels.size(),
            skia_safe::ColorType::Gray8,
            skia_safe::AlphaType::Opaque,
        ),
        _ => return None,
    };
    let image_info = crate::image_info(
        skia_safe::ISize::new(size.width as i32, size.height as i32),
        color_type,
        alpha_type,
    );
    skia_safe::images::raster_from_data(&image_info, data, bpl)
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_core::graphics::{Rgb8Pixel, load_image_from_embedded_data};
    use i_slint_core::slice::Slice;

    static RED_PNG: &[u8] = include_bytes!("../../../tests/screenshots/cases/image/tiny-1x1.png");

    fn to_skia(image: Image) -> skia_safe::Image {
        let mut surface = skia_safe::surfaces::raster_n32_premul((1, 1)).unwrap();
        as_skia_image(
            image,
            &|| LogicalSize::new(1., 1.),
            ImageFit::Fill,
            ScaleFactor::new(1.),
            1.,
            surface.canvas(),
            None,
        )
        .unwrap()
    }

    #[test]
    fn keyed_images_share_skia_image() {
        let load =
            || load_image_from_embedded_data(Slice::from_slice(RED_PNG), Slice::from_slice(b"png"));
        let (first, second) = (load(), load());
        let first = to_skia(first);
        let second = to_skia(second);
        assert_eq!(first.unique_id(), second.unique_id());
        let pixels = first.peek_pixels().unwrap();
        assert_eq!(pixels.color_type(), skia_safe::ColorType::RGBA8888);
        assert_eq!(pixels.bytes().unwrap(), &[255, 0, 0, 255]);
    }

    #[test]
    fn images_without_cache_key_keep_their_pixels() {
        let rgb = |pixel: &[u8]| {
            Image::from_rgb8(SharedPixelBuffer::<Rgb8Pixel>::clone_from_slice(pixel, 1, 1))
        };
        let red = to_skia(rgb(&[255, 0, 0]));
        let blue = to_skia(rgb(&[0, 0, 255]));
        assert_eq!(red.peek_pixels().unwrap().bytes().unwrap(), &[255, 0, 0, 255]);
        assert_eq!(blue.peek_pixels().unwrap().bytes().unwrap(), &[0, 0, 255, 255]);
    }
}
