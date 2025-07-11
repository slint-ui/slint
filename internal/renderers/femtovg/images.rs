// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashMap;
use std::rc::Rc;

use i_slint_core::graphics::euclid;
#[cfg(not(target_arch = "wasm32"))]
use i_slint_core::graphics::BorrowedOpenGLTexture;
use i_slint_core::graphics::{ImageCacheKey, IntSize, SharedImageBuffer};
use i_slint_core::items::ImageTiling;
use i_slint_core::lengths::PhysicalPx;
use i_slint_core::{items::ImageRendering, ImageInner};

use super::itemrenderer::CanvasRc;

pub trait TextureImporter
where
    Self: femtovg::Renderer + Sized,
{
    #[cfg(not(target_family = "wasm"))]
    fn convert_opengl_texture(opengl_texture: std::num::NonZero<u32>) -> Self::NativeTexture;

    #[cfg(feature = "unstable-wgpu-25")]
    fn convert_wgpu_25_texture(wgpu_texture: wgpu_25::Texture) -> Self::NativeTexture;
}

impl TextureImporter for femtovg::renderer::OpenGl {
    #[cfg(not(target_family = "wasm"))]
    fn convert_opengl_texture(opengl_texture: std::num::NonZero<u32>) -> Self::NativeTexture {
        glow::NativeTexture(opengl_texture)
    }

    #[cfg(feature = "unstable-wgpu-25")]
    fn convert_wgpu_25_texture(_wgpu_texture: wgpu_25::Texture) -> Self::NativeTexture {
        unimplemented!()
    }
}

#[cfg(all(feature = "wgpu", not(target_family = "wasm")))]
impl TextureImporter for femtovg::renderer::WGPURenderer {
    fn convert_opengl_texture(_opengl_texture: std::num::NonZero<u32>) -> Self::NativeTexture {
        todo!()
    }

    #[cfg(feature = "unstable-wgpu-25")]
    fn convert_wgpu_25_texture(wgpu_texture: wgpu_25::Texture) -> Self::NativeTexture {
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

    // Upload the image to the GPU. This function could take just a canvas as parameter,
    // but since an upload requires a current context, this is "enforced" by taking
    // a renderer instead (which implies a current context).
    pub fn new_from_image(
        image: &ImageInner,
        canvas: &CanvasRc<R>,
        target_size_for_scalable_source: Option<euclid::Size2D<u32, PhysicalPx>>,
        scaling: ImageRendering,
        tiling: (ImageTiling, ImageTiling),
    ) -> Option<Rc<Self>> {
        let image_flags = base_image_flags(scaling, tiling);

        let image_id = match image {
            #[cfg(target_arch = "wasm32")]
            ImageInner::HTMLImage(html_image) => {
                if html_image.size().is_some() {
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
            #[cfg(all(not(target_arch = "wasm32"), feature = "unstable-wgpu-25"))]
            ImageInner::WGPUTexture(any_wgpu_texture) => {
                let texture = match any_wgpu_texture {
                    i_slint_core::graphics::WGPUTexture::WGPU25Texture(texture) => texture.clone(),
                };
                let size = texture.size();

                canvas
                    .borrow_mut()
                    .create_image_from_native_texture(
                        <R as TextureImporter>::convert_wgpu_25_texture(texture),
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
                let buffer = image.render_to_buffer(target_size_for_scalable_source)?;
                let (image_source, flags) = image_buffer_to_image_source(&buffer);
                canvas.borrow_mut().create_image(image_source, image_flags | flags).unwrap()
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
pub struct TextureCache<R: femtovg::Renderer + TextureImporter>(
    HashMap<TextureCacheKey, Rc<Texture<R>>>,
);

impl<R: femtovg::Renderer + TextureImporter> Default for TextureCache<R> {
    fn default() -> Self {
        Self(Default::default())
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
        Some(match self.0.entry(cache_key) {
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
        self.0.retain(|_, cached_image| {
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
        self.0.clear();
    }
}

fn image_buffer_to_image_source(
    buffer: &SharedImageBuffer,
) -> (femtovg::ImageSource<'_>, femtovg::ImageFlags) {
    match buffer {
        SharedImageBuffer::RGB8(buffer) => (
            {
                imgref::ImgRef::new(buffer.as_slice(), buffer.width() as _, buffer.height() as _)
                    .into()
            },
            femtovg::ImageFlags::empty(),
        ),
        SharedImageBuffer::RGBA8(buffer) => (
            {
                imgref::ImgRef::new(buffer.as_slice(), buffer.width() as _, buffer.height() as _)
                    .into()
            },
            femtovg::ImageFlags::empty(),
        ),
        SharedImageBuffer::RGBA8Premultiplied(buffer) => (
            {
                imgref::ImgRef::new(buffer.as_slice(), buffer.width() as _, buffer.height() as _)
                    .into()
            },
            femtovg::ImageFlags::PREMULTIPLIED,
        ),
    }
}

pub fn base_image_flags(
    scaling: ImageRendering,
    tiling: (ImageTiling, ImageTiling),
) -> femtovg::ImageFlags {
    let image_flags = match scaling {
        ImageRendering::Smooth => femtovg::ImageFlags::empty(),
        ImageRendering::Pixelated => femtovg::ImageFlags::NEAREST,
    } | match tiling.0 {
        ImageTiling::None => femtovg::ImageFlags::empty(),
        ImageTiling::Repeat | ImageTiling::Round => femtovg::ImageFlags::REPEAT_X,
    } | match tiling.1 {
        ImageTiling::None => femtovg::ImageFlags::empty(),
        ImageTiling::Repeat | ImageTiling::Round => femtovg::ImageFlags::REPEAT_Y,
    };
    image_flags
}
