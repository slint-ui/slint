// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
This module contains image decoding and caching related types for the run-time library.
*/

use crate::lengths::{PhysicalPx, ScaleFactor};
use crate::slice::Slice;
#[allow(unused)]
use crate::{SharedString, SharedVector};

use super::{IntRect, IntSize};
use crate::items::{ImageFit, ImageHorizontalAlignment, ImageTiling, ImageVerticalAlignment};

#[cfg(feature = "image-decoders")]
pub mod cache;
#[cfg(target_arch = "wasm32")]
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

#[cfg(target_arch = "wasm32")]
OpaqueImageVTable_static! {
    /// VTable for RC wrapped HtmlImage helper struct.
    pub static HTML_IMAGE_VT for htmlimage::HTMLImage
}

OpaqueImageVTable_static! {
    /// VTable for RC wrapped SVG helper struct.
    pub static NINE_SLICE_VT for NineSliceImage
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

impl<Pixel: Clone + rgb::Pod> SharedPixelBuffer<Pixel>
where
    [Pixel]: rgb::ComponentBytes<u8>,
{
    /// Returns the pixels interpreted as raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        use rgb::ComponentBytes;
        self.data.as_slice().as_bytes()
    }

    /// Returns the pixels interpreted as raw bytes.
    pub fn make_mut_bytes(&mut self) -> &mut [u8] {
        use rgb::ComponentBytes;
        self.data.make_mut_slice().as_bytes_mut()
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
            data: core::iter::repeat(Pixel::default())
                .take(width as usize * height as usize)
                .collect(),
        }
    }
}

impl<Pixel: Clone> SharedPixelBuffer<Pixel> {
    /// Creates a new SharedPixelBuffer by cloning and converting pixels from an existing
    /// slice. This function is useful when another crate was used to allocate an image
    /// and you would like to convert it for use in Slint.
    pub fn clone_from_slice<SourcePixelType>(
        pixel_slice: &[SourcePixelType],
        width: u32,
        height: u32,
    ) -> Self
    where
        [SourcePixelType]: rgb::AsPixels<Pixel>,
    {
        use rgb::AsPixels;
        Self { width, height, data: pixel_slice.as_pixels().into() }
    }
}

/// Convenience alias for a pixel with three color channels (red, green and blue), each
/// encoded as u8.
pub type Rgb8Pixel = rgb::RGB8;
/// Convenience alias for a pixel with four color channels (red, green, blue and alpha), each
/// encoded as u8.
pub type Rgba8Pixel = rgb::RGBA8;

/// SharedImageBuffer is a container for images that are stored in CPU accessible memory.
///
/// The SharedImageBuffer's variants represent the different common formats for encoding
/// images in pixels.
#[derive(Clone, Debug)]
#[repr(C)]
/// TODO: Make this non_exhaustive before making the type public!
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
}

impl SharedImageBuffer {
    /// Returns the width of the image in pixels.
    #[inline]
    pub fn width(&self) -> u32 {
        match self {
            Self::RGB8(buffer) => buffer.width(),
            Self::RGBA8(buffer) => buffer.width(),
            Self::RGBA8Premultiplied(buffer) => buffer.width(),
        }
    }

    /// Returns the height of the image in pixels.
    #[inline]
    pub fn height(&self) -> u32 {
        match self {
            Self::RGB8(buffer) => buffer.height(),
            Self::RGBA8(buffer) => buffer.height(),
            Self::RGBA8Premultiplied(buffer) => buffer.height(),
        }
    }

    /// Returns the size of the image in pixels.
    #[inline]
    pub fn size(&self) -> IntSize {
        match self {
            Self::RGB8(buffer) => buffer.size(),
            Self::RGBA8(buffer) => buffer.size(),
            Self::RGBA8Premultiplied(buffer) => buffer.size(),
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
        }
    }
}

#[repr(u8)]
#[derive(Clone, PartialEq, Debug, Copy)]
/// The pixel format used for textures.
pub enum TexturePixelFormat {
    /// red, green, blue. 24bits.
    Rgb,
    /// Red, green, blue, alpha. 32bits.
    Rgba,
    /// Red, green, blue, alpha. 32bits. The color are premultiplied by alpha
    RgbaPremultiplied,
    /// Alpha map. 8bits. Each pixel is an alpha value. The color is specified separately.
    AlphaMap,
    /// Distance field. 8bit interpreted as i8.
    /// The range is such that i8::MIN corresponds to 3 pixels outside of the shape,
    /// and i8::MAX corresponds to 3 pixels inside the shape.
    /// The array must be width * height +1 bytes long. (the extra bit is read but never used)
    SignedDistanceField,
}

