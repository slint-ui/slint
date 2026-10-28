// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore EPOC htmlimage
/*!
This module contains image decoding and caching related types for the run-time library.
*/

use crate::lengths::{LogicalSize, PhysicalPx, ScaleFactor};
#[allow(unused)]
use crate::slice::Slice;
#[allow(unused)]
use crate::{SharedString, SharedVector};
#[cfg(any(
    feature = "image-decoders",
    all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std")
))]
use alloc::rc::Rc;

#[cfg(feature = "image-owned-textures")]
use super::OwnedStorage;
use super::{IntRect, IntSize, StaticStorage};
use crate::items::{ImageFit, ImageHorizontalAlignment, ImageTiling, ImageVerticalAlignment};
#[cfg(not(feature = "std"))]
use num_traits::Float;

/// Sets a custom image loader callback.
///
/// The callback receives an asset identifier (a string) and should return:
/// - `Some(Ok(bytes))` if it can provide the asset data
/// - `Some(Err(msg))` to abort loading with an error
/// - `None` to fall through to the default file system loading
///
/// This is useful for loading assets from custom sources such as embedded resources,
/// network locations, or in-memory storage.
#[cfg(any(
    feature = "image-decoders",
    all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std")
))]
pub fn set_image_loader(
    loader: impl Fn(&str) -> Option<Result<alloc::vec::Vec<u8>, alloc::string::String>> + 'static,
) {
    use crate::graphics::image::cache::IMAGE_CACHE;
    IMAGE_CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        cache.set_image_loader(Some(Rc::new(loader)));
    });
}

mod animated;
#[cfg(any(
    feature = "image-decoders",
    all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std")
))]
pub mod cache;
pub use animated::AnimatedImage;
#[cfg(all(
    feature = "image-decoders",
    any(not(target_arch = "wasm32"), target_os = "emscripten")
))]
pub(crate) use animated::{Frame, FrameDecoder};
#[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
mod htmlimage;
#[cfg(feature = "svg")]
mod svg;

#[allow(missing_docs)]
#[cfg_attr(not(feature = "ffi"), i_slint_core_macros::remove_extern)]
#[vtable::vtable]
#[repr(C)]
pub struct OpaqueImageVTable {
    drop_in_place: extern "C" fn(VRefMut<OpaqueImageVTable>) -> Layout,
    dealloc: extern "C" fn(&OpaqueImageVTable, ptr: *mut u8, layout: Layout),
    /// Returns the image size
    size: extern "C" fn(VRef<OpaqueImageVTable>) -> IntSize,
    /// Returns a cache key
    cache_key: extern "C" fn(VRef<OpaqueImageVTable>) -> ImageCacheKey,
}

#[cfg(feature = "svg")]
OpaqueImageVTable_static! {
    /// VTable for RC wrapped SVG helper struct.
    pub static PARSED_SVG_VT for svg::ParsedSVG
}

#[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
OpaqueImageVTable_static! {
    /// VTable for RC wrapped HtmlImage helper struct.
    pub static HTML_IMAGE_VT for htmlimage::HTMLImage
}

OpaqueImageVTable_static! {
    /// VTable for RC wrapped SVG helper struct.
    pub static NINE_SLICE_VT for NineSliceImage
}

OpaqueImageVTable_static! {
    /// VTable for RC wrapped animated images.
    pub static ANIMATED_IMAGE_VT for AnimatedImage
}

#[cfg(any(feature = "unstable-wgpu-29", feature = "unstable-wgpu-30"))]
OpaqueImageVTable_static! {
    /// VTable for RC wrapped WGPU textures.
    pub static WGPU_TEXTURE_VT for WGPUTexture
}

/// SharedPixelBuffer is a container for storing image data as pixels. It is
/// internally reference counted and cheap to clone.
///
/// You can construct a new empty shared pixel buffer with [`SharedPixelBuffer::new`],
/// or you can clone it from an existing contiguous buffer that you might already have, using
/// [`SharedPixelBuffer::clone_from_slice`].
///
/// See the documentation for [`Image`] for examples how to use this type to integrate
/// Slint with external rendering functions.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct SharedPixelBuffer<Pixel> {
    width: u32,
    height: u32,
    pub(crate) data: SharedVector<Pixel>,
}

impl<Pixel> SharedPixelBuffer<Pixel> {
    /// Returns the width of the image in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns the height of the image in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Returns the size of the image in pixels.
    pub fn size(&self) -> IntSize {
        [self.width, self.height].into()
    }
}

impl<Pixel: Clone> SharedPixelBuffer<Pixel> {
    /// Return a mutable slice to the pixel data. If the SharedPixelBuffer was shared, this will make a copy of the buffer.
    pub fn make_mut_slice(&mut self) -> &mut [Pixel] {
        self.data.make_mut_slice()
    }
}

impl<Pixel: Clone + rgb::Pod> SharedPixelBuffer<Pixel> {
    /// Returns the pixels interpreted as raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        rgb::bytemuck::cast_slice(self.data.as_slice())
    }

    /// Returns the pixels interpreted as raw bytes.
    pub fn make_mut_bytes(&mut self) -> &mut [u8] {
        rgb::bytemuck::cast_slice_mut(self.data.make_mut_slice())
    }
}

impl<Pixel> SharedPixelBuffer<Pixel> {
    /// Return a slice to the pixel data.
    pub fn as_slice(&self) -> &[Pixel] {
        self.data.as_slice()
    }
}

impl<Pixel: Clone + Default> SharedPixelBuffer<Pixel> {
    /// Creates a new SharedPixelBuffer with the given width and height. Each pixel will be initialized with the value
    /// that [`Default::default()`] returns for the Pixel type.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: core::iter::repeat_n(Pixel::default(), width as usize * height as usize)
                .collect(),
        }
    }
}

impl<Pixel: Clone> SharedPixelBuffer<Pixel> {
    /// Creates a new SharedPixelBuffer by cloning and converting pixels from an existing
    /// slice. This function is useful when another crate was used to allocate an image
    /// and you would like to convert it for use in Slint.
    ///
    /// The slice must hold exactly `width * height * bytes_per_pixel` bytes,
    /// where `bytes_per_pixel` is the size of the target pixel type:
    /// 4 for [`Rgba8Pixel`], 3 for [`Rgb8Pixel`], and 1 for [`Gray8Pixel`].
    /// This panics otherwise.
    #[track_caller]
    pub fn clone_from_slice<SourcePixelType>(
        pixel_slice: &[SourcePixelType],
        width: u32,
        height: u32,
    ) -> Self
    where
        [SourcePixelType]: rgb::AsPixels<Pixel>,
    {
        use rgb::AsPixels;
        let data: SharedVector<Pixel> = pixel_slice.as_pixels().into();
        assert_eq!(
            data.len() as u64,
            width as u64 * height as u64,
            "SharedPixelBuffer::clone_from_slice: the slice does not cover the requested {width}x{height} pixels",
        );
        Self { width, height, data }
    }
}

/// Convenience alias for a pixel with three color channels (red, green and blue), each
/// encoded as u8.
pub type Rgb8Pixel = rgb::RGB8;
/// Convenience alias for a pixel with four color channels (red, green, blue and alpha), each
/// encoded as u8.
pub type Rgba8Pixel = rgb::RGBA8;
/// A grayscale pixel: a single luminance channel encoded as u8,
/// between 0 (black) and 255 (white).
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Gray8Pixel(pub u8);

impl From<Gray8Pixel> for Rgb8Pixel {
    fn from(pixel: Gray8Pixel) -> Self {
        Self { r: pixel.0, g: pixel.0, b: pixel.0 }
    }
}

impl From<Gray8Pixel> for Rgba8Pixel {
    fn from(pixel: Gray8Pixel) -> Self {
        Self { r: pixel.0, g: pixel.0, b: pixel.0, a: 255 }
    }
}

impl rgb::AsPixels<Gray8Pixel> for [u8] {
    fn as_pixels(&self) -> &[Gray8Pixel] {
        bytemuck::cast_slice(self)
    }

    fn as_pixels_mut(&mut self) -> &mut [Gray8Pixel] {
        bytemuck::cast_slice_mut(self)
    }
}

impl From<crate::Color> for Rgb8Pixel {
    fn from(value: crate::Color) -> Self {
        Self { r: value.red(), g: value.green(), b: value.blue() }
    }
}

impl From<crate::Color> for Rgba8Pixel {
    fn from(value: crate::Color) -> Self {
        Self { r: value.red(), g: value.green(), b: value.blue(), a: value.alpha() }
    }
}

/// A 16bit pixel that has 5 red bits, 6 green bits and 5 blue bits
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Rgb565Pixel(pub u16);

impl Rgb565Pixel {
    const R_MASK: u16 = 0b1111_1000_0000_0000;
    const G_MASK: u16 = 0b0000_0111_1110_0000;
    const B_MASK: u16 = 0b0000_0000_0001_1111;

    /// Create a pixel from 8-bit red, green, and blue components.
    /// The low bits are truncated.
    #[inline]
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self(((r as u16 & 0b11111000) << 8) | ((g as u16 & 0b11111100) << 3) | (b as u16 >> 3))
    }

    /// Return the red component as a u8.
    ///
    /// The value is scaled so that a full 5-bit component yields 255.
    #[inline]
    pub const fn red(self) -> u8 {
        let r = (self.0 & Self::R_MASK) >> 11;
        ((r << 3) | (r >> 2)) as u8
    }
    /// Return the green component as a u8.
    ///
    /// The value is scaled so that a full 6-bit component yields 255.
    #[inline]
    pub const fn green(self) -> u8 {
        let g = (self.0 & Self::G_MASK) >> 5;
        ((g << 2) | (g >> 4)) as u8
    }
    /// Return the blue component as a u8.
    ///
    /// The value is scaled so that a full 5-bit component yields 255.
    #[inline]
    pub const fn blue(self) -> u8 {
        let b = self.0 & Self::B_MASK;
        ((b << 3) | (b >> 2)) as u8
    }
}

impl From<Rgb8Pixel> for Rgb565Pixel {
    fn from(p: Rgb8Pixel) -> Self {
        Self::from_rgb(p.r, p.g, p.b)
    }
}

impl From<Rgb565Pixel> for Rgb8Pixel {
    fn from(p: Rgb565Pixel) -> Self {
        Rgb8Pixel { r: p.red(), g: p.green(), b: p.blue() }
    }
}

impl From<Rgb565Pixel> for Rgba8Pixel {
    fn from(p: Rgb565Pixel) -> Self {
        Rgba8Pixel { r: p.red(), g: p.green(), b: p.blue(), a: 255 }
    }
}

#[cfg(any(feature = "image-pixel-format-rgb565", feature = "image-pixel-format-gray8"))]
fn map_pixels<Source: Clone, Dest: Clone + From<Source>>(
    buffer: SharedPixelBuffer<Source>,
) -> SharedPixelBuffer<Dest> {
    SharedPixelBuffer {
        width: buffer.width,
        height: buffer.height,
        data: buffer.data.into_iter().map(Dest::from).collect(),
    }
}

/// SharedImageBuffer is a container for images that are stored in CPU accessible memory.
///
/// The SharedImageBuffer's variants represent the different common formats for encoding
/// images in pixels.
#[derive(Clone, Debug)]
#[repr(C)]
#[non_exhaustive]
pub enum SharedImageBuffer {
    /// This variant holds the data for an image where each pixel has three color channels (red, green,
    /// and blue) and each channel is encoded as unsigned byte.
    RGB8(SharedPixelBuffer<Rgb8Pixel>),
    /// This variant holds the data for an image where each pixel has four color channels (red, green,
    /// blue and alpha) and each channel is encoded as unsigned byte.
    RGBA8(SharedPixelBuffer<Rgba8Pixel>),
    /// This variant holds the data for an image where each pixel has four color channels (red, green,
    /// blue and alpha) and each channel is encoded as unsigned byte. In contrast to [`Self::RGBA8`],
    /// this variant assumes that the alpha channel is also already multiplied to each red, green and blue
    /// component of each pixel.
    /// Only construct this format if you know that your pixels are encoded this way. It is more efficient
    /// for rendering.
    RGBA8Premultiplied(SharedPixelBuffer<Rgba8Pixel>),
    /// This variant holds the data for an image where each pixel is 16 bits: 5 red bits,
    /// 6 green bits, and 5 blue bits. This is the native format of many embedded displays.
    ///
    /// This variant is only available with the `image-pixel-format-rgb565` feature.
    #[cfg(feature = "image-pixel-format-rgb565")]
    RGB565(SharedPixelBuffer<Rgb565Pixel>),
    /// This variant holds the data for a grayscale image where each pixel is a single luminance
    /// channel encoded as unsigned byte.
    ///
    /// This variant is only available with the `image-pixel-format-gray8` feature.
    #[cfg(feature = "image-pixel-format-gray8")]
    Gray8(SharedPixelBuffer<Gray8Pixel>),
}

