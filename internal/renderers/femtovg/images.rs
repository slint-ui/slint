// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashMap;
use std::rc::Rc;

#[cfg(not(target_arch = "wasm32"))]
use i_slint_core::graphics::BorrowedOpenGLTexture;
use i_slint_core::graphics::euclid;
use i_slint_core::graphics::{ImageCacheKey, IntSize, SharedImageBuffer, SharedPixelBuffer};
use i_slint_core::items::ImageTiling;
use i_slint_core::lengths::PhysicalPx;
use i_slint_core::{ImageInner, items::ImageRendering};

use super::itemrenderer::CanvasRc;

pub trait TextureImporter
where
    Self: femtovg::Renderer + Sized,
{
    #[cfg(not(target_family = "wasm"))]
    fn convert_opengl_texture(opengl_texture: std::num::NonZero<u32>) -> Self::NativeTexture;

    #[cfg(feature = "unstable-wgpu-30")]
    fn convert_wgpu_30_texture(wgpu_texture: wgpu_30::Texture) -> Self::NativeTexture;
}

impl TextureImporter for femtovg::renderer::OpenGl {
    #[cfg(not(target_family = "wasm"))]
    fn convert_opengl_texture(opengl_texture: std::num::NonZero<u32>) -> Self::NativeTexture {
        glow::NativeTexture(opengl_texture)
    }

    #[cfg(feature = "unstable-wgpu-30")]
    fn convert_wgpu_30_texture(_wgpu_texture: wgpu_30::Texture) -> Self::NativeTexture {
        unimplemented!()
    }
}

#[cfg(feature = "wgpu-30")]
impl TextureImporter for femtovg::renderer::WGPURenderer {
    #[cfg(not(target_family = "wasm"))]
    fn convert_opengl_texture(_opengl_texture: std::num::NonZero<u32>) -> Self::NativeTexture {
        todo!()
    }

    #[cfg(feature = "unstable-wgpu-30")]
    fn convert_wgpu_30_texture(wgpu_texture: wgpu_30::Texture) -> Self::NativeTexture {
        wgpu_texture
    }
}
pub struct Texture<R: femtovg::Renderer + TextureImporter> {
    pub id: femtovg::ImageId,
    canvas: CanvasRc<R>,
}

impl<R: femtovg::Renderer + TextureImporter> Texture<R> {
    pub fn size(&self) -> Option<IntSize> {
        self.canvas
            .borrow()
            .image_info(self.id)
            .map(|info| [info.width() as u32, info.height() as u32].into())
            .ok()
    }

    pub fn as_render_target(&self) -> femtovg::RenderTarget {
        femtovg::RenderTarget::Image(self.id)
    }

    pub fn adopt(canvas: &CanvasRc<R>, image_id: femtovg::ImageId) -> Rc<Texture<R>> {
        Texture { id: image_id, canvas: canvas.clone() }.into()
    }

    pub fn new_empty_on_gpu(
        canvas: &CanvasRc<R>,
        width: u32,
        height: u32,
    ) -> Option<Rc<Texture<R>>> {
        if width == 0 || height == 0 {
            return None;
        }
        let image_id = canvas
            .borrow_mut()
            .create_image_empty(
                width as usize,
                height as usize,
                femtovg::PixelFormat::Rgba8,
                femtovg::ImageFlags::PREMULTIPLIED | femtovg::ImageFlags::FLIP_Y,
            )
            .unwrap();
        Some(Self { canvas: canvas.clone(), id: image_id }.into())
    }

    pub(crate) fn filter(&self, filter: femtovg::ImageFilter) -> Rc<Self> {
        let size = self.size().unwrap();
        let filtered_image = Self::new_empty_on_gpu(&self.canvas, size.width, size.height).expect(
            "internal error: this can only fail if the filtered image was zero width or height",
        );

        self.canvas.borrow_mut().filter_image(filtered_image.id, filter, self.id);

        filtered_image
    }

    pub fn as_paint(&self) -> femtovg::Paint {
        self.as_paint_with_alpha(1.0)
    }

    pub fn as_paint_with_alpha(&self, alpha_tint: f32) -> femtovg::Paint {
        let size = self
            .size()
            .expect("internal error: CachedImage::as_paint() called on zero-sized texture");
        femtovg::Paint::image(
            self.id,
            0.,
            0.,
            size.width as f32,
            size.height as f32,
            0.,
            alpha_tint,
        )
    }

    pub fn id(&self) -> femtovg::ImageId {
        self.id
    }

