// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;

use crate::graphics::IntSize;
pub use crate::graphics::{CompositionMode, TexturePixelFormat};

/// The pixel data of a for the source of a [`Texture`].
#[derive(Clone)]
#[non_exhaustive]
pub struct TextureData<'a> {
    /// A reference to the pixel bytes of the texture. These bytes are in the format specified by `pixel_format`.
    pub data: &'a [u8],
    /// The pixel format of the texture.
    pub pixel_format: TexturePixelFormat,
    /// The number of bytes between two lines in the data
    pub byte_stride: usize,
    /// The width of the texture in pixels.
    pub width: u32,
    /// The height of the texture in pixels.
    pub height: u32,
}

impl<'a> TextureData<'a> {
    pub fn new(
        data: &'a [u8],
        pixel_format: TexturePixelFormat,
        byte_stride: usize,
        size: IntSize,
    ) -> Self {
        let (width, height) = (size.width, size.height);
        Self { data, pixel_format, byte_stride, width, height }
    }
}

pub(super) enum TextureDataContainer {
    Static(TextureData<'static>),
    Shared { buffer: super::scene::SharedBufferData, source_rect: PhysicalRect },
}

#[derive(Debug, Clone)]
pub struct TilingInfo {
    /// Offset, in destination pixel of the left border of the tile.
    pub offset_x: i32,
    /// Offset, in destination pixel, of the top border of the tile.
    pub offset_y: i32,

    /// Scale factor in the x direction, this is the same as source's width / destination's width of the tile
    pub scale_x: f32,
    /// Scale factor in the y direction, this is the same as source's height / destination's height of the tile
    pub scale_y: f32,

    /// Gap in destination pixel between two tiles on the horizontal source axis.
    pub gap_x: u32,
    /// Gap in destination pixel between two tiles on the vertical source axis.
    pub gap_y: u32,
}

/// This structure describes the properties of a texture for blending with [`TargetPixelBuffer::draw_texture`].
#[non_exhaustive]
pub struct DrawTextureArgs {
    pub(super) data: TextureDataContainer,

    /// When set, the source is to be considered as an alpha map (so for ARGB texture, the RGB component will be ignored).
    /// And the given color is to be blended using the alpha value of the texture.
    pub colorize: Option<Color>,

    /// A value between 0 and 255 that specifies the alpha value of the texture.
    /// If colorize is set, this value can be ignored as the alpha would be part of the `colorize` value.
    /// A value of 0 would mean that the texture is fully transparent (so nothing is drawn),
    /// and a value of 255 would mean fully opaque.
    pub alpha: u8,

    /// The x position in the destination buffer to draw the texture at
    pub dst_x: isize,
    /// The y position in the destination buffer to draw the texture at
    pub dst_y: isize,
    /// The width of the image in the destination. The image should be scaled to fit.
    pub dst_width: usize,
    /// The height of the image in the destination. The Image should be scaled to fit
    pub dst_height: usize,

    /// the rotation to apply to the texture
    pub rotation: RenderingRotation,

    /// If the texture is to be tiled, this contains the information about the tiling
    pub tiling: Option<TilingInfo>,
}

impl DrawTextureArgs {
    /// Returns the source image data for this texture
    pub fn source(&self) -> TextureData<'_> {
        match &self.data {
            TextureDataContainer::Static(data) => data.clone(),
            TextureDataContainer::Shared { buffer, source_rect } => {
                let stride = buffer.width();
                let core::ops::Range { start, end } = compute_range_in_buffer(&source_rect, stride);
                let size = source_rect.size.to_untyped().cast();

                match &buffer {
                    SharedBufferData::SharedImage(SharedImageBuffer::RGB8(b)) => TextureData::new(
                        &b.as_bytes()[start * 3..end * 3],
                        TexturePixelFormat::Rgb,
                        stride * 3,
                        size,
                    ),
                    SharedBufferData::SharedImage(SharedImageBuffer::RGBA8(b)) => TextureData::new(
                        &b.as_bytes()[start * 4..end * 4],
                        TexturePixelFormat::Rgba,
                        stride * 4,
                        size,
                    ),
                    SharedBufferData::SharedImage(SharedImageBuffer::RGBA8Premultiplied(b)) => {
                        TextureData::new(
                            &b.as_bytes()[start * 4..end * 4],
                            TexturePixelFormat::RgbaPremultiplied,
                            stride * 4,
                            size,
                        )
                    }
                    SharedBufferData::AlphaMap { data, .. } => TextureData::new(
                        &data[start..end],
                        TexturePixelFormat::AlphaMap,
                        stride,
                        size,
                    ),
                }
            }
        }
    }

    pub(super) fn source_size(&self) -> PhysicalSize {
        match &self.data {
            TextureDataContainer::Static(data) => {
                PhysicalSize::new(data.width as _, data.height as _)
            }
            TextureDataContainer::Shared { source_rect, .. } => source_rect.size,
        }
    }
}

/// This trait represents access to a buffer of pixels the software renderer can render into, as well
/// as certain operations that the renderer will try to delegate to this trait. Implement these functions
/// to delegate rendering further to hardware-provided 2D acceleration units, such as DMA2D or PXP.
pub trait TargetPixelBuffer {
    /// The pixel type the buffer represents.
    type TargetPixel: TargetPixel;

    /// The optimal type for a color that can be blended
    //type BlendableColor: Copy + From<Color> + From<PremultipliedRgbaColor>;

    /// Returns a slice of pixels for the given line.
    fn line_slice(&mut self, line_number: usize) -> &mut [Self::TargetPixel];

    /// Returns the number of lines the buffer has. This is typically the height in pixels.
    fn num_lines(&self) -> usize;

    /// Fills the buffer with a rectangle at the specified position with the given size and the
    /// provided color. Returns true if the operation was successful; false if it could not be
    /// implemented and instead the software renderer needs to draw the rectangle.
    fn fill_rectangle(
        &mut self,
        _x: i16,
        _y: i16,
        _width: i16,
        _height: i16,
        _color: PremultipliedRgbaColor,
        _composition_mode: CompositionMode,
    ) -> bool {
        false
    }

    /// Draw a texture into the buffer.
    /// The texture must be clipped to the given region.
    /// Returns true if the operation was successful; false if it could not be
    /// implemented and instead the software renderer needs to draw the texture
    fn draw_texture(&mut self, _: &DrawTextureArgs, _clip: &PhysicalRegion) -> bool {
        false
    }
}
