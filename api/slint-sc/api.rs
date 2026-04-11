// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Public API of the Slint SC runtime.
//!
//! The shape mirrors the corresponding pieces of the `slint` crate but with a
//! much smaller surface: there is no `ComponentHandle`, no event loop, no
//! properties API.  A user-defined component is a plain Rust struct generated
//! by the compiler; the user constructs it with `MainWindow::new()` (returning
//! by value), sets properties through `set_*` / `get_*` methods, and calls
//! `render(&mut buffer)` whenever the screen should be repainted.

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

/// A physical size (in device pixels).
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub struct PhysicalSize {
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
}

impl PhysicalSize {
    /// Creates a new physical size.
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// A physical position (in device pixels).
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub struct PhysicalPosition {
    /// Horizontal offset in physical pixels.
    pub x: i32,
    /// Vertical offset in physical pixels.
    pub y: i32,
}

impl PhysicalPosition {
    /// Creates a new physical position.
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

// ---------------------------------------------------------------------------
// Color
// ---------------------------------------------------------------------------

/// An 8-bit per channel ARGB color, matching the public `slint::Color` type.
///
/// Unlike the normal Slint color, which has several helper conversions, this
/// type keeps only what is strictly needed to describe a fill color.
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub struct Color {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl Color {
    /// Creates an opaque color from the given 8-bit RGB channels.
    pub const fn from_rgb_u8(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue, alpha: 255 }
    }

    /// Creates a color from the given 8-bit ARGB channels (non-premultiplied).
    pub const fn from_argb_u8(alpha: u8, red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue, alpha }
    }

    /// Creates a color from a packed 0xAARRGGBB encoded value.
    pub const fn from_argb_encoded(encoded: u32) -> Self {
        Self {
            red: ((encoded >> 16) & 0xff) as u8,
            green: ((encoded >> 8) & 0xff) as u8,
            blue: (encoded & 0xff) as u8,
            alpha: ((encoded >> 24) & 0xff) as u8,
        }
    }

    /// Returns the red channel (0-255).
    pub const fn red(&self) -> u8 {
        self.red
    }
    /// Returns the green channel (0-255).
    pub const fn green(&self) -> u8 {
        self.green
    }
    /// Returns the blue channel (0-255).
    pub const fn blue(&self) -> u8 {
        self.blue
    }
    /// Returns the alpha channel (0-255).
    pub const fn alpha(&self) -> u8 {
        self.alpha
    }

    /// Returns the premultiplied form used internally by the renderer.
    pub const fn to_premultiplied(self) -> PremultipliedRgbaColor {
        // `const` friendly saturating multiplication without importing libm.
        let a = self.alpha as u16;
        PremultipliedRgbaColor {
            alpha: self.alpha,
            red: ((self.red as u16 * a + 127) / 255) as u8,
            green: ((self.green as u16 * a + 127) / 255) as u8,
            blue: ((self.blue as u16 * a + 127) / 255) as u8,
        }
    }
}

/// A pre-multiplied RGBA color used by the renderer.
///
/// Pre-multiplied means that the stored `red`, `green` and `blue` values are
/// already `original_rgb * alpha / 255`.  This allows alpha blending to use
/// one multiply per channel instead of two.
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub struct PremultipliedRgbaColor {
    /// Alpha channel (0-255).
    pub alpha: u8,
    /// Pre-multiplied red.
    pub red: u8,
    /// Pre-multiplied green.
    pub green: u8,
    /// Pre-multiplied blue.
    pub blue: u8,
}

// ---------------------------------------------------------------------------
// Pixel types
// ---------------------------------------------------------------------------

/// Trait implemented by pixel types that a [`TargetPixelBuffer`] can store.
///
/// Implementations must blend a pre-multiplied source color into the pixel
/// using the `src OVER dst` formula.
pub trait TargetPixel: Copy + Default {
    /// Blends `color` on top of `self` using Porter-Duff "over".
    fn blend(&mut self, color: PremultipliedRgbaColor);

    /// Creates an opaque pixel from the given 8-bit RGB channels.
    fn from_rgb(red: u8, green: u8, blue: u8) -> Self;
}

/// One channel of the `src OVER dst` formula with a pre-multiplied source:
/// `out = dst * (1 - alpha) + src_premul`.
#[inline]
fn blend_channel(dst: u8, inv_alpha: u16, src_premul: u8) -> u8 {
    (dst as u16 * inv_alpha / 255) as u8 + src_premul
}

/// A 24-bit opaque RGB pixel (3 bytes, red first).
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Rgb8Pixel {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

impl TargetPixel for Rgb8Pixel {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let inv = 255u16 - color.alpha as u16;
        self.r = blend_channel(self.r, inv, color.red);
        self.g = blend_channel(self.g, inv, color.green);
        self.b = blend_channel(self.b, inv, color.blue);
    }
    fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self { r: red, g: green, b: blue }
    }
}

