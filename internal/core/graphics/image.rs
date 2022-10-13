// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*!
This module contains image decoding and caching related types for the run-time library.
*/

use crate::lengths::PhysicalPx;
use crate::slice::Slice;
use crate::{SharedString, SharedVector};

use super::{IntRect, IntSize};

#[cfg(feature = "image-decoders")]
pub mod cache;
#[cfg(target_arch = "wasm32")]
mod htmlimage;
#[cfg(feature = "svg")]
mod svg;

#[allow(missing_docs)]
#[vtable::vtable]
#[repr(C)]
pub struct OpaqueImageVTable {
    drop_in_place: fn(VRefMut<OpaqueImageVTable>) -> Layout,
    dealloc: fn(&OpaqueImageVTable, ptr: *mut u8, layout: Layout),
    /// Returns the image size
    size: fn(VRef<OpaqueImageVTable>) -> IntSize,
    /// Returns a cache key
    cache_key: fn(VRef<OpaqueImageVTable>) -> ImageCacheKey,
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
    stride: u32,
    data: SharedVector<Pixel>,
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

    /// Returns the number of pixels per line.
    pub fn stride(&self) -> u32 {
        self.stride
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
            stride: width,
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
        Self {
            width,
            height,
            stride: width,
            data: pixel_slice.as_pixels().iter().cloned().collect(),
        }
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
/// The pixel format of a StaticTexture
pub enum PixelFormat {
    /// red, green, blue. 24bits.
    Rgb,
    /// Red, green, blue, alpha. 32bits.
    Rgba,
    /// Red, green, blue, alpha. 32bits. The color are premultiplied by alpha
    RgbaPremultiplied,
    /// Alpha map. 8bits. Each pixel is an alpha value. The color is specified separately.
    AlphaMap,
}

impl PixelFormat {
    /// The number of bytes in a pixel
    pub fn bpp(self) -> usize {
        match self {
            PixelFormat::Rgb => 3,
            PixelFormat::Rgba => 4,
            PixelFormat::RgbaPremultiplied => 4,
            PixelFormat::AlphaMap => 1,
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
    pub format: PixelFormat,
    /// The color, for the alpha map ones
    pub color: crate::Color,
    /// index in the data array
    pub index: usize,
}

#[repr(C)]
#[derive(Clone, PartialEq, Debug)]
/// A texture is stored in read-only memory and may be composed of sub-textures.

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

/// ImageCacheKey encapsulates the different ways of indexing images in the
/// cache of decoded images.
#[derive(PartialEq, Eq, Debug, Hash, Clone)]
#[repr(C)]
pub enum ImageCacheKey {
    /// This variant indicates that no image cache key can be created for the image.
    /// For example this is the case for programmatically created images.
    Invalid,
    /// The image is identified by its path on the file system.
    Path(SharedString),
    /// The image is identified by a URL.
    #[cfg(target_arch = "wasm32")]
    URL(SharedString),
    /// The image is identified by the static address of its encoded data.
    EmbeddedData(usize),
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

/// A resource is a reference to binary data, for example images. They can be accessible on the file
/// system or embedded in the resulting binary. Or they might be URLs to a web server and a downloaded
/// is necessary before they can be used.
/// cbindgen:prefix-with-name
#[derive(Clone, Debug)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum ImageInner {
    /// A resource that does not represent any data.
    None,
    EmbeddedImage {
        cache_key: ImageCacheKey,
        buffer: SharedImageBuffer,
    },
    #[cfg(feature = "svg")]
    Svg(vtable::VRc<OpaqueImageVTable, svg::ParsedSVG>),
    StaticTextures(&'static StaticTextures),
    #[cfg(target_arch = "wasm32")]
    HTMLImage(vtable::VRc<OpaqueImageVTable, htmlimage::HTMLImage>),
    BackendStorage(vtable::VRc<OpaqueImageVTable>),
}

impl ImageInner {
    /// Return or render the image into a buffer
    ///
    /// `target_size_for_scalable_source` is the size to use if the image is scalable.
    ///
    /// Returns None if the image can't be rendered in a buffer
    pub fn render_to_buffer(
        &self,
        _target_size_for_scalable_source: Option<euclid::Size2D<u32, PhysicalPx>>,
    ) -> Option<SharedImageBuffer> {
        match self {
            ImageInner::EmbeddedImage { buffer, .. } => Some(buffer.clone()),
            #[cfg(feature = "svg")]
            ImageInner::Svg(svg) => {
                match svg.render(_target_size_for_scalable_source.unwrap_or_default()) {
                    Ok(b) => Some(b),
                    Err(err) => {
                        eprintln!("Error rendering SVG: {}", err);
                        return None;
                    }
                }
            }
            ImageInner::StaticTextures(ts) => {
                let mut buffer =
                    SharedPixelBuffer::<Rgba8Pixel>::new(ts.size.width, ts.size.height);
                let stride = buffer.stride() as usize;
                let slice = buffer.make_mut_slice();
                for t in ts.textures.iter() {
                    let rect = t.rect.to_usize();
                    for y in 0..rect.height() {
                        let slice = &mut slice[(rect.min_y() + y) * stride..][rect.x_range()];
                        let source = &ts.data[t.index + y * rect.width() * t.format.bpp()..];
                        match t.format {
                            PixelFormat::Rgb => {
                                let mut iter = source.chunks_exact(3).map(|p| Rgba8Pixel {
                                    r: p[0],
                                    g: p[1],
                                    b: p[2],
                                    a: 255,
                                });
                                slice.fill_with(|| iter.next().unwrap());
                            }
                            PixelFormat::RgbaPremultiplied => {
                                let mut iter = source.chunks_exact(4).map(|p| Rgba8Pixel {
                                    r: p[0],
                                    g: p[1],
                                    b: p[2],
                                    a: p[3],
                                });
                                slice.fill_with(|| iter.next().unwrap());
                            }
                            PixelFormat::Rgba => {
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
                            PixelFormat::AlphaMap => {
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
                        };
                    }
                }
                Some(SharedImageBuffer::RGBA8Premultiplied(buffer))
            }
            _ => None,
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
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

impl Default for ImageInner {
    fn default() -> Self {
        ImageInner::None
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
#[repr(transparent)]
#[derive(Default, Clone, Debug, PartialEq, derive_more::From)]
pub struct Image(ImageInner);

impl Image {
    #[cfg(feature = "image-decoders")]
    /// Load an Image from a path to a file containing an image
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

    /// Returns the size of the Image in pixels.
    pub fn size(&self) -> IntSize {
        match &self.0 {
            ImageInner::None => Default::default(),
            ImageInner::EmbeddedImage { buffer, .. } => buffer.size(),
            ImageInner::StaticTextures(StaticTextures { original_size, .. }) => *original_size,
            #[cfg(feature = "svg")]
            ImageInner::Svg(svg) => svg.size(),
            #[cfg(target_arch = "wasm32")]
            ImageInner::HTMLImage(htmlimage) => htmlimage.size().unwrap_or_default(),
            ImageInner::BackendStorage(x) => vtable::VRc::borrow(x).size(),
        }
    }

    #[cfg(feature = "std")]
    /// Returns the path of the image on disk, if it was constructed via [`Self::load_from_path`].
    ///
    /// For example:
    /// ```
    /// # use std::path::Path;
    /// # use i_slint_core::graphics::*;
    /// let path_buf = Path::new(env!("CARGO_MANIFEST_DIR"))
    ///     .join("../../examples/printerdemo/ui/images/cat.jpg");
    /// let image = Image::load_from_path(&path_buf).unwrap();
    /// assert_eq!(image.path(), Some(path_buf.as_path()));
    /// ```
    pub fn path(&self) -> Option<&std::path::Path> {
        match &self.0 {
            ImageInner::EmbeddedImage { cache_key, .. } => match cache_key {
                ImageCacheKey::Path(path) => Some(std::path::Path::new(path.as_str())),
                _ => None,
            },
            _ => None,
        }
    }
}

/// Load an image from an image embedded in the binary.
/// This is called by the generated code.
#[cfg(feature = "image-decoders")]
pub fn load_image_from_embedded_data(
    data: Slice<'static, u8>,
    format: Slice<'static, u8>,
) -> Image {
    self::cache::IMAGE_CACHE.with(|global_cache| {
        global_cache.borrow_mut().load_image_from_embedded_data(data, format).unwrap_or_else(|| {
            panic!("internal error: embedded image data is not supported by run-time library",)
        })
    })
}

#[test]
fn test_image_size_from_buffer_without_backend() {
    {
        assert_eq!(Image::default().size(), Default::default());
    }
    {
        let buffer = SharedPixelBuffer::<Rgb8Pixel>::new(320, 200);
        let image = Image::from_rgb8(buffer);
        assert_eq!(image.size(), [320, 200].into())
    }
}

#[cfg(feature = "ffi")]
pub(crate) mod ffi {
    #![allow(unsafe_code)]

    use super::super::IntSize;
    use super::*;

    /// Expand Rgb8Pixel so that cbindgen can see it. (is in fact rgb::RGB<u8>)
    #[cfg(cbindgen)]
    #[repr(C)]
    struct Rgb8Pixel {
        r: u8,
        g: u8,
        b: u8,
    }

    /// Expand Rgba8Pixel so that cbindgen can see it. (is in fact rgb::RGBA<u8>)
    #[cfg(cbindgen)]
    #[repr(C)]
    struct Rgba8Pixel {
        r: u8,
        g: u8,
        b: u8,
    }

    #[no_mangle]
    pub unsafe extern "C" fn slint_image_load_from_path(path: &SharedString, image: *mut Image) {
        std::ptr::write(
            image,
            Image::load_from_path(std::path::Path::new(path.as_str())).unwrap_or(Image::default()),
        )
    }

    #[no_mangle]
    pub unsafe extern "C" fn slint_image_load_from_embedded_data(
        data: Slice<'static, u8>,
        format: Slice<'static, u8>,
        image: *mut Image,
    ) {
        std::ptr::write(image, super::load_image_from_embedded_data(data, format));
    }

    #[no_mangle]
    pub unsafe extern "C" fn slint_image_size(image: &Image) -> IntSize {
        image.size()
    }

    #[no_mangle]
    pub unsafe extern "C" fn slint_image_path(image: &Image) -> Option<&SharedString> {
        match &image.0 {
            ImageInner::EmbeddedImage { cache_key, .. } => match cache_key {
                ImageCacheKey::Path(path) => Some(path),
                _ => None,
            },
            _ => None,
        }
    }
}
