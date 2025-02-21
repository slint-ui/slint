// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;

pub use crate::graphics::{CompositionMode, TexturePixelFormat};

/// This structure describes the properties of a texture for blending with [`TargetPixelBuffer::draw_texture`].
#[allow(dead_code)]
#[non_exhaustive]
pub struct Texture<'a> {
    /// A reference to the pixel bytes of the texture. These bytes are in the format specified by `pixel_format`.
    pub bytes: &'a [u8],
    /// The pixel format of the texture.
    pub pixel_format: TexturePixelFormat,
    /// The number of pixels per horizontal line of the texture.
    pub pixel_stride: u16,
    /// The width of the texture in pixels.
    pub width: u16,
    /// The height of the texture in pixels.
    pub height: u16,
    /// The delta to apply to the source x coordinate between pixels when drawing the texture.
    /// This is used when scaling the texture. The delta is specified in 8:8 fixed point format.
    pub delta_x: u16,
    /// The delta to apply to the source y coordinate between pixels when drawing the texture.
    /// This is used when scaling the texture. The delta is specified in 8:8 fixed point format.
    pub delta_y: u16,
    /// The offset within the texture to start reading pixels from in the x direction. The
    /// offset is specified in 12:4 fixed point format.
    pub source_offset_x: u16,
    /// The offset within the texture to start reading pixels from in the y direction. The
    /// offset is specified in 12:4 fixed point format.
    pub source_offset_y: u16,
}

/// This trait represents access to a buffer of pixels the software renderer can render into, as well
/// as certain operations that the renderer will try to delegate to this trait. Implement these functions
/// to delegate rendering further to hardware-provided 2D acceleration units, such as DMA2D or PXP.
pub trait TargetPixelBuffer {
    /// The pixel type the buffer represents.
    type TargetPixel: TargetPixel;

    /// Returns a slice of pixels for the given line.
    fn line_slice(&mut self, line_numer: usize) -> &mut [Self::TargetPixel];

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

    /// Draw a texture into the buffer at the specified position with the given size and
    /// colorized if needed. Returns true if the operation was successful; false if it could not be
    /// implemented and instead the software renderer needs to draw the texture
    fn draw_texture(
        &mut self,
        _x: i16,
        _y: i16,
        _width: i16,
        _height: i16,
        _src_texture: Texture<'_>,
        _colorize: u32,
        _alpha: u8,
        _rotation: RenderingRotation,
        _composition_mode: CompositionMode,
    ) -> bool {
        false
    }
}