    // Upload the image to the GPU. This function could take just a canvas as parameter,
    // but since an upload requires a current context, this is "enforced" by taking
    // a renderer instead (which implies a current context).
    pub fn new_from_image(
        image: &ImageInner,
        canvas: &CanvasRc<R>,
        target_size_for_scalable_source: Option<euclid::Size2D<u32, PhysicalPx>>,
        scaling: ImageRendering,
        tiling: (ImageTiling, ImageTiling),
        max_texture_size: u32,
    ) -> Option<Rc<Self>> {
        let image_flags = base_image_flags(scaling, tiling);

        let image_id = match image {
            #[cfg(target_arch = "wasm32")]
            ImageInner::HTMLImage(html_image) => {
                if html_image.is_loaded() {
                    // Anecdotal evidence suggests that HTMLImageElement converts to a texture with
                    // pre-multiplied alpha. It's possible that this is not generally applicable, but it
                    // is the case for SVGs.
                    let image_flags = if html_image.is_svg() {
                        if let Some(target_size) = target_size_for_scalable_source {
                            let dom_element = &html_image.dom_element;
                            dom_element.set_width(target_size.width);
                            dom_element.set_height(target_size.height);
                        }
                        image_flags | femtovg::ImageFlags::PREMULTIPLIED
                    } else {
                        image_flags
                    };
                    canvas.borrow_mut().create_image(&html_image.dom_element, image_flags).unwrap()
                } else {
                    return None;
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            ImageInner::BorrowedOpenGLTexture(BorrowedOpenGLTexture {
                texture_id,
                size,
                origin,
                ..
            }) => {
                let image_flags = match origin {
                    i_slint_core::graphics::BorrowedOpenGLTextureOrigin::TopLeft => image_flags,
                    i_slint_core::graphics::BorrowedOpenGLTextureOrigin::BottomLeft => {
                        image_flags | femtovg::ImageFlags::FLIP_Y
                    }
                    _ => unimplemented!(
                        "internal error: missing implementation for BorrowedOpenGLTextureOrigin"
                    ),
                };
                canvas
                    .borrow_mut()
                    .create_image_from_native_texture(
                        <R as TextureImporter>::convert_opengl_texture(*texture_id),
                        femtovg::ImageInfo::new(
                            image_flags,
                            size.width as _,
                            size.height as _,
                            femtovg::PixelFormat::Rgba8,
                        ),
                    )
                    .unwrap()
            }
            #[cfg(feature = "unstable-wgpu-30")]
            ImageInner::WGPUTexture(i_slint_core::graphics::WGPUTexture::WGPU30Texture(
                texture,
            )) => {
                let texture = texture.clone();
                let size = texture.size();

                canvas
                    .borrow_mut()
                    .create_image_from_native_texture(
                        <R as TextureImporter>::convert_wgpu_30_texture(texture),
                        femtovg::ImageInfo::new(
                            image_flags,
                            size.width as _,
                            size.height as _,
                            femtovg::PixelFormat::Rgba8,
                        ),
                    )
                    .unwrap()
            }
            _ => {
                // Ask a scalable source to rasterize no larger than the GPU can hold.
                let target_size_for_scalable_source = target_size_for_scalable_source
                    .map(|size| fit_size_to_max_texture_size(size, max_texture_size));
                let buffer = fit_to_max_texture_size(
                    image.render_to_buffer(target_size_for_scalable_source)?,
                    max_texture_size,
                );
                let (image_source, flags) = image_buffer_to_image_source(&buffer);
                canvas.borrow_mut().create_image(image_source, image_flags | flags).ok()?
            }
        };

        Some(Self::adopt(canvas, image_id))
    }
}

impl<R: femtovg::Renderer + TextureImporter> Drop for Texture<R> {
    fn drop(&mut self) {
        self.canvas.borrow_mut().delete_image(self.id);
    }
}

#[derive(PartialEq, Eq, Hash, Debug)]
pub struct TextureCacheKey {
    source_key: ImageCacheKey,
    target_size_for_scalable_source: Option<euclid::Size2D<u32, PhysicalPx>>,
    gpu_image_flags: ImageRendering,
    gpu_image_tiling: (ImageTiling, ImageTiling),
}

impl TextureCacheKey {
    pub fn new(
        resource: &ImageInner,
        target_size_for_scalable_source: Option<euclid::Size2D<u32, PhysicalPx>>,
        gpu_image_flags: ImageRendering,
        gpu_image_tiling: (ImageTiling, ImageTiling),
    ) -> Option<Self> {
        ImageCacheKey::new(resource).map(|source_key| Self {
            source_key,
            target_size_for_scalable_source,
            gpu_image_flags,
            gpu_image_tiling,
        })
    }
}

// Cache used to avoid repeatedly decoding images from disk. Entries with a count
// of 1 are drained after flushing the renderer commands to the screen.
pub struct TextureCache<R: femtovg::Renderer + TextureImporter> {
    textures: HashMap<TextureCacheKey, Rc<Texture<R>>>,
    /// Every texture in here was uploaded against this limit: it only changes along with the
    /// graphics context, which clears the cache. See [`crate::GraphicsBackend::max_texture_size`].
    pub(crate) max_texture_size: u32,
}

impl<R: femtovg::Renderer + TextureImporter> Default for TextureCache<R> {
    fn default() -> Self {
        Self { textures: Default::default(), max_texture_size: u32::MAX }
    }
}

impl<R: femtovg::Renderer + TextureImporter> TextureCache<R> {
    // Look up the given image cache key in the image cache and upgrade the weak reference to a strong one if found,
    // otherwise a new image is created/loaded from the given callback.
    pub(crate) fn lookup_image_in_cache_or_create(
        &mut self,
        cache_key: TextureCacheKey,
        image_create_fn: impl Fn() -> Option<Rc<Texture<R>>>,
    ) -> Option<Rc<Texture<R>>> {
        Some(match self.textures.entry(cache_key) {
            std::collections::hash_map::Entry::Occupied(existing_entry) => {
                existing_entry.get().clone()
            }
            std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                let new_image = image_create_fn()?;
                vacant_entry.insert(new_image.clone());
                new_image
            }
        })
    }

    pub(crate) fn drain(&mut self) {
        self.textures.retain(|_, cached_image| {
            // * Retain images that are used by elements, so that they can be effectively
            // shared (one image element refers to foo.png, another element is created
            // and refers to the same -> share).
            // * Also retain images that are still loading (async HTML), where the size
            // is not known yet. Otherwise we end up in a loop where an image is not loaded
            // yet, we report (0, 0) to the layout, the image gets removed here, the closure
            // still triggers a load and marks the layout as dirt, which loads the
            // image again, etc.
            Rc::strong_count(cached_image) > 1 || cached_image.size().is_none()
        });
    }

    pub(crate) fn clear(&mut self) {
        self.textures.clear();
    }
}

fn image_buffer_to_image_source(
    buffer: &SharedImageBuffer,
) -> (femtovg::ImageSource<'_>, femtovg::ImageFlags) {
    fn image_source<Pixel: Clone>(buffer: &SharedPixelBuffer<Pixel>) -> imgref::ImgRef<'_, Pixel> {
        let pixels = buffer.as_slice();
        let read = buffer.width() as u64 * buffer.height() as u64;
        // `femtovg::Canvas::create_image` is unsound: it is safe to call, yet it hands the driver
        // the buffer's pointer along with the pixel count from `ImageSource::dimensions()`, so an
        // `ImgRef` that overstates its buffer reads out of bounds.
        assert!(pixels.len() as u64 >= read);
        imgref::ImgRef::new(&pixels[..read as usize], buffer.width() as _, buffer.height() as _)
    }

