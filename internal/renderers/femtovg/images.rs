// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashMap;
use std::rc::Rc;

use i_slint_core::graphics::BorrowedOpenGLTexture;
#[cfg(feature = "image-pixel-format-rgb565")]
use i_slint_core::graphics::Rgb8Pixel;
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
    fn convert_opengl_texture(opengl_texture: &BorrowedOpenGLTexture) -> Self::NativeTexture;

    #[cfg(feature = "unstable-wgpu-30")]
    fn convert_wgpu_30_texture(wgpu_texture: wgpu_30::Texture) -> Self::NativeTexture;
}

impl TextureImporter for femtovg::renderer::OpenGl {
    fn convert_opengl_texture(opengl_texture: &BorrowedOpenGLTexture) -> Self::NativeTexture {
        #[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
        return glow::NativeTexture(opengl_texture.texture_id);
        #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
        return opengl_texture.texture.clone();
    }

    #[cfg(feature = "unstable-wgpu-30")]
    fn convert_wgpu_30_texture(_wgpu_texture: wgpu_30::Texture) -> Self::NativeTexture {
        unimplemented!()
    }
}

#[cfg(feature = "wgpu-30")]
impl TextureImporter for femtovg::renderer::WGPURenderer {
    fn convert_opengl_texture(_opengl_texture: &BorrowedOpenGLTexture) -> Self::NativeTexture {
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

        let mut canvas = self.canvas.borrow_mut();
        // filter_image() clamps a Gaussian blur to the sigma of one shader pass (8); a chain runs
        // larger blurs at a downsampled size instead. If the chain can't run (e.g. its scratch
        // images don't fit the transient budget), the clamped single pass is better than nothing.
        if canvas.filter_image_chain(filtered_image.id, &[filter], self.id).is_err() {
            canvas.filter_image(filtered_image.id, filter, self.id);
        }
        drop(canvas);

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
        context: &i_slint_core::SlintContext,
    ) -> Option<Rc<Self>> {
        let image_flags = base_image_flags(scaling, tiling);

        let image_id = match image {
            #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
            ImageInner::HTMLImage(html_image) => {
                if html_image.is_loaded() {
                    // Anecdotal evidence suggests that HTMLImageElement converts to a texture with
                    // pre-multiplied alpha. It's possible that this is not generally applicable, but it
                    // is the case for SVGs.
                    let image_flags = if html_image.is_svg() {
                        if let Some(mut target_size) = target_size_for_scalable_source {
                            // Rasterize at the SVG's aspect ratio, like the native SVG rendering does,
                            // and let image-fit stretch the texture. Otherwise the browser applies the
                            // SVG's preserveAspectRatio and bakes transparent bars into the texture.
                            if let Some(source_size) = html_image.svg_intrinsic_size() {
                                target_size = i_slint_core::graphics::aspect_preserving_render_size(
                                    source_size,
                                    target_size,
                                );
                            }
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
            ImageInner::BorrowedOpenGLTexture(opengl_texture) => {
                let size = opengl_texture.size;
                let image_flags = match opengl_texture.origin {
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
                        <R as TextureImporter>::convert_opengl_texture(opengl_texture),
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
            ImageInner::WGPUTexture(texture) => {
                #[allow(irrefutable_let_patterns)]
                let i_slint_core::graphics::WGPUTexture::WGPU30Texture(texture) = &**texture else {
                    return None;
                };
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
                let buffer = image
                    .render_to_premultiplied_buffer(target_size_for_scalable_source, context)?;
                // femtovg has no 16-bit texture format; expand to RGB8 for the upload.
                #[cfg(feature = "image-pixel-format-rgb565")]
                let buffer = if let SharedImageBuffer::RGB565(b) = &buffer {
                    let mut rgb = SharedPixelBuffer::<Rgb8Pixel>::new(b.width(), b.height());
                    for (dst, src) in rgb.make_mut_slice().iter_mut().zip(b.as_slice()) {
                        *dst = (*src).into();
                    }
                    SharedImageBuffer::RGB8(rgb)
                } else {
                    buffer
                };
                let (image_source, flags) = image_buffer_to_image_source(&buffer)?;
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

struct CachedTexture<R: femtovg::Renderer + TextureImporter> {
    texture: Rc<Texture<R>>,
    last_use: u64,
    /// The size in bytes, or `None` for a web image that's still loading.
    weight: Option<usize>,
}

/// The textures made from images, so that an image isn't decoded and uploaded on every frame.
/// It keeps the textures that items hold, plus the most recently used others within a budget.
pub struct TextureCache<R: femtovg::Renderer + TextureImporter> {
    textures: HashMap<TextureCacheKey, CachedTexture<R>>,
    /// The sum of the known weights of the textures.
    total_weight: usize,
    clock: u64,
    /// The `clock` when the cache was last drained.
    drained_at: u64,
    budget: i_slint_core::graphics::cache::UnusedBudget,
}

impl<R: femtovg::Renderer + TextureImporter> Default for TextureCache<R> {
    fn default() -> Self {
        Self {
            textures: HashMap::new(),
            total_weight: 0,
            clock: 0,
            drained_at: 0,
            budget: Default::default(),
        }
    }
}

/// Returns the size in bytes of `texture`, or `None` while it's loading.
fn texture_weight<R: femtovg::Renderer + TextureImporter>(texture: &Texture<R>) -> Option<usize> {
    let size = texture.size()?;
    Some(size.width as usize * size.height as usize * 4)
}

impl<R: femtovg::Renderer + TextureImporter> TextureCache<R> {
    /// Returns the texture of `cache_key`, or makes it with `image_create_fn`.
    pub(crate) fn lookup_image_in_cache_or_create(
        &mut self,
        cache_key: TextureCacheKey,
        image_create_fn: impl Fn() -> Option<Rc<Texture<R>>>,
    ) -> Option<Rc<Texture<R>>> {
        self.clock += 1;
        let last_use = self.clock;
        match self.textures.entry(cache_key) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.get_mut().last_use = last_use;
                Some(entry.get().texture.clone())
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                let texture = image_create_fn()?;
                let weight = texture_weight(&texture);
                self.total_weight += weight.unwrap_or(0);
                entry.insert(CachedTexture { texture: texture.clone(), last_use, weight });
                Some(texture)
            }
        }
    }

    /// Drops the least recently used textures that nothing holds, beyond the budget,
    /// after a window of `window_size` was drawn.
    /// It keeps the textures used while drawing it, and the images that are still loading
    /// on the web, whose size isn't known yet.
    pub(crate) fn drain(&mut self, window_size: i_slint_core::api::PhysicalSize) {
        self.budget.window_drawn(window_size);
        let drained_at = core::mem::replace(&mut self.drained_at, self.clock);
        for cached in self.textures.values_mut().filter(|cached| cached.weight.is_none()) {
            cached.weight = texture_weight(&cached.texture);
            self.total_weight += cached.weight.unwrap_or(0);
        }
        if self.budget.fits(self.total_weight) {
            return;
        }
        // The size in bytes of a texture that may be dropped.
        let droppable = |cached: &CachedTexture<R>| {
            let unused = Rc::strong_count(&cached.texture) == 1 && cached.last_use <= drained_at;
            unused.then_some(cached.weight).flatten()
        };
        let unused =
            self.textures.values().filter_map(|cached| Some((cached.last_use, droppable(cached)?)));
        let Some(cutoff) = self.budget.cutoff(unused) else { return };
        let total_weight = &mut self.total_weight;
        self.textures.retain(|_, cached| {
            let dropped = cached.last_use <= cutoff && droppable(cached).is_some();
            if dropped {
                *total_weight -= cached.weight.unwrap_or(0);
            }
            !dropped
        });
    }

    pub(crate) fn clear(&mut self) {
        self.textures.clear();
        self.total_weight = 0;
    }
}

fn image_buffer_to_image_source(
    buffer: &SharedImageBuffer,
) -> Option<(femtovg::ImageSource<'_>, femtovg::ImageFlags)> {
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
        // Expanded to RGB8 before upload; see Texture::new_from_image.
        #[cfg(feature = "image-pixel-format-rgb565")]
        SharedImageBuffer::RGB565(..) => {
            unreachable!("RGB565 buffers are converted to RGB8 before the femtovg upload")
        }
        SharedImageBuffer::RGB8(buffer) => {
            Some((image_source(buffer).into(), femtovg::ImageFlags::empty()))
        }
        SharedImageBuffer::RGBA8(buffer) => {
            Some((image_source(buffer).into(), femtovg::ImageFlags::empty()))
        }
        SharedImageBuffer::RGBA8Premultiplied(buffer) => {
            Some((image_source(buffer).into(), femtovg::ImageFlags::PREMULTIPLIED))
        }
        #[cfg(feature = "image-pixel-format-gray8")]
        SharedImageBuffer::Gray8(buffer) => {
            use rgb::FromSlice;
            let source = image_source(buffer);
            // femtovg takes `rgb::Gray` pixels, which have the same one-byte layout.
            let pixels = buffer.as_bytes()[..source.buf().len()].as_gray();
            Some((
                imgref::ImgRef::new(pixels, source.width(), source.height()).into(),
                femtovg::ImageFlags::empty(),
            ))
        }
        _ => None,
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