impl SharedImageBuffer {
    /// Returns the width of the image in pixels.
    #[inline]
    pub fn width(&self) -> u32 {
        self.size().width
    }

    /// Returns the height of the image in pixels.
    #[inline]
    pub fn height(&self) -> u32 {
        self.size().height
    }

    /// Returns the size of the image in pixels.
    #[inline]
    pub fn size(&self) -> IntSize {
        match self {
            Self::RGB8(buffer) => buffer.size(),
            Self::RGBA8(buffer) => buffer.size(),
            Self::RGBA8Premultiplied(buffer) => buffer.size(),
            #[cfg(feature = "image-pixel-format-rgb565")]
            Self::RGB565(buffer) => buffer.size(),
            #[cfg(feature = "image-pixel-format-gray8")]
            Self::Gray8(buffer) => buffer.size(),
        }
    }
}

impl PartialEq for SharedImageBuffer {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Self::RGB8(lhs_buffer) => {
                matches!(other, Self::RGB8(rhs_buffer) if lhs_buffer.data.as_ptr().eq(&rhs_buffer.data.as_ptr()))
            }
            Self::RGBA8(lhs_buffer) => {
                matches!(other, Self::RGBA8(rhs_buffer) if lhs_buffer.data.as_ptr().eq(&rhs_buffer.data.as_ptr()))
            }
            Self::RGBA8Premultiplied(lhs_buffer) => {
                matches!(other, Self::RGBA8Premultiplied(rhs_buffer) if lhs_buffer.data.as_ptr().eq(&rhs_buffer.data.as_ptr()))
            }
            #[cfg(feature = "image-pixel-format-rgb565")]
            Self::RGB565(lhs_buffer) => {
                matches!(other, Self::RGB565(rhs_buffer) if lhs_buffer.data.as_ptr().eq(&rhs_buffer.data.as_ptr()))
            }
            #[cfg(feature = "image-pixel-format-gray8")]
            Self::Gray8(lhs_buffer) => {
                matches!(other, Self::Gray8(rhs_buffer) if lhs_buffer.data.as_ptr().eq(&rhs_buffer.data.as_ptr()))
            }
        }
    }
}

pub use i_slint_common::pre_rendered::{StaticTexture, TexturePixelFormat};

/// cbindgen:ignore
pub type StaticTextures<S = StaticStorage> = i_slint_common::pre_rendered::StaticTextures<S>;

/// The pixels of the image that `textures` cover, in a buffer of `size`.
fn textures_to_buffer(size: IntSize, data: &[u8], textures: &[StaticTexture]) -> SharedImageBuffer {
    fn fill<const N: usize>(
        target: &mut [Rgba8Pixel],
        source: &[u8],
        pixel: impl Fn(&[u8; N]) -> Rgba8Pixel,
    ) {
        for (target, p) in target.iter_mut().zip(source.as_chunks::<N>().0) {
            *target = pixel(p);
        }
    }
    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(size.width, size.height);
    let stride = buffer.width() as usize;
    let slice = buffer.make_mut_slice();
    for t in textures {
        let rect = t.rect.to_usize();
        for y in 0..rect.height() {
            let slice = &mut slice[(rect.min_y() + y) * stride..][rect.x_range()];
            let source = &data[t.index + y * rect.width() * t.format.bpp()..];
            match t.format {
                TexturePixelFormat::Rgb => {
                    fill(slice, source, |p: &[u8; 3]| Rgba8Pixel::new(p[0], p[1], p[2], 255))
                }
                TexturePixelFormat::RgbaPremultiplied => {
                    fill(slice, source, |p: &[u8; 4]| Rgba8Pixel::new(p[0], p[1], p[2], p[3]))
                }
                TexturePixelFormat::Rgba => fill(slice, source, |p: &[u8; 4]| {
                    let pm = |c: u8| (c as u16 * p[3] as u16 / 255) as u8;
                    Rgba8Pixel::new(pm(p[0]), pm(p[1]), pm(p[2]), p[3])
                }),
                TexturePixelFormat::AlphaMap => {
                    let [a, r, g, b] = t.color.to_be_bytes().map(u32::from);
                    fill(slice, source, |p: &[u8; 1]| {
                        let a = p[0] as u32 * a;
                        let pm = |c: u32| (c * a / (255 * 255)) as u8;
                        Rgba8Pixel::new(pm(r), pm(g), pm(b), (a / 255) as u8)
                    })
                }
                #[cfg(feature = "image-pixel-format-gray8")]
                TexturePixelFormat::Gray8 => {
                    fill(slice, source, |p: &[u8; 1]| Gray8Pixel(p[0]).into())
                }
                TexturePixelFormat::SignedDistanceField => {
                    todo!("converting from a signed distance field to an image")
                }
                #[cfg(feature = "image-pixel-format-rgb565")]
                TexturePixelFormat::Rgb565 => {
                    fill(slice, source, |p: &[u8; 2]| Rgb565Pixel(u16::from_ne_bytes(*p)).into())
                }
            }
        }
    }
    SharedImageBuffer::RGBA8Premultiplied(buffer)
}

/// A struct that provides a path as a string as well as the last modification
/// time of the file it points to.
#[derive(PartialEq, Eq, Debug, Hash, Clone)]
#[repr(C)]
#[cfg(any(feature = "std", feature = "ffi"))]
pub struct CachedPath {
    path: SharedString,
    /// SystemTime since UNIX_EPOC as secs
    last_modified: u32,
}

#[cfg(all(feature = "image-decoders", any(not(target_arch = "wasm32"), target_os = "emscripten")))]
impl CachedPath {
    fn new<P: AsRef<std::path::Path>>(path: P) -> Self {
        let path_str = path.as_ref().to_string_lossy().as_ref().into();
        let timestamp = std::fs::metadata(path)
            .and_then(|md| md.modified())
            .unwrap_or(std::time::UNIX_EPOCH)
            .duration_since(std::time::UNIX_EPOCH)
            .map(|t| t.as_secs() as u32)
            .unwrap_or_default();
        Self { path: path_str, last_modified: timestamp }
    }
}

/// ImageCacheKey encapsulates the different ways of indexing images in the
/// cache of decoded images.
#[derive(PartialEq, Eq, Debug, Hash, Clone)]
#[repr(u8)]
pub enum ImageCacheKey {
    /// This variant indicates that no image cache key can be created for the image.
    /// For example this is the case for programmatically created images.
    Invalid = 0,
    #[cfg(any(feature = "std", feature = "ffi"))]
    /// The image is identified by its path on the file system and the last modification time stamp.
    Path(CachedPath) = 1,
    /// The image is identified by a URL.
    #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
    URL(SharedString) = 2,
    /// The image is identified by the static address of its encoded data.
    EmbeddedData(usize) = 3,
}

impl ImageCacheKey {
    /// Returns a new cache key if decoded image data can be stored in image cache for
    /// the given ImageInner.
    pub fn new(resource: &ImageInner) -> Option<Self> {
        let key = match resource {
            ImageInner::None => return None,
            ImageInner::EmbeddedImage { cache_key, .. } => cache_key.clone(),
            ImageInner::StaticTextures(textures) => {
                Self::from_embedded_image_data(textures.data.as_slice())
            }
            #[cfg(feature = "svg")]
            ImageInner::Svg(parsed_svg) => parsed_svg.cache_key().clone(),
            #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
            ImageInner::HTMLImage(htmlimage) => Self::URL(htmlimage.source().into()),
            ImageInner::BackendStorage(x) => vtable::VRc::borrow(x).cache_key(),
            #[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
            ImageInner::BorrowedOpenGLTexture(..) => return None,
            ImageInner::NineSlice(nine) => vtable::VRc::borrow(nine).cache_key(),
            #[cfg(any(feature = "unstable-wgpu-29", feature = "unstable-wgpu-30"))]
            ImageInner::WGPUTexture(..) => return None,
            ImageInner::AnimatedImage(animated) => vtable::VRc::borrow(animated).cache_key(),
            #[cfg(feature = "image-owned-textures")]
            ImageInner::OwnedTextures(..) => return None,
        };
        if matches!(key, ImageCacheKey::Invalid) { None } else { Some(key) }
    }

    /// Returns a cache key for static embedded image data.
    pub fn from_embedded_image_data(data: &'static [u8]) -> Self {
        Self::EmbeddedData(data.as_ptr() as usize)
    }
}

/// Represent a nine-slice image with the base image and the 4 borders
pub struct NineSliceImage(pub ImageInner, pub [u16; 4]);

impl NineSliceImage {
    /// return the backing Image
    pub fn image(&self) -> Image {
        Image(self.0.clone())
    }
}

impl OpaqueImage for NineSliceImage {
    fn size(&self) -> IntSize {
        self.0.size()
    }
    fn cache_key(&self) -> ImageCacheKey {
        ImageCacheKey::new(&self.0).unwrap_or(ImageCacheKey::Invalid)
    }
}

/// Represents a `wgpu::Texture` for each version of WGPU we support.
#[cfg(any(feature = "unstable-wgpu-29", feature = "unstable-wgpu-30"))]
#[derive(Clone, Debug)]
pub enum WGPUTexture {
    /// A texture for WGPU version 29.
    #[cfg(feature = "unstable-wgpu-29")]
    WGPU29Texture(wgpu_29::Texture),
    /// A texture for WGPU version 30.
    #[cfg(feature = "unstable-wgpu-30")]
    WGPU30Texture(wgpu_30::Texture),
}

#[cfg(any(feature = "unstable-wgpu-29", feature = "unstable-wgpu-30"))]
impl OpaqueImage for WGPUTexture {
    fn size(&self) -> IntSize {
        match self {
            #[cfg(feature = "unstable-wgpu-29")]
            Self::WGPU29Texture(texture) => {
                let size = texture.size();
                (size.width, size.height).into()
            }
            #[cfg(feature = "unstable-wgpu-30")]
            Self::WGPU30Texture(texture) => {
                let size = texture.size();
                (size.width, size.height).into()
            }
        }
    }
    fn cache_key(&self) -> ImageCacheKey {
        ImageCacheKey::Invalid
    }
}

/// A resource is a reference to binary data, for example images. They can be accessible on the file
/// system or embedded in the resulting binary. Or they might be URLs to a web server and a downloaded
/// is necessary before they can be used.
/// cbindgen:prefix-with-name
#[derive(Clone, Debug, Default)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum ImageInner {
    /// A resource that does not represent any data.
    #[default]
    None = 0,
    EmbeddedImage {
        cache_key: ImageCacheKey,
        buffer: SharedImageBuffer,
    } = 1,
    #[cfg(feature = "svg")]
    Svg(vtable::VRc<OpaqueImageVTable, svg::ParsedSVG>) = 2,
    #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
    HTMLImage(vtable::VRc<OpaqueImageVTable, htmlimage::HTMLImage>) = 4,
    BackendStorage(vtable::VRc<OpaqueImageVTable>) = 5,
    #[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
    BorrowedOpenGLTexture(BorrowedOpenGLTexture) = 6,
    NineSlice(vtable::VRc<OpaqueImageVTable, NineSliceImage>) = 7,
    #[cfg(any(feature = "unstable-wgpu-29", feature = "unstable-wgpu-30"))]
    WGPUTexture(vtable::VRc<OpaqueImageVTable, WGPUTexture>) = 8,
    AnimatedImage(vtable::VRc<OpaqueImageVTable, AnimatedImage>) = 9,
    #[cfg(feature = "image-owned-textures")]
    OwnedTextures(alloc::rc::Rc<StaticTextures<OwnedStorage>>) = 10,
    StaticTextures(&'static StaticTextures<StaticStorage>) = 11,
}

