// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Drawing primitives invoked by generated Slint SC components.
//!
//! There are exactly two primitives:
//!
//!   * [`fill_rectangle`] — blit a solid color over a rectangular region.
//!   * [`draw_static_texture`] — blit an image embedded at compile time.
//!
//! Both primitives always draw into the whole [`TargetPixelBuffer`] — there
//! is no partial or line-by-line rendering.  Clipping to the buffer bounds is
//! performed by these functions: callers can always pass unclipped
//! coordinates.

use crate::api::{Color, PremultipliedRgbaColor, TargetPixel, TargetPixelBuffer};

/// The in-memory layout of a texture embedded by the compiler.
///
/// These values mirror a subset of
/// `i_slint_compiler::embedded_resources::PixelFormat`, but duplicated here
/// so that the runtime crate has zero external dependencies.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    /// Three bytes per pixel: R, G, B, fully opaque.
    Rgb,
    /// Four bytes per pixel: R, G, B, A, where R/G/B are already multiplied
    /// by A (the format produced by the compiler's `EmbedTextures` pass for
    /// images with transparency).
    RgbaPremultiplied,
}

/// A texture (image) stored in read-only memory.
///
/// Generated code produces a `static` value of this type for every
/// `@image-url(...)` expression in the .slint source.
///
/// [`StaticTexture::data`] holds the *trimmed* non-transparent sub-rectangle
/// of the original image.
/// Its pixel footprint in the source coordinate system starts at
/// (`offset_x`, `offset_y`) — the generator copies these from the compiler's
/// `Texture::rect.x()/.y()`.
/// [`draw_static_texture`] adds that offset to the destination so the
/// visible pixels land in the same place the full software renderer would
/// put them.
pub struct StaticTexture {
    /// Width of the stored (trimmed) pixel block, in pixels.
    pub width: u32,
    /// Height of the stored (trimmed) pixel block, in pixels.
    pub height: u32,
    /// X offset of the trimmed block inside the original image.
    pub offset_x: u32,
    /// Y offset of the trimmed block inside the original image.
    pub offset_y: u32,
    /// Raw pixel bytes, laid out row-major according to [`StaticTexture::format`].
    pub data: &'static [u8],
    /// Pixel format of [`StaticTexture::data`].
    pub format: PixelFormat,
}

/// Fills an axis-aligned rectangle with a solid color, blending with the
/// existing buffer contents using the `src OVER dst` formula.
///
/// `x` and `y` may be negative and `width`/`height` may extend past the
/// buffer: the function clips internally.  A fully-opaque color replaces the
/// destination pixels without a per-pixel multiply.
pub fn fill_rectangle<B: TargetPixelBuffer>(
    buffer: &mut B,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    color: Color,
) {
    if width <= 0 || height <= 0 || color.alpha() == 0 {
        return;
    }
    let (x1, y1, x2, y2) = match clip_rect(buffer, x, y, width, height) {
        Some(r) => r,
        None => return,
    };
    let premul = color.to_premultiplied();
    if premul.alpha == 0xff {
        let opaque =
            <B::TargetPixel as TargetPixel>::from_rgb(color.red(), color.green(), color.blue());
        for row in y1..y2 {
            for px in buffer.line_slice(row)[x1..x2].iter_mut() {
                *px = opaque;
            }
        }
    } else {
        for row in y1..y2 {
            for px in buffer.line_slice(row)[x1..x2].iter_mut() {
                px.blend(premul);
            }
        }
    }
}

/// Draws a [`StaticTexture`] at `(x, y)` in buffer coordinates.
///
/// The texture is drawn at its natural pixel size with no scaling or
/// rotation.
/// `(x, y)` refers to the image's top-left *before* trimming; the function
/// adds the texture's [`offset_x`](StaticTexture::offset_x) /
/// [`offset_y`](StaticTexture::offset_y) internally so the visible pixels
/// land in the correct place.
/// Clipping to the buffer is performed internally.
pub fn draw_static_texture<B: TargetPixelBuffer>(
    buffer: &mut B,
    x: i32,
    y: i32,
    texture: &StaticTexture,
) {
    if texture.width == 0 || texture.height == 0 {
        return;
    }
    let tw = texture.width as i32;
    let th = texture.height as i32;
    let origin_x = x + texture.offset_x as i32;
    let origin_y = y + texture.offset_y as i32;
    let (dst_x1, dst_y1, dst_x2, dst_y2) = match clip_rect(buffer, origin_x, origin_y, tw, th) {
        Some(r) => r,
        None => return,
    };
    let copy_w = dst_x2 - dst_x1;
    let src_x = (dst_x1 as i32 - origin_x) as usize;
    let src_y = (dst_y1 as i32 - origin_y) as usize;

    // The format match is hoisted out of the pixel loop so each pixel walks a
    // monomorphic inner body.  The inner loops use slice iterators so the
    // compiler elides the per-pixel bounds checks that indexed access would
    // incur.
    match texture.format {
        PixelFormat::Rgb => {
            const BPP: usize = 3;
            let src_stride = texture.width as usize * BPP;
            for row in 0..(dst_y2 - dst_y1) {
                let src_off = (src_y + row) * src_stride + src_x * BPP;
                let src_row = &texture.data[src_off..src_off + copy_w * BPP];
                let dst_row = &mut buffer.line_slice(dst_y1 + row)[dst_x1..dst_x2];
                for (dst_px, chunk) in dst_row.iter_mut().zip(src_row.chunks_exact(BPP)) {
                    *dst_px = <B::TargetPixel as TargetPixel>::from_rgb(chunk[0], chunk[1], chunk[2]);
                }
            }
        }
        PixelFormat::RgbaPremultiplied => {
            const BPP: usize = 4;
            let src_stride = texture.width as usize * BPP;
            for row in 0..(dst_y2 - dst_y1) {
                let src_off = (src_y + row) * src_stride + src_x * BPP;
                let src_row = &texture.data[src_off..src_off + copy_w * BPP];
                let dst_row = &mut buffer.line_slice(dst_y1 + row)[dst_x1..dst_x2];
                for (dst_px, chunk) in dst_row.iter_mut().zip(src_row.chunks_exact(BPP)) {
                    let a = chunk[3];
                    if a == 0 {
                        continue;
                    }
                    if a == 0xff {
                        *dst_px =
                            <B::TargetPixel as TargetPixel>::from_rgb(chunk[0], chunk[1], chunk[2]);
                    } else {
                        dst_px.blend(PremultipliedRgbaColor {
                            alpha: a,
                            red: chunk[0],
                            green: chunk[1],
                            blue: chunk[2],
                        });
                    }
                }
            }
        }
    }
}

/// Clips a rectangle given in buffer coordinates to the buffer bounds and
/// returns `Some((x1, y1, x2, y2))` in `usize` (x2/y2 are exclusive) if the
/// result is non-empty.  Returns `None` otherwise.
fn clip_rect<B: TargetPixelBuffer>(
    buffer: &mut B,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Option<(usize, usize, usize, usize)> {
    let height_lines = buffer.num_lines() as i32;
    // Assume every line has the same width.
    let buf_w = if height_lines > 0 { buffer.line_slice(0).len() as i32 } else { 0 };
    if buf_w == 0 || height_lines == 0 {
        return None;
    }
    let x1 = x.max(0);
    let y1 = y.max(0);
    let x2 = (x.saturating_add(width)).min(buf_w);
    let y2 = (y.saturating_add(height)).min(height_lines);
    if x1 >= x2 || y1 >= y2 {
        None
    } else {
        Some((x1 as usize, y1 as usize, x2 as usize, y2 as usize))
    }
}