impl TexturePixelFormat {
    /// The number of bytes in a pixel
    pub fn bpp(self) -> usize {
        match self {
            TexturePixelFormat::Rgb => 3,
            TexturePixelFormat::Rgba => 4,
            TexturePixelFormat::RgbaPremultiplied => 4,
            TexturePixelFormat::AlphaMap => 1,
            TexturePixelFormat::SignedDistanceField => 1,
        }
    }
}

#[repr(C)]
#[derive(Clone, PartialEq, Debug)]
/// Some raw pixel data which is typically stored in the binary
pub struct StaticTexture {
    /// The position and size of the texture within the image
    pub rect: IntRect,
    /// The pixel format of this texture
    pub format: TexturePixelFormat,
    /// The color, for the alpha map ones
    pub color: crate::Color,
    /// index in the data array
    pub index: usize,
}

/// A texture is stored in read-only memory and may be composed of sub-textures.
#[repr(C)]
#[derive(Clone, PartialEq, Debug)]
pub struct StaticTextures {
    /// The total size of the image (this might not be the size of the full image
    /// as some transparent part are not part of any texture)
    pub size: IntSize,
    /// The size of the image before the compiler applied any scaling
    pub original_size: IntSize,
    /// The pixel data referenced by the textures
    pub data: Slice<'static, u8>,
    /// The list of textures
    pub textures: Slice<'static, StaticTexture>,
}

/// A struct that provides a path as a string as well as the last modification
/// time of the file it points to.
#[derive(PartialEq, Eq, Debug, Hash, Clone)]
#[repr(C)]
#[cfg(feature = "std")]
pub struct CachedPath {
    path: SharedString,
    /// SystemTime since UNIX_EPOC as secs
    last_modified: u32,
}

#[cfg(feature = "std")]
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
    #[cfg(feature = "std")]
    /// The image is identified by its path on the file system and the last modification time stamp.
    Path(CachedPath) = 1,
    /// The image is identified by a URL.
    #[cfg(target_arch = "wasm32")]
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
            ImageInner::Svg(parsed_svg) => parsed_svg.cache_key(),
            #[cfg(target_arch = "wasm32")]
            ImageInner::HTMLImage(htmlimage) => Self::URL(htmlimage.source().into()),
            ImageInner::BackendStorage(x) => vtable::VRc::borrow(x).cache_key(),
            #[cfg(not(target_arch = "wasm32"))]
            ImageInner::BorrowedOpenGLTexture(..) => return None,
            ImageInner::NineSlice(nine) => vtable::VRc::borrow(nine).cache_key(),
            #[cfg(feature = "unstable-wgpu-24")]
            ImageInner::WGPUTexture(..) => return None,
        };
        if matches!(key, ImageCacheKey::Invalid) {
            None
        } else {
            Some(key)
        }
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
#[cfg(feature = "unstable-wgpu-24")]
#[derive(Clone, Debug)]
pub enum WGPUTexture {
    /// A texture for WGPU version 24.
    #[cfg(feature = "unstable-wgpu-24")]
    WGPU24Texture(wgpu_24::Texture),
}