#[cfg(any(
    feature = "image-decoders",
    all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std")
))]
#[derive(Clone, Copy)]
pub(crate) enum ImageData<'a> {
    Static(&'static [u8]),
    Borrowed(&'a [u8]),
    /// Data that an animation can keep without a copy.
    #[cfg(all(
        feature = "image-decoders",
        any(not(target_arch = "wasm32"), target_os = "emscripten")
    ))]
    Shared(&'a SharedVector<u8>),
}

#[cfg(any(
    feature = "image-decoders",
    all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std")
))]
impl core::ops::Deref for ImageData<'_> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            Self::Static(data) => data,
            Self::Borrowed(data) => data,
            #[cfg(all(
                feature = "image-decoders",
                any(not(target_arch = "wasm32"), target_os = "emscripten")
            ))]
            Self::Shared(data) => data,
        }
    }
}

impl ImageCacheKey {
    #[cfg(feature = "std")]
    fn path(&self) -> Option<&SharedString> {
        match self {
            ImageCacheKey::Path(CachedPath { path, .. }) => Some(path),
            _ => None,
        }
    }
}

impl ImageInner {
    #[cfg(feature = "std")]
    fn path(&self) -> Option<&SharedString> {
        match self {
            ImageInner::EmbeddedImage { cache_key, .. } => cache_key.path(),
            ImageInner::AnimatedImage(animated) => animated.source_cache_key().path(),
            ImageInner::NineSlice(nine) => nine.0.path(),
            #[cfg(feature = "svg")]
            ImageInner::Svg(svg) => svg.cache_key().path(),
            _ => None,
        }
    }

    /// Return or render the image into a buffer
    ///
    /// `target_size_for_scalable_source` is the size to use if the image is scalable.
    /// (when unspecified, will default to the intrinsic size of the image)
    ///
    /// Returns None if the image can't be rendered in a buffer or if the image is empty
    pub fn render_to_buffer(
        &self,
        _target_size_for_scalable_source: Option<euclid::Size2D<u32, PhysicalPx>>,
    ) -> Option<SharedImageBuffer> {
        match self {
            ImageInner::EmbeddedImage { buffer, .. } => Some(buffer.clone()),
            #[cfg(feature = "svg")]
            ImageInner::Svg(svg) => match svg.render(_target_size_for_scalable_source) {
                Ok(b) => Some(b),
                // Ignore error when rendering a 0x0 image, that's just an empty image
                Err(resvg::usvg::Error::InvalidSize) => None,
                Err(err) => {
                    std::eprintln!("Error rendering SVG: {err}");
                    None
                }
            },
            ImageInner::StaticTextures(ts) => {
                Some(textures_to_buffer(ts.size, &ts.data, &ts.textures))
            }
            #[cfg(feature = "image-owned-textures")]
            ImageInner::OwnedTextures(textures) => {
                Some(textures_to_buffer(textures.size, &textures.data, &textures.textures))
            }
            ImageInner::NineSlice(nine) => nine.0.render_to_buffer(None),
            ImageInner::AnimatedImage(animated) => Some(animated.first_frame_buffer().clone()),
            _ => None,
        }
    }

    /// Returns true if the image is an SVG (either backed by resvg or HTML image wrapper).
    pub fn is_svg(&self) -> bool {
        match self {
            #[cfg(feature = "svg")]
            Self::Svg(_) => true,
            #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
            Self::HTMLImage(html_image) => html_image.is_svg(),
            _ => false,
        }
    }

    /// Return the image size
    pub fn size(&self) -> IntSize {
        match self {
            ImageInner::None => Default::default(),
            ImageInner::EmbeddedImage { buffer, .. } => buffer.size(),
            ImageInner::StaticTextures(StaticTextures { original_size, .. }) => *original_size,
            #[cfg(feature = "image-owned-textures")]
            ImageInner::OwnedTextures(textures) => textures.original_size,
            #[cfg(feature = "svg")]
            ImageInner::Svg(svg) => svg.size(),
            #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
            ImageInner::HTMLImage(htmlimage) => htmlimage.size().unwrap_or_default(),
            ImageInner::BackendStorage(x) => vtable::VRc::borrow(x).size(),
            #[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
            ImageInner::BorrowedOpenGLTexture(BorrowedOpenGLTexture { size, .. }) => *size,
            ImageInner::NineSlice(nine) => nine.0.size(),
            #[cfg(any(feature = "unstable-wgpu-29", feature = "unstable-wgpu-30"))]
            ImageInner::WGPUTexture(texture) => texture.size(),
            ImageInner::AnimatedImage(animated) => animated.size(),
        }
    }

    /// Internal helper to abstract over either loading from a file or parsing internal data.
    ///
    /// This can create an `ImageInner` with a dangling cache key reference if used incorrectly,
    /// which could lead to bad behavior. This constructor should be called from within
    /// `ImageCache::lookup_image_in_cache_or_create`, or `ImageCacheKey::Invalid` should be
    /// supplied.
    #[cfg(any(
        feature = "image-decoders",
        all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std")
    ))]
    pub(crate) fn load_from_data_with_cache_key(
        cache_key: ImageCacheKey,
        data: ImageData<'_>,
        format: Slice<'_, u8>,
    ) -> Option<Self> {
        // On the web, let the browser decode the image instead of shipping decoders in the binary.
        #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
        {
            let _ = cache_key;
            let mime_type = core::str::from_utf8(format.as_slice())
                .ok()
                .and_then(image_mime_type_from_extension)
                .unwrap_or_else(|| {
                    if data.starts_with(b"<?xml") || data.starts_with(b"<svg") {
                        "image/svg+xml"
                    } else {
                        // An empty type makes the browser sniff the format from the data.
                        ""
                    }
                });
            if mime_type == "image/svg+xml" && data.starts_with(&[0x1f, 0x8b]) {
                crate::debug_log!("Compressed SVG (.svgz) is not supported on the web");
                return None;
            }
            htmlimage::HTMLImage::new_from_data(&data, mime_type)
                .map(|html_image| ImageInner::HTMLImage(vtable::VRc::new(html_image)))
        }

        #[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
        {
            #[cfg(feature = "svg")]
            if format.as_slice() == b"svg"
                || format.as_slice() == b"svgz"
                || (format.is_empty() && (data.starts_with(b"<?xml") || data.starts_with(b"<svg")))
            {
                return Some(ImageInner::Svg(vtable::VRc::new(
                    svg::load_from_data(&data, cache_key).map_or_else(
                        |svg_err| {
                            crate::debug_log!("Error loading SVG: {}", svg_err);
                            None
                        },
                        Some,
                    )?,
                )));
            }

            let format = std::str::from_utf8(format.as_slice())
                .ok()
                .and_then(image::ImageFormat::from_extension);
            Self::decode_with_cache_key(cache_key, data, format)
                .map_err(|err| crate::debug_log!("Error decoding embedded image: {err}"))
                .ok()
        }
    }

    #[cfg(all(
        feature = "image-decoders",
        any(not(target_arch = "wasm32"), target_os = "emscripten")
    ))]
    pub(crate) fn decode_with_cache_key(
        cache_key: ImageCacheKey,
        data: ImageData<'_>,
        format: Option<image::ImageFormat>,
    ) -> image::ImageResult<Self> {
        let format = format.or_else(|| image::guess_format(&data).ok());
        // Files that the animation decoders reject may still decode as a still image.
        if let Some(image) = format.and_then(|format| {
            animated::load(cache_key.clone(), data, format)
                .map_err(|err| {
                    crate::debug_log!("Error decoding animation, showing a still image: {err}")
                })
                .ok()
                .flatten()
        }) {
            return Ok(image);
        }
        let image = match format {
            Some(format) => image::load_from_memory_with_format(&data, format),
            None => image::load_from_memory(&data),
        }?;
        Ok(ImageInner::EmbeddedImage {
            cache_key,
            buffer: dynamic_image_to_shared_image_buffer(image),
        })
    }
}

/// Convert `image::DynamicImage` to `SharedImageBuffer`
#[cfg(all(feature = "image-decoders", any(not(target_arch = "wasm32"), target_os = "emscripten")))]
fn dynamic_image_to_shared_image_buffer(dynamic_image: image::DynamicImage) -> SharedImageBuffer {
    if dynamic_image.color().has_alpha() {
        // Prefer pre-multiplied alpha so that smooth-scaling won't bleed the alpha when blending
        // in the renderers.
        SharedImageBuffer::RGBA8Premultiplied(rgba_image_to_premultiplied(
            dynamic_image.into_rgba8(),
        ))
    } else {
        let rgb8image = dynamic_image.into_rgb8();
        SharedImageBuffer::RGB8(SharedPixelBuffer::clone_from_slice(
            rgb8image.as_raw(),
            rgb8image.width(),
            rgb8image.height(),
        ))
    }
}

#[cfg(all(feature = "image-decoders", any(not(target_arch = "wasm32"), target_os = "emscripten")))]
fn rgba_image_to_premultiplied(image: image::RgbaImage) -> SharedPixelBuffer<Rgba8Pixel> {
    use rgb::AsPixels;
    SharedPixelBuffer {
        width: image.width(),
        height: image.height(),
        data: image
            .as_pixels()
            .iter()
            .map(|pixel| Image::rgba_to_premultiplied_rgba(*pixel))
            .collect(),
    }
}

impl PartialEq for ImageInner {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::None, Self::None) => true,
            (
                Self::EmbeddedImage { cache_key: l_cache_key, buffer: l_buffer },
                Self::EmbeddedImage { cache_key: r_cache_key, buffer: r_buffer },
            ) => l_cache_key == r_cache_key && l_buffer == r_buffer,
            #[cfg(feature = "svg")]
            (Self::Svg(l0), Self::Svg(r0)) => vtable::VRc::ptr_eq(l0, r0),
            (Self::StaticTextures(l0), Self::StaticTextures(r0)) => core::ptr::eq(*l0, *r0),
            #[cfg(feature = "image-owned-textures")]
            (Self::OwnedTextures(l0), Self::OwnedTextures(r0)) => alloc::rc::Rc::ptr_eq(l0, r0),
            #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
            (Self::HTMLImage(l0), Self::HTMLImage(r0)) => vtable::VRc::ptr_eq(l0, r0),
            (Self::BackendStorage(l0), Self::BackendStorage(r0)) => vtable::VRc::ptr_eq(l0, r0),
            #[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
            (Self::BorrowedOpenGLTexture(l0), Self::BorrowedOpenGLTexture(r0)) => l0 == r0,
            (Self::NineSlice(l), Self::NineSlice(r)) => l.0 == r.0 && l.1 == r.1,
            (Self::AnimatedImage(l), Self::AnimatedImage(r)) => **l == **r,
            _ => false,
        }
    }
}

impl<'a> From<&'a Image> for &'a ImageInner {
    fn from(other: &'a Image) -> Self {
        &other.0
    }
}

/// Error generated if an image cannot be loaded for any reasons.
#[derive(Default, Debug, PartialEq)]
pub struct LoadImageError(());

impl core::fmt::Display for LoadImageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("The image cannot be loaded")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for LoadImageError {}

