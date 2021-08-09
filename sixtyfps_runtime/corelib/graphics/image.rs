/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use crate::slice::Slice;
use crate::{SharedString, SharedVector};

/// SharedPixelBuffer is a container for storing image data as pixels, backed by
/// [`SharedVector`]. That means it is internally reference counted and cheap
/// to clone.
///
/// You can construct new a new empty shared pixel buffer with [`SharedPixelBuffer::new`],
/// or you can clone it from an existing contiguous buffer that you might already have, using
/// [`SharedPixelBuffer::clone_from_slice`].
///
/// See the documentation for [`SharedImageBuffer`] for examples how to use this type to integrate
/// SixtyFPS with external rendering functions.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct SharedPixelBuffer<Pixel> {
    width: usize,
    height: usize,
    stride: usize,
    data: SharedVector<Pixel>,
}

impl<Pixel> SharedPixelBuffer<Pixel> {
    /// Returns the width of the image in pixels.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Returns the height of the image in pixels.
    pub fn height(&self) -> usize {
        self.height
    }

    /// Returns the number of pixels per line.
    pub fn stride(&self) -> usize {
        self.stride
    }
}

impl<Pixel: Clone> SharedPixelBuffer<Pixel> {
    /// Return a mutable slice to the pixel data. If the SharedPixelBuffer was shared, this will make a copy of the buffer.
    pub fn as_mut_slice(&mut self) -> &mut [Pixel] {
        self.data.as_mut_slice()
    }
}

impl<Pixel> SharedPixelBuffer<Pixel> {
    /// Return a slice to the pixel data.
    pub fn as_slice(&mut self) -> &[Pixel] {
        self.data.as_slice()
    }
}

impl<Pixel> imgref::ImgExt<Pixel> for SharedPixelBuffer<Pixel> {
    fn width_padded(&self) -> usize {
        self.stride
    }

    fn height_padded(&self) -> usize {
        self.data.len() / self.stride
    }

    fn rows_padded(&self) -> std::slice::Chunks<'_, Pixel> {
        self.data.as_ref().chunks(self.stride)
    }

    fn as_ref(&self) -> imgref::ImgRef<Pixel> {
        imgref::Img::new(self.data.as_ref(), self.width, self.height)
    }
}

impl<Pixel: Clone> imgref::ImgExtMut<Pixel> for SharedPixelBuffer<Pixel> {
    fn rows_padded_mut(&mut self) -> std::slice::ChunksMut<'_, Pixel> {
        self.data.as_mut_slice().chunks_mut(self.stride)
    }

    fn as_mut(&mut self) -> imgref::ImgRefMut<Pixel> {
        imgref::Img::new(self.data.as_mut_slice(), self.width, self.height)
    }
}

impl<Pixel: Clone + Default> SharedPixelBuffer<Pixel> {
    /// Creates a new SharedPixelBuffer with the given width and height. Each pixel will be initialized with the value
    /// that `[Default::default()`] returns for the Pixel type.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            stride: width,
            data: std::iter::repeat(Pixel::default()).take(width * height).collect(),
        }
    }
}