#[cfg(feature = "unstable-wgpu-24")]
impl OpaqueImage for WGPUTexture {
    fn size(&self) -> IntSize {
        match self {
            Self::WGPU24Texture(texture) => {
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
    StaticTextures(&'static StaticTextures) = 3,
    #[cfg(target_arch = "wasm32")]
    HTMLImage(vtable::VRc<OpaqueImageVTable, htmlimage::HTMLImage>) = 4,
    BackendStorage(vtable::VRc<OpaqueImageVTable>) = 5,
    #[cfg(not(target_arch = "wasm32"))]
    BorrowedOpenGLTexture(BorrowedOpenGLTexture) = 6,
    NineSlice(vtable::VRc<OpaqueImageVTable, NineSliceImage>) = 7,
    #[cfg(feature = "unstable-wgpu-24")]
    WGPUTexture(WGPUTexture) = 8,
}

impl ImageInner {
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
                let mut buffer =
                    SharedPixelBuffer::<Rgba8Pixel>::new(ts.size.width, ts.size.height);
                let stride = buffer.width() as usize;
                let slice = buffer.make_mut_slice();
                for t in ts.textures.iter() {
                    let rect = t.rect.to_usize();
                    for y in 0..rect.height() {
                        let slice = &mut slice[(rect.min_y() + y) * stride..][rect.x_range()];
                        let source = &ts.data[t.index + y * rect.width() * t.format.bpp()..];
                        match t.format {
                            TexturePixelFormat::Rgb => {
                                let mut iter = source.chunks_exact(3).map(|p| Rgba8Pixel {
                                    r: p[0],
                                    g: p[1],
                                    b: p[2],
                                    a: 255,
                                });
                                slice.fill_with(|| iter.next().unwrap());
                            }
                            TexturePixelFormat::RgbaPremultiplied => {
                                let mut iter = source.chunks_exact(4).map(|p| Rgba8Pixel {
                                    r: p[0],
                                    g: p[1],
                                    b: p[2],
                                    a: p[3],
                                });
                                slice.fill_with(|| iter.next().unwrap());
                            }
                            TexturePixelFormat::Rgba => {
                                let mut iter = source.chunks_exact(4).map(|p| {
                                    let a = p[3];
                                    Rgba8Pixel {
                                        r: (p[0] as u16 * a as u16 / 255) as u8,
                                        g: (p[1] as u16 * a as u16 / 255) as u8,
                                        b: (p[2] as u16 * a as u16 / 255) as u8,
                                        a,
                                    }
                                });
                                slice.fill_with(|| iter.next().unwrap());
                            }
                            TexturePixelFormat::AlphaMap => {
                                let col = t.color.to_argb_u8();
                                let mut iter = source.iter().map(|p| {
                                    let a = *p as u32 * col.alpha as u32;
                                    Rgba8Pixel {
                                        r: (col.red as u32 * a / (255 * 255)) as u8,
                                        g: (col.green as u32 * a / (255 * 255)) as u8,
                                        b: (col.blue as u32 * a / (255 * 255)) as u8,
                                        a: (a / 255) as u8,
                                    }
                                });
                                slice.fill_with(|| iter.next().unwrap());
                            }
                            TexturePixelFormat::SignedDistanceField => {
                                todo!("converting from a signed distance field to an image")
                            }
                        };
                    }
                }
                Some(SharedImageBuffer::RGBA8Premultiplied(buffer))
            }
            ImageInner::NineSlice(nine) => nine.0.render_to_buffer(None),
            _ => None,
        }
    }

    /// Returns true if the image is an SVG (either backed by resvg or HTML image wrapper).
    pub fn is_svg(&self) -> bool {
        match self {
            #[cfg(feature = "svg")]
            Self::Svg(_) => true,
            #[cfg(target_arch = "wasm32")]
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
            #[cfg(feature = "svg")]
            ImageInner::Svg(svg) => svg.size(),
            #[cfg(target_arch = "wasm32")]
            ImageInner::HTMLImage(htmlimage) => htmlimage.size().unwrap_or_default(),
            ImageInner::BackendStorage(x) => vtable::VRc::borrow(x).size(),
            #[cfg(not(target_arch = "wasm32"))]
            ImageInner::BorrowedOpenGLTexture(BorrowedOpenGLTexture { size, .. }) => *size,
            ImageInner::NineSlice(nine) => nine.0.size(),
            #[cfg(feature = "unstable-wgpu-24")]
            ImageInner::WGPUTexture(texture) => texture.size(),
        }
    }
}

impl PartialEq for ImageInner {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::EmbeddedImage { cache_key: l_cache_key, buffer: l_buffer },
                Self::EmbeddedImage { cache_key: r_cache_key, buffer: r_buffer },
            ) => l_cache_key == r_cache_key && l_buffer == r_buffer,
            #[cfg(feature = "svg")]
            (Self::Svg(l0), Self::Svg(r0)) => vtable::VRc::ptr_eq(l0, r0),
            (Self::StaticTextures(l0), Self::StaticTextures(r0)) => l0 == r0,
            #[cfg(target_arch = "wasm32")]
            (Self::HTMLImage(l0), Self::HTMLImage(r0)) => vtable::VRc::ptr_eq(l0, r0),
            (Self::BackendStorage(l0), Self::BackendStorage(r0)) => vtable::VRc::ptr_eq(l0, r0),
            #[cfg(not(target_arch = "wasm32"))]
            (Self::BorrowedOpenGLTexture(l0), Self::BorrowedOpenGLTexture(r0)) => l0 == r0,
            (Self::NineSlice(l), Self::NineSlice(r)) => l.0 == r.0 && l.1 == r.1,
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
    #[cfg(feature = "image-decoders")]
    /// Load an Image from a path to a file containing an image.
    ///
    /// Supported formats are SVG, PNG and JPEG.
    /// Enable support for additional formats supported by the [`image` crate](https://crates.io/crates/image) (
    /// AVIF, BMP, DDS, Farbfeld, GIF, HDR, ICO, JPEG, EXR, PNG, PNM, QOI, TGA, TIFF, WebP)
    /// by enabling the `image-default-formats` cargo feature.
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