/// An image type that can be displayed by the Image element. You can construct
/// Image objects from a path to an image file on disk, using [`Self::load_from_path`].
///
/// Clones of an Image share the same underlying data, so cloning is cheap.
///
/// Another typical use-case is to render the image content with Rust code.
/// For this it's most efficient to create a new SharedPixelBuffer with the known dimensions
/// and pass the mutable slice to your rendering function. Afterwards you can create an
/// Image.
///
/// The following example creates a 320x200 RGB pixel buffer and calls an external
/// low_level_render() function to draw a shape into it. Finally the result is
/// stored in an Image with [`Self::from_rgb8()`]:
/// ```
/// # use i_slint_core::graphics::{SharedPixelBuffer, Image, Rgb8Pixel};
///
/// fn low_level_render(width: u32, height: u32, buffer: &mut [u8]) {
///     // render beautiful circle or other shapes here
/// }
///
/// let mut pixel_buffer = SharedPixelBuffer::<Rgb8Pixel>::new(320, 200);
///
/// low_level_render(pixel_buffer.width(), pixel_buffer.height(),
///                  pixel_buffer.make_mut_bytes());
///
/// let image = Image::from_rgb8(pixel_buffer);
/// ```
///
/// Another use-case is to import existing image data into Slint, by
/// creating a new Image through cloning of another image type.
///
/// The following example uses the popular [image crate](https://docs.rs/image/) to
/// load a `.png` file from disk, apply brightening filter on it and then import
/// it into an [`Image`]:
/// ```no_run
/// # use i_slint_core::graphics::{SharedPixelBuffer, Image, Rgba8Pixel};
/// let mut cat_image = image::open("cat.png").expect("Error loading cat image").into_rgba8();
///
/// image::imageops::colorops::brighten_in_place(&mut cat_image, 20);
///
/// let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
///     cat_image.as_raw(),
///     cat_image.width(),
///     cat_image.height(),
/// );
/// let image = Image::from_rgba8(buffer);
/// ```
///
/// A popular software (CPU) rendering library in Rust is tiny-skia. The following example shows
/// how to use tiny-skia to render into a [`SharedPixelBuffer`]:
/// ```
/// # use i_slint_core::graphics::{SharedPixelBuffer, Image, Rgba8Pixel};
/// let mut pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::new(640, 480);
/// let width = pixel_buffer.width();
/// let height = pixel_buffer.height();
/// let mut pixmap = tiny_skia::PixmapMut::from_bytes(
///     pixel_buffer.make_mut_bytes(), width, height
/// ).unwrap();
/// pixmap.fill(tiny_skia::Color::TRANSPARENT);
///
/// let circle = tiny_skia::PathBuilder::from_circle(320., 240., 150.).unwrap();
///
/// let mut paint = tiny_skia::Paint::default();
/// paint.shader = tiny_skia::LinearGradient::new(
///     tiny_skia::Point::from_xy(100.0, 100.0),
///     tiny_skia::Point::from_xy(400.0, 400.0),
///     vec![
///         tiny_skia::GradientStop::new(0.0, tiny_skia::Color::from_rgba8(50, 127, 150, 200)),
///         tiny_skia::GradientStop::new(1.0, tiny_skia::Color::from_rgba8(220, 140, 75, 180)),
///     ],
///     tiny_skia::SpreadMode::Pad,
///     tiny_skia::Transform::identity(),
/// ).unwrap();
///
/// pixmap.fill_path(&circle, &paint, tiny_skia::FillRule::Winding, Default::default(), None);
///
/// let image = Image::from_rgba8_premultiplied(pixel_buffer);
/// ```
///
/// ### Sending Image to a thread
///
/// `Image` is not [`Send`], because it uses internal cache that are local to the Slint thread.
/// If you want to create image data in a thread and send that to slint, construct the
/// [`SharedPixelBuffer`] in a thread, and send that to Slint's UI thread.
///
/// ```rust,no_run
/// # use i_slint_core::graphics::{SharedPixelBuffer, Image, Rgba8Pixel};
/// std::thread::spawn(move || {
///     let mut pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::new(640, 480);
///     // ... fill the pixel_buffer with data as shown in the previous example ...
///     slint::invoke_from_event_loop(move || {
///         // this will run in the Slint's UI thread
///         let image = Image::from_rgba8_premultiplied(pixel_buffer);
///         // ... use the image, eg:
///         // my_ui_handle.upgrade().unwrap().set_image(image);
///     });
/// });
/// ```
#[repr(transparent)]
#[derive(Default, Clone, Debug, PartialEq, derive_more::From)]
pub struct Image(pub(crate) ImageInner);

impl Image {
    #[cfg(any(
        feature = "image-decoders",
        all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std")
    ))]
    /// Load an Image from a path to a file containing an image.
    ///
    /// Supported formats are SVG, PNG and JPEG.
    /// Enable support for additional formats supported by the [`image` crate](https://crates.io/crates/image) (
    /// AVIF, BMP, DDS, Farbfeld, GIF, HDR, ICO, JPEG, EXR, PNG, PNM, QOI, TGA, TIFF, WebP)
    /// by enabling the `image-default-formats` cargo feature.
    ///
    /// This function always fails on the web, where there is no file system.
    pub fn load_from_path(path: &std::path::Path) -> Result<Self, LoadImageError> {
        self::cache::IMAGE_CACHE.with(|global_cache| {
            let path: SharedString = path.to_str().ok_or(LoadImageError(()))?.into();
            global_cache.borrow_mut().load_image_from_path(&path).ok_or(LoadImageError(()))
        })
    }

    /// Creates a new Image from the specified shared pixel buffer, where each pixel has three color
    /// channels (red, green and blue) encoded as u8.
    pub fn from_rgb8(buffer: SharedPixelBuffer<Rgb8Pixel>) -> Self {
        Image(ImageInner::EmbeddedImage {
            cache_key: ImageCacheKey::Invalid,
            buffer: SharedImageBuffer::RGB8(buffer),
        })
    }

    /// Creates a new Image from the specified shared pixel buffer, where each pixel has four color
    /// channels (red, green, blue and alpha) encoded as u8.
    pub fn from_rgba8(buffer: SharedPixelBuffer<Rgba8Pixel>) -> Self {
        Image(ImageInner::EmbeddedImage {
            cache_key: ImageCacheKey::Invalid,
            buffer: SharedImageBuffer::RGBA8(buffer),
        })
    }

    /// Creates a new Image from the specified shared pixel buffer, where each pixel has four color
    /// channels (red, green, blue and alpha) encoded as u8 and, in contrast to [`Self::from_rgba8`],
    /// the alpha channel is also assumed to be multiplied to the red, green and blue channels.
    ///
    /// Only construct an Image with this function if you know that your pixels are encoded this way.
    pub fn from_rgba8_premultiplied(buffer: SharedPixelBuffer<Rgba8Pixel>) -> Self {
        Image(ImageInner::EmbeddedImage {
            cache_key: ImageCacheKey::Invalid,
            buffer: SharedImageBuffer::RGBA8Premultiplied(buffer),
        })
    }

    /// Creates a new Image from the specified shared pixel buffer, where each pixel is 16 bits:
    /// 5 red bits, 6 green bits, and 5 blue bits.
    ///
    /// This is the native format of many embedded displays. The software renderer can draw such
    /// images without any pixel conversion when the target is also RGB565.
    ///
    /// This function is only available with the `image-pixel-format-rgb565` feature.
    #[cfg(feature = "image-pixel-format-rgb565")]
    pub fn from_rgb565(buffer: SharedPixelBuffer<Rgb565Pixel>) -> Self {
        Image(ImageInner::EmbeddedImage {
            cache_key: ImageCacheKey::Invalid,
            buffer: SharedImageBuffer::RGB565(buffer),
        })
    }

    /// Creates a new Image from the specified shared pixel buffer, where each pixel is a single
    /// grayscale luminance value encoded as u8.
    ///
    /// This function is only available with the `image-pixel-format-gray8` feature.
    #[cfg(feature = "image-pixel-format-gray8")]
    pub fn from_gray8(buffer: SharedPixelBuffer<Gray8Pixel>) -> Self {
        Image(ImageInner::EmbeddedImage {
            cache_key: ImageCacheKey::Invalid,
            buffer: SharedImageBuffer::Gray8(buffer),
        })
    }

    /// Returns the pixel buffer for the Image if available in RGB format without alpha.
    /// Returns None if the pixels cannot be obtained, for example when the image was created from borrowed OpenGL textures.
    /// For an animated image, returns its first frame.
    pub fn to_rgb8(&self) -> Option<SharedPixelBuffer<Rgb8Pixel>> {
        self.0.render_to_buffer(None).and_then(|image| match image {
            SharedImageBuffer::RGB8(buffer) => Some(buffer),
            // Dropping the alpha channel would change what the image looks like, so these
            // are not converted. The opaque formats below are.
            SharedImageBuffer::RGBA8(_) | SharedImageBuffer::RGBA8Premultiplied(_) => None,
            #[cfg(feature = "image-pixel-format-rgb565")]
            SharedImageBuffer::RGB565(buffer) => Some(map_pixels(buffer)),
            #[cfg(feature = "image-pixel-format-gray8")]
            SharedImageBuffer::Gray8(buffer) => Some(map_pixels(buffer)),
        })
    }

    /// Returns the pixel buffer for the Image if available in RGBA format.
    /// Returns None if the pixels cannot be obtained, for example when the image was created from borrowed OpenGL textures.
    /// For an animated image, returns its first frame.
    pub fn to_rgba8(&self) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
        self.render_to_rgba8(None)
    }

    fn render_to_rgba8(
        &self,
        target_size: Option<euclid::Size2D<u32, PhysicalPx>>,
    ) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
        self.0.render_to_buffer(target_size).map(|image| match image {
            SharedImageBuffer::RGB8(buffer) => SharedPixelBuffer::<Rgba8Pixel> {
                width: buffer.width,
                height: buffer.height,
                data: buffer.data.into_iter().map(Into::into).collect(),
            },
            SharedImageBuffer::RGBA8(buffer) => buffer,
            SharedImageBuffer::RGBA8Premultiplied(buffer) => SharedPixelBuffer::<Rgba8Pixel> {
                width: buffer.width,
                height: buffer.height,
                data: buffer.data.into_iter().map(Image::premultiplied_rgba_to_rgba).collect(),
            },
            #[cfg(feature = "image-pixel-format-rgb565")]
            SharedImageBuffer::RGB565(buffer) => map_pixels(buffer),
            #[cfg(feature = "image-pixel-format-gray8")]
            SharedImageBuffer::Gray8(buffer) => map_pixels(buffer),
        })
    }

    /// Returns the pixel buffer for the Image if available in RGBA format, with the alpha channel pre-multiplied
    /// to the red, green, and blue channels.
    /// Returns None if the pixels cannot be obtained, for example when the image was created from borrowed OpenGL textures.
    /// For an animated image, returns its first frame.
    pub fn to_rgba8_premultiplied(&self) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
        self.0.render_to_buffer(None).map(|image| match image {
            SharedImageBuffer::RGB8(buffer) => SharedPixelBuffer::<Rgba8Pixel> {
                width: buffer.width,
                height: buffer.height,
                data: buffer.data.into_iter().map(Into::into).collect(),
            },
            SharedImageBuffer::RGBA8(buffer) => SharedPixelBuffer::<Rgba8Pixel> {
                width: buffer.width,
                height: buffer.height,
                data: buffer.data.into_iter().map(Image::rgba_to_premultiplied_rgba).collect(),
            },
            SharedImageBuffer::RGBA8Premultiplied(buffer) => buffer,
            #[cfg(feature = "image-pixel-format-rgb565")]
            SharedImageBuffer::RGB565(buffer) => map_pixels(buffer),
            #[cfg(feature = "image-pixel-format-gray8")]
            SharedImageBuffer::Gray8(buffer) => map_pixels(buffer),
        })
    }

    /// Returns the pixel converted from premultiplied RGBA to RGBA.
    fn premultiplied_rgba_to_rgba(pixel: Rgba8Pixel) -> Rgba8Pixel {
        if pixel.a == 0 {
            Rgba8Pixel::new(0, 0, 0, 0)
        } else {
            let af = pixel.a as u32;
            let round = (af / 2) as u32;
            Rgba8Pixel {
                r: ((pixel.r as u32 * 255 + round) / af).min(255) as u8,
                g: ((pixel.g as u32 * 255 + round) / af).min(255) as u8,
                b: ((pixel.b as u32 * 255 + round) / af).min(255) as u8,
                a: pixel.a,
            }
        }
    }

    /// Returns the pixel converted from RGBA to premultiplied RGBA.
    fn rgba_to_premultiplied_rgba(pixel: Rgba8Pixel) -> Rgba8Pixel {
        if pixel.a == 255 {
            pixel
        } else {
            let af = pixel.a as u32;
            Rgba8Pixel {
                r: (((pixel.r as u32 * af + 128) * 257) >> 16) as u8,
                g: (((pixel.g as u32 * af + 128) * 257) >> 16) as u8,
                b: (((pixel.b as u32 * af + 128) * 257) >> 16) as u8,
                a: pixel.a,
            }
        }
    }

    /// Returns the [WGPU](http://wgpu.rs) 29.x texture that this image wraps; returns None if the image does not
    /// hold such a previously wrapped texture.
    ///
    /// *Note*: This function is behind a feature flag and may be removed or changed in future minor releases,
    ///         as new major WGPU releases become available.
    #[cfg(feature = "unstable-wgpu-29")]
    pub fn to_wgpu_29_texture(&self) -> Option<wgpu_29::Texture> {
        match &self.0 {
            ImageInner::WGPUTexture(texture) => match &**texture {
                WGPUTexture::WGPU29Texture(texture) => Some(texture.clone()),
                #[allow(unreachable_patterns)]
                _ => None,
            },
            _ => None,
        }
    }

    /// Returns the [WGPU](http://wgpu.rs) 30.x texture that this image wraps; returns None if the image does not
    /// hold such a previously wrapped texture.
    ///
    /// *Note*: This function is behind a feature flag and may be removed or changed in future minor releases,
    ///         as new major WGPU releases become available.
    #[cfg(feature = "unstable-wgpu-30")]
    pub fn to_wgpu_30_texture(&self) -> Option<wgpu_30::Texture> {
        match &self.0 {
            ImageInner::WGPUTexture(texture) => match &**texture {
                WGPUTexture::WGPU30Texture(texture) => Some(texture.clone()),
                #[allow(unreachable_patterns)]
                _ => None,
            },
            _ => None,
        }
    }

    /// Creates a new Image from an existing OpenGL texture. The texture remains borrowed by Slint
    /// for the duration of being used for rendering, such as when assigned as source property to
    /// an `Image` element. It's the application's responsibility to delete the texture when it is
    /// not used anymore.
    ///
    /// The texture must be bindable against the `GL_TEXTURE_2D` target, have `GL_RGBA` as format
    /// for the pixel data.
    ///
    /// When Slint renders the texture, it assumes that the origin of the texture is at the top-left.
    /// This is different from the default OpenGL coordinate system.
    ///
    /// # Safety
    ///
    /// This function is unsafe because invalid texture ids may lead to undefined behavior in OpenGL
    /// drivers. A valid texture id is one that was created by the same OpenGL context that is
    /// current during any of the invocations of the callback set on [`Window::set_rendering_notifier()`](crate::api::Window::set_rendering_notifier).
    /// OpenGL contexts between instances of [`slint::Window`](crate::api::Window) are not sharing resources. Consequently
    /// [`slint::Image`](Self) objects created from borrowed OpenGL textures cannot be shared between
    /// different windows.
    #[allow(unsafe_code)]
    #[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
    #[deprecated(since = "1.2.0", note = "Use BorrowedOpenGLTextureBuilder")]
    pub unsafe fn from_borrowed_gl_2d_rgba_texture(
        texture_id: core::num::NonZeroU32,
        size: IntSize,
    ) -> Self {
        unsafe { BorrowedOpenGLTextureBuilder::new_gl_2d_rgba_texture(texture_id, size).build() }
    }

    /// Creates a new Image from the specified buffer, which contains SVG raw data.
    ///
    /// On the web, the browser renders the SVG, and compressed SVG data (svgz) is not supported.
    #[cfg(any(feature = "svg", all(target_arch = "wasm32", not(target_os = "emscripten"))))]
    pub fn load_from_svg_data(buffer: &[u8]) -> Result<Self, LoadImageError> {
        // On the web, the browser decodes the SVG.
        #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
        {
            htmlimage::HTMLImage::new_from_data(buffer, "image/svg+xml")
                .map(|html_image| Image(ImageInner::HTMLImage(vtable::VRc::new(html_image))))
                .ok_or(LoadImageError(()))
        }
        #[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
        {
            let cache_key = ImageCacheKey::Invalid;
            Ok(Image(ImageInner::Svg(vtable::VRc::new(
                svg::load_from_data(buffer, cache_key).map_err(|_| LoadImageError(()))?,
            ))))
        }
    }

    /// Creates a new Image from a buffer in memory holding the content of an encoded image file,
    /// such as a PNG, JPEG or SVG.
    ///
    /// `format` is the lowercase file extension of the encoded data (for example `"png"`, `"jpg"`
    /// or `"svg"`). Pass `None` to guess the format from the data; SVG is only recognized by this
    /// guess when the data begins with an `<?xml` or `<svg` tag, otherwise pass `Some("svg")`.
    ///
    /// The supported formats are the same as for [`Self::load_from_path`].
    #[cfg(any(
        feature = "image-decoders",
        all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std")
    ))]
    pub fn load_from_data(data: &[u8], format: Option<&str>) -> Result<Self, LoadImageError> {
        ImageInner::load_from_data_with_cache_key(
            ImageCacheKey::Invalid,
            ImageData::Borrowed(data),
            Slice::from_slice(format.unwrap_or_default().as_bytes()),
        )
        .map(Image)
        .ok_or(LoadImageError(()))
    }

    /// Sets the nine-slice edges of the image.
    ///
    /// [Nine-slice scaling](https://en.wikipedia.org/wiki/9-slice_scaling) is a method for scaling
    /// images in such a way that the corners are not distorted.
    /// The arguments define the pixel sizes of the edges that cut the image into 9 slices.
    pub fn set_nine_slice_edges(&mut self, top: u16, right: u16, bottom: u16, left: u16) {
        if top == 0 && left == 0 && right == 0 && bottom == 0 {
            if let ImageInner::NineSlice(n) = &self.0 {
                self.0 = n.0.clone();
            }
        } else {
            let array = [top, right, bottom, left];
            let inner = if let ImageInner::NineSlice(n) = &mut self.0 {
                n.0.clone()
            } else {
                self.0.clone()
            };
            self.0 = ImageInner::NineSlice(vtable::VRc::new(NineSliceImage(inner, array)));
        }
    }

    /// Returns the size of the Image in pixels.
    pub fn size(&self) -> IntSize {
        self.0.size()
    }

    #[cfg(feature = "std")]
    /// Returns the path of the image on disk, if it was constructed via [`Self::load_from_path`].
    ///
    /// For example:
    /// ```
    /// # use std::path::Path;
    /// # use i_slint_core::graphics::*;
    /// let path_buf = Path::new(env!("CARGO_MANIFEST_DIR"))
    ///     .join("../../demos/printerdemo/ui/images/cat.jpg");
    /// let image = Image::load_from_path(&path_buf).unwrap();
    /// assert_eq!(image.path(), Some(path_buf.as_path()));
    /// ```
    pub fn path(&self) -> Option<&std::path::Path> {
        self.0.path().map(|path| std::path::Path::new(path.as_str()))
    }
}