    match buffer {
        SharedImageBuffer::RGB8(buffer) => {
            (image_source(buffer).into(), femtovg::ImageFlags::empty())
        }
        SharedImageBuffer::RGBA8(buffer) => {
            (image_source(buffer).into(), femtovg::ImageFlags::empty())
        }
        SharedImageBuffer::RGBA8Premultiplied(buffer) => {
            (image_source(buffer).into(), femtovg::ImageFlags::PREMULTIPLIED)
        }
    }
}

pub fn base_image_flags(
    scaling: ImageRendering,
    tiling: (ImageTiling, ImageTiling),
) -> femtovg::ImageFlags {
    (match scaling {
        ImageRendering::Pixelated => femtovg::ImageFlags::NEAREST,
        ImageRendering::Smooth | _ => femtovg::ImageFlags::empty(),
    } | match tiling.0 {
        ImageTiling::Repeat | ImageTiling::Round => femtovg::ImageFlags::REPEAT_X,
        ImageTiling::None | _ => femtovg::ImageFlags::empty(),
    } | match tiling.1 {
        ImageTiling::Repeat | ImageTiling::Round => femtovg::ImageFlags::REPEAT_Y,
        ImageTiling::None | _ => femtovg::ImageFlags::empty(),
    })
}

fn fit_size_to_max_texture_size<U>(
    size: euclid::Size2D<u32, U>,
    max_texture_size: u32,
) -> euclid::Size2D<u32, U> {
    let longest_side = size.width.max(size.height);
    if longest_side <= max_texture_size {
        return size;
    }
    (size.to_f64() * (max_texture_size as f64 / longest_side as f64))
        .to_u32()
        .max(euclid::size2(1, 1))
}