/// A 32-bit RGBA pixel (4 bytes, red first).  Not pre-multiplied when the
/// alpha channel is read back, but written in premultiplied form by `blend`.
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Rgba8Pixel {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel.
    pub a: u8,
}

impl TargetPixel for Rgba8Pixel {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let inv = 255u16 - color.alpha as u16;
        self.r = blend_channel(self.r, inv, color.red);
        self.g = blend_channel(self.g, inv, color.green);
        self.b = blend_channel(self.b, inv, color.blue);
        self.a = blend_channel(self.a, inv, color.alpha);
    }
    fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self { r: red, g: green, b: blue, a: 255 }
    }
}

/// A 32-bit BGRA pixel packed into a single `u32`, blue in the lowest byte
/// and alpha in the highest.  This matches the memory layout of many
/// embedded LCD drivers.
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct Bgra8Pixel(pub u32);

impl TargetPixel for Bgra8Pixel {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let v = self.0;
        let inv = 255u16 - color.alpha as u16;
        let b = blend_channel(v as u8, inv, color.blue);
        let g = blend_channel((v >> 8) as u8, inv, color.green);
        let r = blend_channel((v >> 16) as u8, inv, color.red);
        let a = blend_channel((v >> 24) as u8, inv, color.alpha);
        self.0 = ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
    }
    fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self(0xff000000 | ((red as u32) << 16) | ((green as u32) << 8) | (blue as u32))
    }
}

/// A 16-bit RGB565 pixel (5 bits red, 6 bits green, 5 bits blue) packed
/// into a single `u16`.  Common on low-memory embedded displays.
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct Rgb565Pixel(pub u16);

impl TargetPixel for Rgb565Pixel {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        // Unpack to 8-bit per channel, blend, repack.
        let v = self.0;
        let r5 = ((v >> 11) & 0x1f) as u8;
        let g6 = ((v >> 5) & 0x3f) as u8;
        let b5 = (v & 0x1f) as u8;
        let r = (r5 << 3) | (r5 >> 2);
        let g = (g6 << 2) | (g6 >> 4);
        let b = (b5 << 3) | (b5 >> 2);
        let inv = 255u16 - color.alpha as u16;
        *self = Self::from_rgb(
            blend_channel(r, inv, color.red),
            blend_channel(g, inv, color.green),
            blend_channel(b, inv, color.blue),
        );
    }
    fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self(((red as u16 >> 3) << 11) | ((green as u16 >> 2) << 5) | (blue as u16 >> 3))
    }
}

// ---------------------------------------------------------------------------
// Pixel buffer
// ---------------------------------------------------------------------------

/// Trait describing a target pixel buffer that generated components can draw
/// into.  Inspired by the `TargetPixelBuffer` trait in the full software
/// renderer but stripped to the two methods the SC renderer needs.
pub trait TargetPixelBuffer {
    /// The concrete pixel type stored by this buffer.
    type TargetPixel: TargetPixel;

    /// Returns the mutable slice for the given scan-line.  The slice length
    /// equals the buffer's width in pixels.
    fn line_slice(&mut self, line: usize) -> &mut [Self::TargetPixel];

    /// Returns the number of scan-lines (i.e. the height in pixels).
    fn num_lines(&self) -> usize;
}

/// A simple [`TargetPixelBuffer`] implementation backed by a contiguous slice
/// of pixels with a known stride.  Convenient when you already own a frame
/// buffer as a `&mut [P]`.
pub struct SliceBuffer<'a, P: TargetPixel> {
    data: &'a mut [P],
    stride: usize,
    height: usize,
}

impl<'a, P: TargetPixel> SliceBuffer<'a, P> {
    /// Wraps the given slice as a pixel buffer.
    ///
    /// `stride` is the number of pixels per row (may exceed the visible
    /// `width` if the caller allocated a padded row) and `height` is the
    /// number of visible rows.  The slice must be long enough to hold
    /// `stride * height` pixels.
    pub fn new(data: &'a mut [P], stride: usize, height: usize) -> Self {
        assert!(data.len() >= stride * height, "pixel buffer too small for stride * height");
        Self { data, stride, height }
    }
}

impl<P: TargetPixel> TargetPixelBuffer for SliceBuffer<'_, P> {
    type TargetPixel = P;
    fn line_slice(&mut self, line: usize) -> &mut [P] {
        let start = line * self.stride;
        &mut self.data[start..start + self.stride]
    }
    fn num_lines(&self) -> usize {
        self.height
    }
}