    /// Returns the pixel buffer for the Image if available in RGB format without alpha.
    /// Returns None if the pixels cannot be obtained, for example when the image was created from borrowed OpenGL textures.
    pub fn to_rgb8(&self) -> Option<SharedPixelBuffer<Rgb8Pixel>> {
        self.0.render_to_buffer(None).and_then(|image| match image {
            SharedImageBuffer::RGB8(buffer) => Some(buffer),
            _ => None,
        })
    }

    /// Returns the pixel buffer for the Image if available in RGBA format.
    /// Returns None if the pixels cannot be obtained, for example when the image was created from borrowed OpenGL textures.
    pub fn to_rgba8(&self) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
        self.0.render_to_buffer(None).map(|image| match image {
            SharedImageBuffer::RGB8(buffer) => SharedPixelBuffer::<Rgba8Pixel> {
                width: buffer.width,
                height: buffer.height,
                data: buffer.data.into_iter().map(Into::into).collect(),
            },
            SharedImageBuffer::RGBA8(buffer) => buffer,
            SharedImageBuffer::RGBA8Premultiplied(buffer) => SharedPixelBuffer::<Rgba8Pixel> {
                width: buffer.width,
                height: buffer.height,
                data: buffer
                    .data
                    .into_iter()
                    .map(|rgba_premul| {
                        if rgba_premul.a == 0 {
                            Rgba8Pixel::new(0, 0, 0, 0)
                        } else {
                            let af = rgba_premul.a as f32 / 255.0;
                            Rgba8Pixel {
                                r: (rgba_premul.r as f32 * 255. / af) as u8,
                                g: (rgba_premul.g as f32 * 255. / af) as u8,
                                b: (rgba_premul.b as f32 * 255. / af) as u8,
                                a: rgba_premul.a,
                            }
                        }
                    })
                    .collect(),
            },
        })
    }

    /// Returns the pixel buffer for the Image if available in RGBA format, with the alpha channel pre-multiplied
    /// to the red, green, and blue channels.
    /// Returns None if the pixels cannot be obtained, for example when the image was created from borrowed OpenGL textures.
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
                data: buffer
                    .data
                    .into_iter()
                    .map(|rgba| {
                        if rgba.a == 255 {
                            rgba
                        } else {
                            let af = rgba.a as f32 / 255.0;
                            Rgba8Pixel {
                                r: (rgba.r as f32 * af / 255.) as u8,
                                g: (rgba.g as f32 * af / 255.) as u8,
                                b: (rgba.b as f32 * af / 255.) as u8,
                                a: rgba.a,
                            }
                        }
                    })
                    .collect(),
            },
            SharedImageBuffer::RGBA8Premultiplied(buffer) => buffer,
        })
    }

    /// Returns the [WGPU](http://wgpu.rs) 24.x texture that this image wraps; returns None if the image does not
    /// hold such a previously wrapped texture.
    ///
    /// *Note*: This function is behind a feature flag and may be removed or changed in future minor releases,
    ///         as new major WGPU releases become available.
    #[cfg(feature = "unstable-wgpu-24")]
    pub fn to_wgpu_24_texture(&self) -> Option<wgpu_24::Texture> {
        match &self.0 {
            ImageInner::WGPUTexture(WGPUTexture::WGPU24Texture(texture)) => Some(texture.clone()),
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
    #[cfg(not(target_arch = "wasm32"))]
    #[deprecated(since = "1.2.0", note = "Use BorrowedOpenGLTextureBuilder")]
    pub unsafe fn from_borrowed_gl_2d_rgba_texture(
        texture_id: core::num::NonZeroU32,
        size: IntSize,
    ) -> Self {
        BorrowedOpenGLTextureBuilder::new_gl_2d_rgba_texture(texture_id, size).build()
    }

    /// Creates a new Image from the specified buffer, which contains SVG raw data.
    #[cfg(feature = "svg")]
    pub fn load_from_svg_data(buffer: &[u8]) -> Result<Self, LoadImageError> {
        let cache_key = ImageCacheKey::Invalid;
        Ok(Image(ImageInner::Svg(vtable::VRc::new(
            svg::load_from_data(buffer, cache_key).map_err(|_| LoadImageError(()))?,
        ))))
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
        match &self.0 {
            ImageInner::EmbeddedImage {
                cache_key: ImageCacheKey::Path(CachedPath { path, .. }),
                ..
            } => Some(std::path::Path::new(path.as_str())),
            ImageInner::NineSlice(nine) => match &nine.0 {
                ImageInner::EmbeddedImage {
                    cache_key: ImageCacheKey::Path(CachedPath { path, .. }),
                    ..
                } => Some(std::path::Path::new(path.as_str())),
                _ => None,
            },
            _ => None,
        }
    }
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
#[cfg(not(target_arch = "wasm32"))]
pub struct BorrowedOpenGLTextureBuilder(BorrowedOpenGLTexture);

#[cfg(not(target_arch = "wasm32"))]
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
    /// [`slint::Image`](Self) objects created from borrowed OpenGL textures cannot be shared between
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

/// Load an image from an image embedded in the binary.
/// This is called by the generated code.
#[cfg(feature = "image-decoders")]
pub fn load_image_from_embedded_data(data: Slice<'static, u8>, format: Slice<'_, u8>) -> Image {
    self::cache::IMAGE_CACHE.with(|global_cache| {
        global_cache.borrow_mut().load_image_from_embedded_data(data, format).unwrap_or_default()
    })
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
fn test_image_size_from_svg() {
    let simple_svg = r#"<svg width="320" height="200" xmlns="http://www.w3.org/2000/svg"></svg>"#;
    let image = Image::load_from_svg_data(simple_svg.as_bytes()).unwrap();
    assert_eq!(image.size(), [320, 200].into());
    assert_eq!(image.to_rgba8().unwrap().size(), image.size());
}

#[cfg(feature = "svg")]
#[test]
fn test_image_invalid_svg() {
    let invalid_svg = r#"AaBbCcDd"#;
    let result = Image::load_from_svg_data(invalid_svg.as_bytes());
    assert!(result.is_err());
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
        return num_traits::Euclid::rem_euclid(&self, &b);
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
            }
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

    #[cfg(feature = "image-decoders")]
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_image_load_from_path(path: &SharedString, image: *mut Image) {
        core::ptr::write(
            image,
            Image::load_from_path(std::path::Path::new(path.as_str())).unwrap_or(Image::default()),
        )
    }

    #[cfg(feature = "std")]
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_image_load_from_embedded_data(
        data: Slice<'static, u8>,
        format: Slice<'static, u8>,
        image: *mut Image,
    ) {
        core::ptr::write(image, super::load_image_from_embedded_data(data, format));
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_image_size(image: &Image) -> IntSize {
        image.size()
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_image_path(image: &Image) -> Option<&SharedString> {
        match &image.0 {
            ImageInner::EmbeddedImage { cache_key, .. } => match cache_key {
                #[cfg(feature = "std")]
                ImageCacheKey::Path(CachedPath { path, .. }) => Some(path),
                _ => None,
            },
            ImageInner::NineSlice(nine) => match &nine.0 {
                ImageInner::EmbeddedImage { cache_key, .. } => match cache_key {
                    #[cfg(feature = "std")]
                    ImageCacheKey::Path(CachedPath { path, .. }) => Some(path),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_image_from_embedded_textures(
        textures: &'static StaticTextures,
        image: *mut Image,
    ) {
        core::ptr::write(image, Image::from(ImageInner::StaticTextures(textures)));
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_image_compare_equal(image1: &Image, image2: &Image) -> bool {
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
#[cfg(not(target_arch = "wasm32"))]
#[repr(C)]
pub struct BorrowedOpenGLTexture {
    /// The id or name of the texture, as created by [`glGenTextures`](https://registry.khronos.org/OpenGL-Refpages/gl4/html/glGenTextures.xhtml).
    pub texture_id: core::num::NonZeroU32,
    /// The size of the texture in pixels.
    pub size: IntSize,
    /// Origin of the texture when rendering.
    pub origin: BorrowedOpenGLTextureOrigin,
}