/// A texture bigger than the graphics API's limit fails to allocate, and sampling the incomplete
/// texture that's left over returns opaque black (#11785).
fn fit_to_max_texture_size(buffer: SharedImageBuffer, max_texture_size: u32) -> SharedImageBuffer {
    let size = fit_size_to_max_texture_size(buffer.size(), max_texture_size);
    if size == buffer.size() {
        return buffer;
    }

    // An image whose source changes every frame is uploaded again on every frame.
    static REPORTED: std::sync::Once = std::sync::Once::new();
    REPORTED.call_once(|| {
        i_slint_core::debug_log!(
            "Slint: image of {}x{} pixels exceeds the maximum texture size of {max_texture_size}, scaling it down to {}x{}",
            buffer.width(),
            buffer.height(),
            size.width,
            size.height
        );
    });

    match buffer {
        SharedImageBuffer::RGB8(buffer) => {
            SharedImageBuffer::RGB8(downscale::<image::Rgb<u8>, _>(&buffer, size))
        }
        // Averaging straight alpha lets the color of a transparent pixel bleed into its
        // neighbors, so scale the premultiplied form and hand that back.
        SharedImageBuffer::RGBA8(buffer) => {
            let premultiplied = i_slint_core::graphics::Image::from_rgba8(buffer)
                .to_rgba8_premultiplied()
                .expect("internal error: an image built from a pixel buffer has pixels");
            SharedImageBuffer::RGBA8Premultiplied(downscale::<image::Rgba<u8>, _>(
                &premultiplied,
                size,
            ))
        }
        SharedImageBuffer::RGBA8Premultiplied(buffer) => {
            SharedImageBuffer::RGBA8Premultiplied(downscale::<image::Rgba<u8>, _>(&buffer, size))
        }
    }
}

fn downscale<P, Pixel>(source: &SharedPixelBuffer<Pixel>, size: IntSize) -> SharedPixelBuffer<Pixel>
where
    P: image::Pixel<Subpixel = u8> + 'static,
    Pixel: Clone + rgb::Pod,
    [Pixel]: rgb::ComponentBytes<u8>,
    [u8]: rgb::AsPixels<Pixel>,
{
    let view =
        image::ImageBuffer::<P, _>::from_raw(source.width(), source.height(), source.as_bytes())
            .expect("internal error: pixel buffer does not match its own dimensions");
    let scaled = image::imageops::thumbnail(&view, size.width, size.height);
    SharedPixelBuffer::clone_from_slice(scaled.as_raw(), size.width, size.height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_core::graphics::{Rgb8Pixel, Rgba8Pixel};

    #[test]
    fn fits_within_the_limit_untouched() {
        let buffer = SharedImageBuffer::RGB8(SharedPixelBuffer::new(8, 4));
        assert_eq!(fit_to_max_texture_size(buffer, 8).size(), IntSize::new(8, 4));
    }

    #[test]
    fn keeps_the_aspect_ratio_when_fitting_a_size() {
        let fit = |width, height| {
            fit_size_to_max_texture_size(euclid::Size2D::<u32, ()>::new(width, height), 100)
        };
        assert_eq!(fit(80, 40), euclid::size2(80, 40));
        assert_eq!(fit(400, 200), euclid::size2(100, 50));
        // A side that rounds down to nothing still has to be uploadable.
        assert_eq!(fit(100000, 1), euclid::size2(100, 1));
    }

    #[test]
    fn averages_the_colors_it_merges() {
        let mut source = SharedPixelBuffer::<Rgb8Pixel>::new(4, 2);
        // Left half is black and white, right half is a uniform gray.
        for (i, pixel) in source.make_mut_slice().iter_mut().enumerate() {
            let value = match i % 4 {
                0 => 0,
                1 => 100,
                _ => 60,
            };
            *pixel = Rgb8Pixel { r: value, g: value, b: value };
        }

        let scaled = fit_to_max_texture_size(SharedImageBuffer::RGB8(source), 2);
        assert_eq!(scaled.size(), IntSize::new(2, 1));
        let SharedImageBuffer::RGB8(scaled) = scaled else { panic!("format changed") };
        assert_eq!(scaled.as_slice()[0].r, 50);
        assert_eq!(scaled.as_slice()[1].r, 60);
    }

    #[test]
    fn ignores_the_color_of_fully_transparent_pixels() {
        let mut source = SharedPixelBuffer::<Rgba8Pixel>::new(2, 1);
        source.make_mut_slice().copy_from_slice(&[
            Rgba8Pixel { r: 200, g: 200, b: 200, a: 255 },
            Rgba8Pixel { r: 10, g: 10, b: 10, a: 0 },
        ]);

        let scaled = fit_to_max_texture_size(SharedImageBuffer::RGBA8(source), 1);
        let SharedImageBuffer::RGBA8Premultiplied(scaled) = scaled else {
            panic!("scaling straight alpha has to produce premultiplied pixels")
        };
        // Half of the opaque white pixel, and none of the transparent one.
        let pixel = scaled.as_slice()[0];
        assert_eq!((pixel.r, pixel.a), (100, 128));
    }
}