/// Like [`Image::to_rgba8`], but scalable sources (such as SVGs) are rasterized to
/// `target_size` (in physical pixels) instead of their intrinsic size.
/// Returns None if the pixels cannot be obtained.
pub fn image_to_rgba8_with_target_size(
    image: &Image,
    target_size: IntSize,
) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
    image.render_to_rgba8(Some(target_size.cast_unit()))
}

#[cfg(any(
    feature = "image-decoders",
    all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std")
))]
/// Load an image from the decoded payload of a data URI.
/// This is called by the interpreter.
pub fn load_image_from_data_uri(
    uri: &str,
    bytes: &[u8],
    format: &str,
) -> Result<Image, LoadImageError> {
    // The browser loads images asynchronously, so every evaluation of the same data URI
    // must share one image and its loading state: cache under the URI.
    #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
    {
        self::cache::IMAGE_CACHE.with(|global_cache| {
            global_cache
                .borrow_mut()
                .load_image_from_data_uri(uri, bytes, format)
                .ok_or(LoadImageError(()))
        })
    }
    // Native decoding is synchronous; don't fill the cache with one-shot images.
    #[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
    {
        let _ = uri;
        ImageInner::load_from_data_with_cache_key(
            ImageCacheKey::Invalid,
            ImageData::Borrowed(bytes),
            format.as_bytes().into(),
        )
        .map(Image)
        .ok_or(Default::default())
    }
}

/// Returns the MIME type for an image file extension, or None if the extension is unknown.
/// The extension is matched ASCII case-insensitively and given without the leading dot.
pub fn image_mime_type_from_extension(extension: &str) -> Option<&'static str> {
    for (ext, mime) in [
        ("png", "image/png"),
        ("jpg", "image/jpeg"),
        ("jpeg", "image/jpeg"),
        ("svg", "image/svg+xml"),
        ("svgz", "image/svg+xml"),
        ("gif", "image/gif"),
        ("webp", "image/webp"),
        ("bmp", "image/bmp"),
        ("ico", "image/x-icon"),
        ("avif", "image/avif"),
    ] {
        if extension.eq_ignore_ascii_case(ext) {
            return Some(mime);
        }
    }
    None
}

/// This enum describes the origin to use when rendering a borrowed OpenGL texture.
/// Use this with [`BorrowedOpenGLTextureBuilder::origin`].
#[derive(Copy, Clone, Debug, PartialEq, Default)]
#[repr(u8)]
#[non_exhaustive]
pub enum BorrowedOpenGLTextureOrigin {
    /// The top-left of the texture is the top-left of the texture drawn on the screen.
    #[default]
    TopLeft,
    /// The bottom-left of the texture is the top-left of the texture draw on the screen,
    /// flipping it vertically.
    BottomLeft,
}

/// Factory to create [`slint::Image`](crate::graphics::Image) from an existing OpenGL texture.
///
/// Methods can be chained on it in order to configure it.
///
///  * `origin`: Change the texture's origin when rendering (default: TopLeft).
///
/// Complete the builder by calling [`Self::build()`] to create a [`slint::Image`](crate::graphics::Image):
///
/// ```
/// # use i_slint_core::graphics::{BorrowedOpenGLTextureBuilder, Image, IntSize, BorrowedOpenGLTextureOrigin};
/// # let texture_id = core::num::NonZeroU32::new(1).unwrap();
/// # let size = IntSize::new(100, 100);
/// let builder = unsafe { BorrowedOpenGLTextureBuilder::new_gl_2d_rgba_texture(texture_id, size) }
///              .origin(BorrowedOpenGLTextureOrigin::TopLeft);
///
/// let image: slint::Image = builder.build();
/// ```
#[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
pub struct BorrowedOpenGLTextureBuilder(BorrowedOpenGLTexture);

#[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
impl BorrowedOpenGLTextureBuilder {
    /// Generates the base configuration for a borrowed OpenGL texture.
    ///
    /// The texture must be bindable against the `GL_TEXTURE_2D` target, have `GL_RGBA` as format
    /// for the pixel data.
    ///
    /// By default, when Slint renders the texture, it assumes that the origin of the texture is at the top-left.
    /// This is different from the default OpenGL coordinate system. Use the `mirror_vertically` function
    /// to reconfigure this.
    ///
    /// # Safety
    ///
    /// This function is unsafe because invalid texture ids may lead to undefined behavior in OpenGL
    /// drivers. A valid texture id is one that was created by the same OpenGL context that is
    /// current during any of the invocations of the callback set on [`Window::set_rendering_notifier()`](crate::api::Window::set_rendering_notifier).
    /// OpenGL contexts between instances of [`slint::Window`](crate::api::Window) are not sharing resources. Consequently
    /// [`slint::Image`](Image) objects created from borrowed OpenGL textures cannot be shared between
    /// different windows.
    #[allow(unsafe_code)]
    pub unsafe fn new_gl_2d_rgba_texture(texture_id: core::num::NonZeroU32, size: IntSize) -> Self {
        Self(BorrowedOpenGLTexture { texture_id, size, origin: Default::default() })
    }