impl<Pixel: Clone> SharedPixelBuffer<Pixel> {
    /// Creates a new SharedPixelBuffer by cloning and converting pixels from an existing
    /// slice. This function is useful when another crate was used to allocate an image
    /// and you would like to convert it for use in SixtyFPS.
    pub fn clone_from_slice<SourcePixelType>(
        pixel_slice: &[SourcePixelType],
        width: usize,
        height: usize,
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

/// SharedImageBuffer is a container for images that are stored in CPU accessible memory.
///
/// The SharedImageBuffer's variants represent the different common formats for encoding
/// images in pixels.
///
/// A typical use-case is to have an external crate for rendering some content as an image.
/// For this it's most efficient to create a new SharedPixelBuffer with the known dimensions
/// and pass the the mutable slice to your rendering function. Afterwards you can create a
/// SharedImageBuffer variant that tells SixtyFPS about the format of the image.
///
/// The following example creates a 320x200 RGB pixel buffer and calls an external
/// low_level_render() function to draw a shape into it. Finally the result is
/// stored in a SharedImageBuffer, which in turn can be converted into an `[Image]`
/// via `[std::convert::Into`]:
/// ```
/// # use sixtyfps_corelib::graphics::{SharedPixelBuffer, SharedImageBuffer, Image};
/// use rgb::ComponentBytes; // Allow converting the RGB8 struct to u8 slices.
///
/// fn low_level_render(width: usize, height: usize, buffer: &mut [u8]) {
///     // render beautiful circle or other shapes here
/// }
///
/// let mut pixel_buffer = SharedPixelBuffer::<rgb::RGB8>::new(320, 200);
///
/// low_level_render(pixel_buffer.width(), pixel_buffer.height(),
///                  pixel_buffer.as_mut_slice().as_bytes_mut());
///
/// let image: Image = SharedImageBuffer::RGB8(pixel_buffer).into();
/// ```
///
/// Another use-case is to import existing image data into SixtyFPS, by
/// creating a new SharedImageBuffer through cloning of another image
/// type.
///
/// The following example uses the popular [image crate](https://docs.rs/image/) to
/// load a `.png` file from disk, apply brightening filter on it and then import
/// it into an `[Image]`:
/// ```no_run
/// # use sixtyfps_corelib::graphics::{SharedPixelBuffer, SharedImageBuffer, Image};
/// let mut cat_image = image::open("cat.png").expect("Error loading cat image").into_rgba8();
///
/// image::imageops::colorops::brighten_in_place(&mut cat_image, 20);
///
/// let buffer = SharedImageBuffer::RGBA8(SharedPixelBuffer::clone_from_slice(
///     cat_image.as_raw(),
///     cat_image.width() as _,
///     cat_image.height() as _,
/// ));
/// let image: Image = buffer.into();
/// ```
///

#[derive(Clone, Debug)]
#[repr(C)]
pub enum SharedImageBuffer {
    /// This variant holds the data for an image where each pixel has three color channels (red, green,
    /// and blue) and each channel is encoded as unsigned byte.
    RGB8(SharedPixelBuffer<rgb::RGB8>),
    /// This variant holds the data for an image where each pixel has four color channels (red, green,
    /// blue and alpha) and each channel is encoded as unsigned byte.
    RGBA8(SharedPixelBuffer<rgb::RGBA8>),
    /// This variant holds the data for an image where each pixel has four color channels (red, green,
    /// blue and alpha) and each channel is encoded as unsigned byte. In contrast to [`Self::RGBA8`],
    /// this variant assumes that the alpha channel is also already multiplied to each red, green and blue
    /// component of each pixel.
    /// Only construct this format if you know that your pixels are encoded this way. It is more efficient
    /// for rendering.
    RGBA8Premultiplied(SharedPixelBuffer<rgb::RGBA8>),
}

impl SharedImageBuffer {
    /// Returns the width of the image in pixels.
    #[inline]
    pub fn width(&self) -> usize {
        match self {
            Self::RGB8(buffer) => buffer.width(),
            Self::RGBA8(buffer) => buffer.width(),
            Self::RGBA8Premultiplied(buffer) => buffer.width(),
        }
    }

    /// Returns the height of the image in pixels.
    #[inline]
    pub fn height(&self) -> usize {
        match self {
            Self::RGB8(buffer) => buffer.height(),
            Self::RGBA8(buffer) => buffer.height(),
            Self::RGBA8Premultiplied(buffer) => buffer.height(),
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

/// A resource is a reference to binary data, for example images. They can be accessible on the file
/// system or embedded in the resulting binary. Or they might be URLs to a web server and a downloaded
/// is necessary before they can be used.
#[derive(Clone, PartialEq, Debug)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum ImageInner {
    /// A resource that does not represent any data.
    None,
    /// A resource that points to a file in the file system
    AbsoluteFilePath(SharedString),
    /// A image file that is embedded in the program as is. The format is the extension
    EmbeddedData {
        data: Slice<'static, u8>,
        format: Slice<'static, u8>,
    },
    EmbeddedImage {
        buffer: SharedImageBuffer,
    },
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

/// An image type that can be displayed by the Image element
#[repr(transparent)]
#[derive(Default, Clone, Debug, PartialEq, derive_more::From)]
pub struct Image(ImageInner);

impl Image {
    /// Load an Image from a path to a file containing an image
    pub fn load_from_path(path: &std::path::Path) -> Result<Self, LoadImageError> {
        Ok(Image(ImageInner::AbsoluteFilePath(path.to_str().ok_or(LoadImageError(()))?.into())))
    }

    /// Returns the size of the Image in pixels.
    pub fn size(&self) -> crate::graphics::Size {
        match crate::backend::instance() {
            Some(backend) => backend.image_size(self),
            None => panic!("sixtyfps::Image::size() called too early (before a graphics backend was chosen). You need to create a component first."),
        }
    }
}

impl From<SharedImageBuffer> for Image {
    fn from(buffer: SharedImageBuffer) -> Self {
        Self(ImageInner::EmbeddedImage { buffer })
    }
}

#[cfg(feature = "ffi")]
pub(crate) mod ffi {
    #![allow(unsafe_code)]

    use super::super::Size;
    use super::*;

    /// Expand RGB8 so that cbindgen can see it. (is in fact rgb::RGB<u8>)
    #[cfg(cbindgen)]
    #[repr(C)]
    struct RGB8 {
        r: u8,
        g: u8,
        b: u8,
    }

    /// Expand RGBA8 so that cbindgen can see it. (is in fact rgb::RGBA<u8>)
    #[cfg(cbindgen)]
    #[repr(C)]
    struct RGBA8 {
        r: u8,
        g: u8,
        b: u8,
    }

    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_image_size(image: &Image) -> Size {
        image.size()
    }
}