    /// Configures the texture to be rendered vertically mirrored.
    pub fn origin(mut self, origin: BorrowedOpenGLTextureOrigin) -> Self {
        self.0.origin = origin;
        self
    }

    /// Completes the process of building a slint::Image that holds a borrowed OpenGL texture.
    pub fn build(self) -> Image {
        Image(ImageInner::BorrowedOpenGLTexture(self.0))
    }
}

/// Load an image by handing a URL to an HTML `<img>` element for the browser
/// to fetch. This is a web-only mechanism used by slintpad to display image
/// references that are URLs rather than file-system paths; it is not general
/// network image loading.
/// This is called by the interpreter and the generated code.
#[cfg(all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std"))]
pub fn load_as_html_image(url: &str) -> Result<Image, LoadImageError> {
    self::cache::IMAGE_CACHE.with(|global_cache| {
        global_cache.borrow_mut().load_as_html_image(url).ok_or(LoadImageError(()))
    })
}

/// Load an image from an image embedded in the binary.
/// This is called by the generated code.
#[cfg(any(
    feature = "image-decoders",
    all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std")
))]
pub fn load_image_from_embedded_data(data: Slice<'static, u8>, format: Slice<'_, u8>) -> Image {
    self::cache::IMAGE_CACHE.with(|global_cache| {
        global_cache.borrow_mut().load_image_from_embedded_data(data, format).unwrap_or_default()
    })
}

#[test]
fn test_empty_images_are_equal() {
    // An empty image compared unequal to itself, so a model row holding one never compared equal.
    assert_eq!(Image::default(), Image::default());
    let buffer = SharedPixelBuffer::<Rgb8Pixel>::new(1, 1);
    assert_ne!(Image::default(), Image::from_rgb8(buffer));
}

#[test]
fn test_image_size_from_buffer_without_backend() {
    {
        assert_eq!(Image::default().size(), Default::default());
        assert!(Image::default().to_rgb8().is_none());
        assert!(Image::default().to_rgba8().is_none());
        assert!(Image::default().to_rgba8_premultiplied().is_none());
    }
    {
        let buffer = SharedPixelBuffer::<Rgb8Pixel>::new(320, 200);
        let image = Image::from_rgb8(buffer.clone());
        assert_eq!(image.size(), [320, 200].into());
        assert_eq!(image.to_rgb8().as_ref().map(|b| b.as_slice()), Some(buffer.as_slice()));
    }
}

#[cfg(feature = "svg")]
#[test]
// memchr's manually aligned SIMD loads are a false positive under -Zmiri-symbolic-alignment-check
#[cfg_attr(miri, ignore)]
fn test_image_size_from_svg() {
    let simple_svg = r#"<svg width="320" height="200" xmlns="http://www.w3.org/2000/svg"></svg>"#;
    let image = Image::load_from_svg_data(simple_svg.as_bytes()).unwrap();
    assert_eq!(image.size(), [320, 200].into());
    assert_eq!(image.to_rgba8().unwrap().size(), image.size());
}

#[cfg(all(feature = "svg", feature = "image-decoders"))]
#[test]
#[cfg_attr(miri, ignore)]
fn test_image_path_of_svg() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../demos/printerdemo/ui/images/back.svg");
    let image = Image::load_from_path(&path).unwrap();
    assert_eq!(image.path(), Some(path.as_path()));
}

#[cfg(feature = "svg")]
#[test]
// memchr's manually aligned SIMD loads are a false positive under -Zmiri-symbolic-alignment-check
#[cfg_attr(miri, ignore)]
fn test_image_invalid_svg() {
    let invalid_svg = r#"AaBbCcDd"#;
    let result = Image::load_from_svg_data(invalid_svg.as_bytes());
    assert!(result.is_err());
}

#[cfg(feature = "svg")]
#[test]
// memchr's manually aligned SIMD loads are a false positive under -Zmiri-symbolic-alignment-check
#[cfg_attr(miri, ignore)]
fn test_image_load_from_data_svg() {
    let simple_svg = r#"<svg width="320" height="200" xmlns="http://www.w3.org/2000/svg"></svg>"#;
    // The leading `<svg` tag lets the format be guessed.
    let guessed = Image::load_from_data(simple_svg.as_bytes(), None).unwrap();
    assert_eq!(guessed.size(), [320, 200].into());
    // An explicit format hint works too.
    let hinted = Image::load_from_data(simple_svg.as_bytes(), Some("svg")).unwrap();
    assert_eq!(hinted.size(), [320, 200].into());
}

#[cfg(feature = "svg")]
#[test]
// memchr's manually aligned SIMD loads are a false positive under -Zmiri-symbolic-alignment-check
#[cfg_attr(miri, ignore)]
fn test_image_load_from_data_svgz() {
    // usvg gates gzip decompression behind its `svgz` feature, so a missing feature would
    // silently turn every compressed SVG into a load error. Gzip of the `simple_svg` above.
    const SIMPLE_SVGZ: &[u8] = &[
        0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x03, 0xb3, 0x29, 0x2e, 0x4b, 0x57,
        0x28, 0xcf, 0x4c, 0x29, 0xc9, 0xb0, 0x55, 0x32, 0x36, 0x32, 0x50, 0x52, 0xc8, 0x48, 0xcd,
        0x4c, 0xcf, 0x28, 0xb1, 0x55, 0x32, 0x32, 0x00, 0x72, 0x2a, 0x72, 0x73, 0xf2, 0x8a, 0x6d,
        0x95, 0x32, 0x4a, 0x4a, 0x0a, 0xac, 0xf4, 0xf5, 0xcb, 0xcb, 0xcb, 0xf5, 0xca, 0x8d, 0xf5,
        0xf2, 0x8b, 0xd2, 0xf5, 0x81, 0xb2, 0x06, 0xfa, 0x40, 0xad, 0x4a, 0x76, 0x36, 0x20, 0xca,
        0x0e, 0x00, 0x37, 0x91, 0x7a, 0xd6, 0x47, 0x00, 0x00, 0x00,
    ];
    let image = Image::load_from_data(SIMPLE_SVGZ, Some("svgz")).unwrap();
    assert_eq!(image.size(), [320, 200].into());
}

#[cfg(feature = "image-decoders")]
#[test]
#[cfg_attr(miri, ignore)]
fn test_image_load_from_data_png() {
    let mut png = std::io::Cursor::new(std::vec::Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(2, 3, image::Rgb([0, 255, 0])))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let png = png.into_inner();

    // The PNG magic bytes let the format be guessed.
    let guessed = Image::load_from_data(&png, None).unwrap();
    assert_eq!(guessed.size(), [2, 3].into());
    // An explicit format hint works too.
    let hinted = Image::load_from_data(&png, Some("png")).unwrap();
    assert_eq!(hinted.size(), [2, 3].into());

    assert!(Image::load_from_data(b"not an image", None).is_err());
}

/// The result of the fit function
#[derive(Debug)]
pub struct FitResult {
    /// The clip rect in the source image (in source image coordinate)
    pub clip_rect: IntRect,
    /// The scale to apply to go from the source to the target horizontally
    pub source_to_target_x: f32,
    /// The scale to apply to go from the source to the target vertically
    pub source_to_target_y: f32,
    /// The size of the target
    pub size: euclid::Size2D<f32, PhysicalPx>,
    /// The offset in the target in which we draw the image
    pub offset: euclid::Point2D<f32, PhysicalPx>,
    /// When Some, it means the image should be tiled instead of stretched to the target
    /// but still scaled with the source_to_target_x and source_to_target_y factor
    /// The point is the coordinate within the image's clip_rect of the pixel at the offset
    pub tiled: Option<euclid::default::Point2D<u32>>,
}

impl FitResult {
    fn adjust_for_tiling(
        self,
        ratio: f32,
        alignment: (ImageHorizontalAlignment, ImageVerticalAlignment),
        tiling: (ImageTiling, ImageTiling),
    ) -> Self {
        let mut r = self;
        let mut tiled = euclid::Point2D::default();
        let target = r.size;
        let o = r.clip_rect.size.cast::<f32>();
        match tiling.0 {
            ImageTiling::None => {
                r.size.width = o.width * r.source_to_target_x;
                if (o.width as f32) > target.width / r.source_to_target_x {
                    let diff = (o.width as f32 - target.width / r.source_to_target_x) as i32;
                    r.clip_rect.size.width -= diff;
                    r.clip_rect.origin.x += match alignment.0 {
                        ImageHorizontalAlignment::Center => diff / 2,
                        ImageHorizontalAlignment::Left => 0,
                        ImageHorizontalAlignment::Right => diff,
                    };
                    r.size.width = target.width;
                } else if (o.width as f32) < target.width / r.source_to_target_x {
                    r.offset.x += match alignment.0 {
                        ImageHorizontalAlignment::Center => {
                            (target.width - o.width as f32 * r.source_to_target_x) / 2.
                        }
                        ImageHorizontalAlignment::Left => 0.,
                        ImageHorizontalAlignment::Right => {
                            target.width - o.width as f32 * r.source_to_target_x
                        }
                    };
                }
            }
            ImageTiling::Repeat => {
                tiled.x = match alignment.0 {
                    ImageHorizontalAlignment::Left => 0,
                    ImageHorizontalAlignment::Center => {
                        ((o.width - target.width / ratio) / 2.).rem_euclid(o.width) as u32
                    }
                    ImageHorizontalAlignment::Right => {
                        (-target.width / ratio).rem_euclid(o.width) as u32
                    }
                };
                r.source_to_target_x = ratio;
            }
            ImageTiling::Round => {
                if target.width / ratio <= o.width * 1.5 {
                    r.source_to_target_x = target.width / o.width;
                } else {
                    let mut rem = (target.width / ratio).rem_euclid(o.width);
                    if rem > o.width / 2. {
                        rem -= o.width;
                    }
                    r.source_to_target_x = ratio * target.width / (target.width - rem * ratio);
                }
            }
        }

        match tiling.1 {
            ImageTiling::None => {
                r.size.height = o.height * r.source_to_target_y;
                if (o.height as f32) > target.height / r.source_to_target_y {
                    let diff = (o.height as f32 - target.height / r.source_to_target_y) as i32;
                    r.clip_rect.size.height -= diff;
                    r.clip_rect.origin.y += match alignment.1 {
                        ImageVerticalAlignment::Center => diff / 2,
                        ImageVerticalAlignment::Top => 0,
                        ImageVerticalAlignment::Bottom => diff,
                    };
                    r.size.height = target.height;
                } else if (o.height as f32) < target.height / r.source_to_target_y {
                    r.offset.y += match alignment.1 {
                        ImageVerticalAlignment::Center => {
                            (target.height - o.height as f32 * r.source_to_target_y) / 2.
                        }
                        ImageVerticalAlignment::Top => 0.,
                        ImageVerticalAlignment::Bottom => {
                            target.height - o.height as f32 * r.source_to_target_y
                        }
                    };
                }
            }
            ImageTiling::Repeat => {
                tiled.y = match alignment.1 {
                    ImageVerticalAlignment::Top => 0,
                    ImageVerticalAlignment::Center => {
                        ((o.height - target.height / ratio) / 2.).rem_euclid(o.height) as u32
                    }
                    ImageVerticalAlignment::Bottom => {
                        (-target.height / ratio).rem_euclid(o.height) as u32
                    }
                };
                r.source_to_target_y = ratio;
            }
            ImageTiling::Round => {
                if target.height / ratio <= o.height * 1.5 {
                    r.source_to_target_y = target.height / o.height;
                } else {
                    let mut rem = (target.height / ratio).rem_euclid(o.height);
                    if rem > o.height / 2. {
                        rem -= o.height;
                    }
                    r.source_to_target_y = ratio * target.height / (target.height - rem * ratio);
                }
            }
        }
        let has_tiling = tiling != (ImageTiling::None, ImageTiling::None);
        r.tiled = has_tiling.then_some(tiled);
        r
    }
}

#[cfg(not(feature = "std"))]
trait RemEuclid {
    fn rem_euclid(self, b: f32) -> f32;
}
#[cfg(not(feature = "std"))]
impl RemEuclid for f32 {
    fn rem_euclid(self, b: f32) -> f32 {
        num_traits::Euclid::rem_euclid(&self, &b)
    }
}

/// Return an FitResult that can be used to render an image in a buffer that matches a given ImageFit
pub fn fit(
    image_fit: ImageFit,
    target: euclid::Size2D<f32, PhysicalPx>,
    source_rect: IntRect,
    scale_factor: ScaleFactor,
    alignment: (ImageHorizontalAlignment, ImageVerticalAlignment),
    tiling: (ImageTiling, ImageTiling),
) -> FitResult {
    let has_tiling = tiling != (ImageTiling::None, ImageTiling::None);
    let o = source_rect.size.cast::<f32>();
    let ratio = match image_fit {
        // If there is any tiling, we ignore image_fit
        _ if has_tiling => scale_factor.get(),
        ImageFit::Fill => {
            return FitResult {
                clip_rect: source_rect,
                source_to_target_x: target.width / o.width,
                source_to_target_y: target.height / o.height,
                size: target,
                offset: Default::default(),
                tiled: None,
            };
        }
        ImageFit::Preserve => scale_factor.get(),
        ImageFit::Contain => f32::min(target.width / o.width, target.height / o.height),
        ImageFit::Cover => f32::max(target.width / o.width, target.height / o.height),
    };

    FitResult {
        clip_rect: source_rect,
        source_to_target_x: ratio,
        source_to_target_y: ratio,
        size: target,
        offset: euclid::Point2D::default(),
        tiled: None,
    }
    .adjust_for_tiling(ratio, alignment, tiling)
}

/// The pixel size a scalable image (an SVG) needs to be rasterized at to fill `target`
/// under `image_fit`, i.e. its intrinsic size scaled by what [`fit`] would scale it by.
///
/// Returns `None` when the source has no area to scale from, or when the fitted size
/// collapses to nothing.
pub fn scalable_render_size(
    source_size: IntSize,
    image_fit: ImageFit,
    target: euclid::Size2D<f32, PhysicalPx>,
    scale_factor: ScaleFactor,
    tiling: (ImageTiling, ImageTiling),
) -> Option<euclid::Size2D<u32, PhysicalPx>> {
    let source = source_size.cast::<f32>();
    if source.is_empty() {
        return None;
    }
    let fit = fit(
        image_fit,
        target,
        IntRect::from_size(source_size.cast()),
        scale_factor,
        // Only the size is of interest here, so the alignment doesn't matter.
        Default::default(),
        tiling,
    );
    let size = euclid::size2(
        (source.width * fit.source_to_target_x) as u32,
        (source.height * fit.source_to_target_y) as u32,
    );
    (!size.is_empty()).then_some(size)
}

/// The pixel size to rasterize a scalable image (an SVG) at when a transform, such as `transform-scale`, magnifies it by `transform_scale`.
/// The other arguments are those of [`scalable_render_size`], which gives the size without that transform,
/// except that `target_size` is the element's logical size.
/// Scaling up the image rasterized at that size would make it look blocky.
///
/// The magnification is rounded up to a power of two,
/// so that animating the scale rarely requires rasterizing the image again.
/// The result stays within 2048 pixels on its longest side, a texture size that GPUs commonly support.
/// Use [`svg_needs_rasterizing_again`] to tell whether a cached rasterization is too small for it.
pub fn transformed_scalable_render_size(
    source_size: IntSize,
    image_fit: ImageFit,
    target_size: LogicalSize,
    scale_factor: ScaleFactor,
    tiling: (ImageTiling, ImageTiling),
    transform_scale: f32,
) -> Option<euclid::Size2D<u32, PhysicalPx>> {
    let render_size = scalable_render_size(
        source_size,
        image_fit,
        target_size.cast::<f32>() * scale_factor,
        scale_factor,
        tiling,
    )?;
    Some(magnified_render_size(render_size, transform_scale))
}

/// The factor by which a transform magnifies in the direction where it magnifies the most:
/// the largest singular value of its linear part `[a b; c d]`.
pub fn transform_max_scale(a: f32, b: f32, c: f32, d: f32) -> f32 {
    let sum_of_squares = a * a + b * b + c * c + d * d;
    let determinant = a * d - b * c;
    let discriminant = sum_of_squares * sum_of_squares - 4. * determinant * determinant;
    ((sum_of_squares + discriminant.max(0.).sqrt()) / 2.).sqrt()
}

/// Whether `image`, an SVG rasterized at `cached` pixels, is too small for a transform that
/// magnifies it by `transform_scale`; the other arguments are those of
/// [`transformed_scalable_render_size`]. Always `false` for an image that isn't scalable.
///
/// Rasterizing keeps the image's aspect ratio, so for the same magnification the rasterized image
/// can be smaller than the wanted size in one direction;
/// only one rasterized for a smaller magnification is smaller in both.
pub fn svg_needs_rasterizing_again(
    cached: IntSize,
    image: &Image,
    image_fit: ImageFit,
    target_size: LogicalSize,
    scale_factor: ScaleFactor,
    tiling: (ImageTiling, ImageTiling),
    transform_scale: f32,
) -> bool {
    if transform_scale <= 1. || !<&ImageInner>::from(image).is_svg() {
        return false;
    }
    transformed_scalable_render_size(
        image.size(),
        image_fit,
        target_size,
        scale_factor,
        tiling,
        transform_scale,
    )
    .is_some_and(|wanted| cached.width < wanted.width && cached.height < wanted.height)
}

fn magnified_render_size(
    render_size: euclid::Size2D<u32, PhysicalPx>,
    transform_scale: f32,
) -> euclid::Size2D<u32, PhysicalPx> {
    const MAX_MAGNIFIED_SIZE: f32 = 2048.;
    if transform_scale.is_nan() || transform_scale <= 1. {
        return render_size;
    }
    // Tolerate rounding errors in the transform, so that a scale of 2 doesn't become 4.
    let power_of_two = (transform_scale.log2() - 0.001).ceil().exp2();
    let longest_side = render_size.width.max(render_size.height) as f32;
    let scale = power_of_two.min(MAX_MAGNIFIED_SIZE / longest_side);
    if scale > 1. { (render_size.cast::<f32>() * scale).round().cast() } else { render_size }
}

/// Generate an iterator of  [`FitResult`] for each slice of a nine-slice border image
pub fn fit9slice(
    source_rect: IntSize,
    [t, r, b, l]: [u16; 4],
    target: euclid::Size2D<f32, PhysicalPx>,
    scale_factor: ScaleFactor,
    alignment: (ImageHorizontalAlignment, ImageVerticalAlignment),
    tiling: (ImageTiling, ImageTiling),
) -> impl Iterator<Item = FitResult> {
    let fit_to = |clip_rect: euclid::default::Rect<u16>, target: euclid::Rect<f32, PhysicalPx>| {
        (!clip_rect.is_empty() && !target.is_empty()).then(|| {
            FitResult {
                clip_rect: clip_rect.cast(),
                source_to_target_x: target.width() / clip_rect.width() as f32,
                source_to_target_y: target.height() / clip_rect.height() as f32,
                size: target.size,
                offset: target.origin,
                tiled: None,
            }
            .adjust_for_tiling(scale_factor.get(), alignment, tiling)
        })
    };
    use euclid::rect;
    let sf = |x| scale_factor.get() * x as f32;
    let source = source_rect.cast::<u16>();
    if t + b > source.height || l + r > source.width {
        [None, None, None, None, None, None, None, None, None]
    } else {
        [
            fit_to(rect(0, 0, l, t), rect(0., 0., sf(l), sf(t))),
            fit_to(
                rect(l, 0, source.width - l - r, t),
                rect(sf(l), 0., target.width - sf(l) - sf(r), sf(t)),
            ),
            fit_to(rect(source.width - r, 0, r, t), rect(target.width - sf(r), 0., sf(r), sf(t))),
            fit_to(
                rect(0, t, l, source.height - t - b),
                rect(0., sf(t), sf(l), target.height - sf(t) - sf(b)),
            ),
            fit_to(
                rect(l, t, source.width - l - r, source.height - t - b),
                rect(sf(l), sf(t), target.width - sf(l) - sf(r), target.height - sf(t) - sf(b)),
            ),
            fit_to(
                rect(source.width - r, t, r, source.height - t - b),
                rect(target.width - sf(r), sf(t), sf(r), target.height - sf(t) - sf(b)),
            ),
            fit_to(rect(0, source.height - b, l, b), rect(0., target.height - sf(b), sf(l), sf(b))),
            fit_to(
                rect(l, source.height - b, source.width - l - r, b),
                rect(sf(l), target.height - sf(b), target.width - sf(l) - sf(r), sf(b)),
            ),
            fit_to(
                rect(source.width - r, source.height - b, r, b),
                rect(target.width - sf(r), target.height - sf(b), sf(r), sf(b)),
            ),
        ]
    }
    .into_iter()
    .flatten()
}

#[cfg(feature = "ffi")]
pub(crate) mod ffi {
    #![allow(unsafe_code)]

    use super::*;
    use core::ffi::c_void;

    // Expand Rgb8Pixel so that cbindgen can see it. (is in fact rgb::RGB<u8>)
    /// Represents an RGB pixel.
    #[cfg(cbindgen)]
    #[repr(C)]
    struct Rgb8Pixel {
        /// red value (between 0 and 255)
        r: u8,
        /// green value (between 0 and 255)
        g: u8,
        /// blue value (between 0 and 255)
        b: u8,
    }

    // Expand Rgba8Pixel so that cbindgen can see it. (is in fact rgb::RGBA<u8>)
    /// Represents an RGBA pixel.
    #[cfg(cbindgen)]
    #[repr(C)]
    struct Rgba8Pixel {
        /// red value (between 0 and 255)
        r: u8,
        /// green value (between 0 and 255)
        g: u8,
        /// blue value (between 0 and 255)
        b: u8,
        /// alpha value (between 0 and 255)
        a: u8,
    }

    // Keep the cfg free of target_arch: cbindgen maps target_arch = wasm32 to a C macro and
    // would guard the declaration, but the C++ API is native only and relies on it being there.
    #[cfg(all(feature = "std", feature = "image-decoders"))]
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_image_load_from_path(path: &SharedString, image: *mut Image) {
        unsafe {
            core::ptr::write(
                image,
                Image::load_from_path(std::path::Path::new(path.as_str())).unwrap_or_default(),
            )
        }
    }

    #[cfg(all(feature = "std", feature = "image-decoders"))]
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_image_load_from_embedded_data(
        data: Slice<'static, u8>,
        format: Slice<'static, u8>,
        image: *mut Image,
    ) {
        unsafe { core::ptr::write(image, super::load_image_from_embedded_data(data, format)) };
    }

    /// Unlike `slint_image_load_from_embedded_data`, this does not go through the image cache,
    /// so `data` does not have to be `'static`.
    #[cfg(all(feature = "std", feature = "image-decoders"))]
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_image_load_from_data(
        data: Slice<'_, u8>,
        format: Slice<'_, u8>,
        image: *mut Image,
    ) {
        let format = core::str::from_utf8(format.as_slice()).ok();
        let loaded = super::Image::load_from_data(data.as_slice(), format).unwrap_or_default();
        unsafe { core::ptr::write(image, loaded) };
    }
    /// Result of an asset loader callback.
    #[allow(dead_code)]
    #[repr(u8)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ImageLoaderResult {
        /// The asset was not found; fall through to default resolution.
        NotFound = 0,
        /// The asset was found; its data is in the `out` parameter.
        Found = 1,
        /// An error occurred; the error message is in the `error_out` parameter.
        Error = 2,
    }

    struct ImageLoaderUserData {
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
        callback: extern "C" fn(
            user_data: *mut c_void,
            id: &SharedString,
            out: *mut SharedVector<u8>,
            error_out: *mut SharedString,
        ) -> ImageLoaderResult,
    }

    impl Drop for ImageLoaderUserData {
        fn drop(&mut self) {
            if let Some(x) = self.drop_user_data {
                x(self.user_data)
            }
        }
    }

    /// Set a custom asset loader callback for loading images from custom sources.
    /// The callback is invoked with an asset identifier and should return the asset data.
    /// Return values: `LoaderResult::NotFound` (try default resolution),
    /// `LoaderResult::Found` (data written to `out`),
    /// or `LoaderResult::Error` (error message written to `error_out`).
    /// The user_data pointer is passed to the callback and cleaned up by drop_user_data.
    #[cfg(all(
        feature = "image-decoders",
        any(not(target_arch = "wasm32"), target_os = "emscripten")
    ))]
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_set_image_loader(
        callback: extern "C" fn(
            user_data: *mut c_void,
            id: &SharedString,
            out: *mut SharedVector<u8>,
            error_out: *mut SharedString,
        ) -> ImageLoaderResult,
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    ) {
        let ud = ImageLoaderUserData { user_data, drop_user_data, callback };
        set_image_loader(move |id: &str| {
            let mut out_vec = SharedVector::default();
            let mut error_msg = SharedString::default();
            let result =
                (ud.callback)(ud.user_data, &SharedString::from(id), &mut out_vec, &mut error_msg);
            match result {
                ImageLoaderResult::NotFound => None,
                ImageLoaderResult::Found => Some(Ok(out_vec.to_vec())),
                ImageLoaderResult::Error => {
                    Some(Err(alloc::string::String::from(error_msg.as_str())))
                }
            }
        });
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_image_size(image: &Image) -> IntSize {
        image.size()
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_image_path(image: &Image) -> Option<&SharedString> {
        #[cfg(feature = "std")]
        return image.0.path();
        #[cfg(not(feature = "std"))]
        {
            let _ = image;
            None
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_image_from_embedded_textures(
        textures: &'static StaticTextures<StaticStorage>,
        image: *mut Image,
    ) {
        unsafe { core::ptr::write(image, Image::from(ImageInner::StaticTextures(textures))) };
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_image_compare_equal(image1: &Image, image2: &Image) -> bool {
        image1.eq(image2)
    }

    /// Call [`Image::set_nine_slice_edges`]
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_image_set_nine_slice_edges(
        image: &mut Image,
        top: u16,
        right: u16,
        bottom: u16,
        left: u16,
    ) {
        image.set_nine_slice_edges(top, right, bottom, left);
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_image_to_rgb8(
        image: &Image,
        data: &mut SharedVector<Rgb8Pixel>,
        width: &mut u32,
        height: &mut u32,
    ) -> bool {
        image.to_rgb8().is_some_and(|pixel_buffer| {
            *data = pixel_buffer.data.clone();
            *width = pixel_buffer.width();
            *height = pixel_buffer.height();
            true
        })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_image_to_rgba8(
        image: &Image,
        data: &mut SharedVector<Rgba8Pixel>,
        width: &mut u32,
        height: &mut u32,
    ) -> bool {
        image.to_rgba8().is_some_and(|pixel_buffer| {
            *data = pixel_buffer.data.clone();
            *width = pixel_buffer.width();
            *height = pixel_buffer.height();
            true
        })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_image_to_rgba8_premultiplied(
        image: &Image,
        data: &mut SharedVector<Rgba8Pixel>,
        width: &mut u32,
        height: &mut u32,
    ) -> bool {
        image.to_rgba8_premultiplied().is_some_and(|pixel_buffer| {
            *data = pixel_buffer.data.clone();
            *width = pixel_buffer.width();
            *height = pixel_buffer.height();
            true
        })
    }
}

/// This structure contains fields to identify and render an OpenGL texture that Slint borrows from the application code.
/// Use this to embed a native OpenGL texture into a Slint scene.
///
/// The ownership of the texture remains with the application. It is the application's responsibility to delete the texture
/// when it is not used anymore.
///
/// Note that only 2D RGBA textures are supported.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
#[repr(C)]
pub struct BorrowedOpenGLTexture {
    /// The id or name of the texture, as created by [`glGenTextures`](https://registry.khronos.org/OpenGL-Refpages/gl4/html/glGenTextures.xhtml).
    pub texture_id: core::num::NonZeroU32,
    /// The size of the texture in pixels.
    pub size: IntSize,
    /// Origin of the texture when rendering.
    pub origin: BorrowedOpenGLTextureOrigin,
}

#[cfg(test)]
mod tests {
    use crate::graphics::{Rgba8Pixel, SharedPixelBuffer};

    use super::{Image, Rgb565Pixel};

    #[test]
    #[should_panic(expected = "the requested 8x8 pixels")]
    fn clone_from_slice_rejects_a_short_slice() {
        // One byte per pixel read as four: what an 8 bit grayscale buffer handed to
        // `Image::from_rgba8` looks like (#13491).
        let gray = [0u8; 8 * 8];
        SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&gray, 8, 8);
    }

    #[test]
    #[should_panic(expected = "the requested 8x8 pixels")]
    fn clone_from_slice_rejects_a_long_slice() {
        // Four bytes per pixel read as three, the same mistake the other way around.
        let rgba = [0u8; 8 * 8 * 4];
        SharedPixelBuffer::<crate::graphics::Rgb8Pixel>::clone_from_slice(&rgba, 8, 8);
    }

    #[test]
    fn test_magnified_render_size() {
        use super::magnified_render_size;
        let size = |width, height| euclid::size2::<u32, crate::lengths::PhysicalPx>(width, height);
        // Without magnification, the image is rasterized at its usual size.
        assert_eq!(magnified_render_size(size(100, 50), 1.), size(100, 50));
        assert_eq!(magnified_render_size(size(100, 50), 0.5), size(100, 50));
        assert_eq!(magnified_render_size(size(100, 50), f32::NAN), size(100, 50));
        // The magnification is rounded up to a power of two, ignoring rounding errors.
        assert_eq!(magnified_render_size(size(100, 50), 2.), size(200, 100));
        assert_eq!(magnified_render_size(size(100, 50), 2.000_001), size(200, 100));
        assert_eq!(magnified_render_size(size(100, 50), 3.), size(400, 200));
        // It stops at 2048 pixels on the longest side...
        assert_eq!(magnified_render_size(size(1000, 500), 4.), size(2048, 1024));
        // ...but never shrinks an image that's already larger.
        assert_eq!(magnified_render_size(size(3000, 100), 4.), size(3000, 100));
    }

    #[test]
    fn test_premultiplied_to_rgb_zero_alpha() {
        let pixel = Rgba8Pixel::new(5, 10, 15, 0);
        let converted = Image::premultiplied_rgba_to_rgba(pixel);
        assert_eq!(converted, Rgba8Pixel::new(0, 0, 0, 0));
    }

    #[test]
    fn test_premultiplied_to_rgb_full_alpha() {
        let pixel = Rgba8Pixel::new(5, 10, 15, 255);
        let converted = Image::premultiplied_rgba_to_rgba(pixel);
        assert_eq!(converted, Rgba8Pixel::new(5, 10, 15, 255));
    }

    #[test]
    fn test_premultiplied_to_rgb() {
        let pixel = Rgba8Pixel::new(5, 10, 15, 128);
        let converted = Image::premultiplied_rgba_to_rgba(pixel);
        assert_eq!(converted, Rgba8Pixel::new(10, 20, 30, 128));
    }

    #[test]
    fn test_rgb_to_premultiplied_zero_alpha() {
        let pixel = Rgba8Pixel::new(10, 20, 30, 0);
        let converted = Image::rgba_to_premultiplied_rgba(pixel);
        assert_eq!(converted, Rgba8Pixel::new(0, 0, 0, 0));
    }

    #[test]
    fn test_rgb_to_premultiplied_full_alpha() {
        let pixel = Rgba8Pixel::new(10, 20, 30, 255);
        let converted = Image::rgba_to_premultiplied_rgba(pixel);
        assert_eq!(converted, Rgba8Pixel::new(10, 20, 30, 255));
    }

    #[test]
    fn test_rgb_to_premultiplied() {
        let pixel = Rgba8Pixel::new(10, 20, 30, 128);
        let converted = Image::rgba_to_premultiplied_rgba(pixel);
        assert_eq!(converted, Rgba8Pixel::new(5, 10, 15, 128));
    }

    #[cfg(feature = "image-pixel-format-rgb565")]
    #[test]
    fn test_image_from_rgb565() {
        let mut buffer = SharedPixelBuffer::<Rgb565Pixel>::new(2, 1);
        buffer.make_mut_slice()[0] = Rgb565Pixel::from_rgb(0xff, 0, 0);
        buffer.make_mut_slice()[1] = Rgb565Pixel(0b00000_111111_00000);
        let image = Image::from_rgb565(buffer);
        assert_eq!(image.size(), crate::graphics::IntSize::new(2, 1));

        // RGB565 is opaque, so to_rgb8 converts it.
        let rgb = image.to_rgb8().unwrap();
        assert_eq!(rgb.as_slice()[0], super::Rgb8Pixel::new(0xff, 0, 0));
        assert_eq!(rgb.as_slice()[1], super::Rgb8Pixel::new(0, 0xff, 0));

        // A full component scales back up to 0xff.
        let rgba = image.to_rgba8().unwrap();
        assert_eq!(rgba.as_slice()[0], Rgba8Pixel::new(0xff, 0, 0, 0xff));
        assert_eq!(rgba.as_slice()[1], Rgba8Pixel::new(0, 0xff, 0, 0xff));

        // RGB565 has no alpha, so the premultiplied form is the same.
        let premultiplied = image.to_rgba8_premultiplied().unwrap();
        assert_eq!(premultiplied.as_slice(), rgba.as_slice());
    }

    #[cfg(feature = "image-pixel-format-gray8")]
    #[test]
    fn test_image_from_gray8() {
        let mut buffer = SharedPixelBuffer::<super::Gray8Pixel>::new(2, 1);
        buffer.make_mut_slice()[0] = super::Gray8Pixel(0x00);
        buffer.make_mut_slice()[1] = super::Gray8Pixel(0xff);
        let image = Image::from_gray8(buffer);
        assert_eq!(image.size(), crate::graphics::IntSize::new(2, 1));

        // Gray8 is opaque, so to_rgb8 converts it, writing the luminance to all three channels.
        let rgb = image.to_rgb8().unwrap();
        assert_eq!(rgb.as_slice()[0], super::Rgb8Pixel::new(0, 0, 0));
        assert_eq!(rgb.as_slice()[1], super::Rgb8Pixel::new(0xff, 0xff, 0xff));

        let rgba = image.to_rgba8().unwrap();
        assert_eq!(rgba.as_slice()[0], Rgba8Pixel::new(0, 0, 0, 0xff));
        assert_eq!(rgba.as_slice()[1], Rgba8Pixel::new(0xff, 0xff, 0xff, 0xff));

        // Gray8 has no alpha, so the premultiplied form is the same.
        let premultiplied = image.to_rgba8_premultiplied().unwrap();
        assert_eq!(premultiplied.as_slice(), rgba.as_slice());
    }

    #[cfg(feature = "image-pixel-format-gray8")]
    #[test]
    fn test_gray8_from_luminance_bytes() {
        let buffer =
            SharedPixelBuffer::<super::Gray8Pixel>::clone_from_slice(&[0x10u8, 0x80], 2, 1);
        assert_eq!(buffer.as_slice(), &[super::Gray8Pixel(0x10), super::Gray8Pixel(0x80)]);
    }

    #[test]
    fn test_rgb565_pixel_conversions() {
        for &(r, g, b) in &[(0xff, 0x25, 0u8), (0x56, 0x42, 0xe3), (0, 0, 0), (255, 255, 255)] {
            let pix565 = Rgb565Pixel::from_rgb(r, g, b);
            let pix888: super::Rgb8Pixel = pix565.into();
            // 565 -> 888 -> 565 is lossless.
            assert_eq!(pix565, pix888.into(), "mismatch for ({r}, {g}, {b})");
        }
    }
}
