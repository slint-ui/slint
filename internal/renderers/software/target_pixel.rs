// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The pixels the software renderer draws into.

use i_slint_core::graphics::{Rgb8Pixel, Rgba8Pixel};
use i_slint_core::{Color, InternalToken};

/// A color whose component have been pre-multiplied by alpha
///
/// The renderer operates faster on pre-multiplied color since it
/// caches the multiplication of its component
///
/// PremultipliedRgbaColor can be constructed from a [`Color`] with
/// the [`From`] trait. This conversion will pre-multiply the color
/// components
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct PremultipliedRgbaColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

/// Convert a non-premultiplied color to a premultiplied one
impl From<Color> for PremultipliedRgbaColor {
    fn from(col: Color) -> Self {
        Self::premultiply(col)
    }
}

impl PremultipliedRgbaColor {
    /// Convert a non premultiplied color to a premultiplied one
    pub(crate) fn premultiply(col: Color) -> Self {
        let a = col.alpha() as u16;
        Self {
            alpha: col.alpha(),
            red: (col.red() as u16 * a / 255) as u8,
            green: (col.green() as u16 * a / 255) as u8,
            blue: (col.blue() as u16 * a / 255) as u8,
        }
    }
}

/// Trait for the pixels in the buffer
pub trait TargetPixel: Sized + Copy {
    /// Blend a single pixel with a color
    fn blend(&mut self, color: PremultipliedRgbaColor);
    /// Blend a color to all the pixel in the slice.
    fn blend_slice(slice: &mut [Self], color: PremultipliedRgbaColor) {
        if color.alpha == u8::MAX {
            slice.fill(Self::from_rgb(color.red, color.green, color.blue))
        } else {
            for x in slice {
                Self::blend(x, color);
            }
        }
    }
    /// Create a pixel from the red, gree, blue component in the range 0..=255
    fn from_rgb(red: u8, green: u8, blue: u8) -> Self;

    /// Pixel which will be filled as the background in case the slint view has transparency
    fn background() -> Self {
        Self::from_rgb(0, 0, 0)
    }

    /// Like `blend`, with `noise` in 0..=255 (a fraction of one quantization step).
    /// Formats with less than 8 bits per channel can use it to dither.
    #[doc(hidden)]
    #[inline]
    fn blend_dithered(&mut self, color: PremultipliedRgbaColor, _noise: u8, _: InternalToken) {
        self.blend(color)
    }

    /// Blends `src`, pixels of an RGBA texture, at opacity `alpha`.
    #[doc(hidden)]
    #[inline]
    fn blend_rgba_slice<S: RgbaTexel>(slice: &mut [Self], src: &[S], alpha: u8, _: InternalToken) {
        for (pix, p) in slice.iter_mut().zip(src) {
            p.blend_into(pix, alpha);
        }
    }
}

/// The number of pixels [`blend_rgba_slice_in_bulk`] classifies at a time.
pub(crate) const RGBA_CHUNK: usize = 128;

/// [`TargetPixel::blend_rgba_slice`] for the pixel types that blend opaque pixels faster in bulk,
/// with `blend_opaque`: chunks that are all transparent are skipped, and those that are all opaque
/// blended in one call.
#[inline(always)]
pub(crate) fn blend_rgba_slice_in_bulk<T: TargetPixel, S: RgbaTexel>(
    slice: &mut [T],
    src: &[S],
    alpha: u8,
    blend_opaque: impl Fn(&mut [T], &[Rgba8Pixel], u8),
) {
    if alpha == 0 {
        return;
    }
    // The alpha is the last byte of both texel types: the top byte of a little-endian u32.
    let to_u32 = |p: &S| u32::from_le_bytes(bytemuck::cast(*p));
    for (slice, src) in slice.chunks_mut(RGBA_CHUNK).zip(src.chunks(RGBA_CHUNK)) {
        // A chunk is all opaque or all transparent only if its first pixel is.
        if src.first().is_some_and(|p| matches!(to_u32(p) >> 24, 0 | 0xff)) {
            let (all, any) =
                src.iter().map(to_u32).fold((!0, 0), |(all, any), p| (all & p, any | p));
            if any >> 24 == 0 {
                continue;
            }
            if all >> 24 == 0xff {
                // Opaque pixels are the same with straight or premultiplied alpha.
                blend_opaque(slice, bytemuck::cast_slice(src), alpha);
                continue;
            }
        }
        for (pix, p) in slice.iter_mut().zip(src) {
            p.blend_into(pix, alpha);
        }
    }
}

#[inline]
pub(crate) fn blend_opaque_per_pixel(
    slice: &mut [impl TargetPixel],
    src: &[Rgba8Pixel],
    alpha: u8,
) {
    for (pix, p) in slice.iter_mut().zip(src) {
        blend_rgba(pix, alpha, *p);
    }
}

/// Blends `p`, ignoring its alpha, at opacity `alpha`.
#[inline(always)]
fn blend_rgba(pix: &mut impl TargetPixel, alpha: u8, p: Rgba8Pixel) {
    match alpha {
        0 => {}
        0xff => *pix = TargetPixel::from_rgb(p.r, p.g, p.b),
        a => pix.blend(PremultipliedRgbaColor::premultiply(Color::from_argb_u8(a, p.r, p.g, p.b))),
    }
}

impl TargetPixel for Rgb8Pixel {
    #[inline]
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let a = (u8::MAX - color.alpha) as u16;
        self.r = (self.r as u16 * a / 255) as u8 + color.red;
        self.g = (self.g as u16 * a / 255) as u8 + color.green;
        self.b = (self.b as u16 * a / 255) as u8 + color.blue;
    }

    #[inline]
    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b)
    }

    fn blend_rgba_slice<S: RgbaTexel>(slice: &mut [Self], src: &[S], alpha: u8, _: InternalToken) {
        blend_rgba_slice_in_bulk(slice, src, alpha, blend_opaque_per_pixel)
    }
}

impl TargetPixel for PremultipliedRgbaColor {
    #[inline]
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let a = (u8::MAX - color.alpha) as u16;
        self.red = (self.red as u16 * a / 255) as u8 + color.red;
        self.green = (self.green as u16 * a / 255) as u8 + color.green;
        self.blue = (self.blue as u16 * a / 255) as u8 + color.blue;
        self.alpha = (self.alpha as u16 + color.alpha as u16
            - (self.alpha as u16 * color.alpha as u16) / 255) as u8;
    }

    #[inline]
    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { red: r, green: g, blue: b, alpha: 255 }
    }

    fn background() -> Self {
        Self { red: 0, green: 0, blue: 0, alpha: 0 }
    }
}

pub use i_slint_core::graphics::Rgb565Pixel;

const R_MASK: u16 = 0b1111_1000_0000_0000;
const G_MASK: u16 = 0b0000_0111_1110_0000;
const B_MASK: u16 = 0b0000_0000_0001_1111;

impl TargetPixel for Rgb565Pixel {
    #[inline]
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let a = (u8::MAX - color.alpha) as u32;
        // convert to 5 bits
        let a = (a + 4) >> 3;

        // 00000ggg_ggg00000_rrrrr000_000bbbbb
        let expanded = (self.0 & (R_MASK | B_MASK)) as u32 | (((self.0 & G_MASK) as u32) << 16);

        // gggggggg_000rrrrr_rrr000bb_bbbbbb00
        let c =
            ((color.red as u32) << 13) | ((color.green as u32) << 24) | ((color.blue as u32) << 2);
        // gggggg00_000rrrrr_000000bb_bbb00000
        let c = c & 0b11111100_00011111_00000011_11100000;

        let res = expanded * a + c;

        self.0 = ((res >> 21) as u16 & G_MASK) | ((res >> 5) as u16 & (R_MASK | B_MASK));
    }

    #[inline]
    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        // This calls the inherent from_rgb, not this trait method.
        Rgb565Pixel::from_rgb(r, g, b)
    }

    #[inline]
    fn blend_dithered(&mut self, color: PremultipliedRgbaColor, noise: u8, _: InternalToken) {
        self.blend(dither_rgb565(color, noise))
    }

    fn blend_rgba_slice<S: RgbaTexel>(slice: &mut [Self], src: &[S], alpha: u8, _: InternalToken) {
        blend_rgba_slice_in_bulk(slice, src, alpha, blend_opaque_per_pixel)
    }
}

/// See [`TargetPixel::blend_dithered`].
#[inline]
pub(crate) fn dither_rgb565(color: PremultipliedRgbaColor, noise: u8) -> PremultipliedRgbaColor {
    let dither = |v: u8, bits: u8| v.saturating_add(noise >> bits).min(color.alpha);
    PremultipliedRgbaColor {
        red: dither(color.red, 5),
        green: dither(color.green, 6),
        blue: dither(color.blue, 5),
        alpha: color.alpha,
    }
}

// cSpell: ignore RRRRRGGG GGGBBBBB bswap

/// A 16bit RGB565 pixel stored in big-endian byte order.
///
/// The in-memory byte layout is `[RRRRRGGG, GGGBBBBB]` regardless of
/// host endianness — the format expected by most SPI display
/// controllers (ILI9341, ILI9342C, ST7789, etc.) without any
/// post-render byte swapping.
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Rgb565BigEndianPixel(pub u16);

impl Rgb565BigEndianPixel {
    /// Return the red component as a u8.
    ///
    /// The bits are shifted so that the result is between 0 and 255
    pub fn red(self) -> u8 {
        Rgb565Pixel(u16::from_be(self.0)).red()
    }
    /// Return the green component as a u8.
    ///
    /// The bits are shifted so that the result is between 0 and 255
    pub fn green(self) -> u8 {
        Rgb565Pixel(u16::from_be(self.0)).green()
    }
    /// Return the blue component as a u8.
    ///
    /// The bits are shifted so that the result is between 0 and 255
    pub fn blue(self) -> u8 {
        Rgb565Pixel(u16::from_be(self.0)).blue()
    }
}

impl TargetPixel for Rgb565BigEndianPixel {
    #[inline]
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        // Reuse the canonical native-endian Rgb565Pixel::blend by decoding
        // from BE byte order, blending, and re-encoding. On targets with a
        // byte-swap instruction (ARM REV16, RISC-V Zbb rev8, x86 bswap) each
        // `to_be`/`from_be` is one cycle on a little-endian host and a no-op
        // on a big-endian host. Benchmarking this against a direct-BE
        // bit-reassembly variant on Cortex-M33 showed the swap-around-native
        // form generates tighter code.
        let mut native = Rgb565Pixel(u16::from_be(self.0));
        native.blend(color);
        self.0 = native.0.to_be();
    }

    #[inline]
    fn blend_dithered(&mut self, color: PremultipliedRgbaColor, noise: u8, _: InternalToken) {
        self.blend(dither_rgb565(color, noise))
    }

    #[inline]
    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self(Rgb565Pixel::from_rgb(r, g, b).0.to_be())
    }
}

impl From<Rgb8Pixel> for Rgb565BigEndianPixel {
    fn from(p: Rgb8Pixel) -> Self {
        Self(Rgb565Pixel::from(p).0.to_be())
    }
}

impl From<Rgb565BigEndianPixel> for Rgb8Pixel {
    fn from(p: Rgb565BigEndianPixel) -> Self {
        Rgb565Pixel(u16::from_be(p.0)).into()
    }
}

/// A pixel of an RGBA texture, with straight or premultiplied alpha.
#[doc(hidden)]
pub trait RgbaTexel: bytemuck::Pod {
    /// Blends this pixel onto `pix`, at opacity `alpha`.
    fn blend_into(&self, pix: &mut impl TargetPixel, alpha: u8);
}

impl RgbaTexel for Rgba8Pixel {
    #[inline(always)]
    fn blend_into(&self, pix: &mut impl TargetPixel, alpha: u8) {
        blend_rgba(pix, ((self.a as u16 * alpha as u16) / 255) as u8, *self);
    }
}

impl RgbaTexel for PremultipliedRgbaColor {
    #[inline(always)]
    fn blend_into(&self, pix: &mut impl TargetPixel, alpha: u8) {
        if alpha == 0xff {
            match self.alpha {
                0 => {}
                0xff => *pix = TargetPixel::from_rgb(self.red, self.green, self.blue),
                _ => pix.blend(*self),
            }
        } else if self.alpha != 0 {
            let scale = |c: u8| (c as u16 * alpha as u16 / 255) as u8;
            pix.blend(PremultipliedRgbaColor {
                alpha: scale(self.alpha),
                red: scale(self.red),
                green: scale(self.green),
                blue: scale(self.blue),
            });
        }
    }
}

#[test]
fn rgb565() {
    let pix565 = Rgb565Pixel::from_rgb(0xff, 0x25, 0);
    let pix888: Rgb8Pixel = pix565.into();
    assert_eq!(pix565, pix888.into());

    let pix565 = Rgb565Pixel::from_rgb(0x56, 0x42, 0xe3);
    let pix888: Rgb8Pixel = pix565.into();
    assert_eq!(pix565, pix888.into());
}

#[test]
fn rgb565_full_component_expands_to_255() {
    // The C++ Rgb565Pixel accessors replicate the high bits the same way, so an RGB565 image
    // has to reach pure white on every renderer, not #f8fcf8.
    let white = Rgb565Pixel::from_rgb(0xff, 0xff, 0xff);
    assert_eq!(white.red(), 0xff);
    assert_eq!(white.green(), 0xff);
    assert_eq!(white.blue(), 0xff);

    let black = Rgb565Pixel::from_rgb(0, 0, 0);
    assert_eq!(black.red(), 0);
    assert_eq!(black.green(), 0);
    assert_eq!(black.blue(), 0);

    let white_be = Rgb565BigEndianPixel::from_rgb(0xff, 0xff, 0xff);
    assert_eq!(white_be.red(), 0xff);
    assert_eq!(white_be.green(), 0xff);
    assert_eq!(white_be.blue(), 0xff);
}

#[test]
fn rgb565_be() {
    // BE should be byte-swapped LE for any color
    for &(r, g, b) in &[(0xff, 0x25, 0u8), (0x56, 0x42, 0xe3), (0, 0xff, 0), (0, 0, 0xff)] {
        let le = Rgb565Pixel::from_rgb(r, g, b);
        let be = Rgb565BigEndianPixel::from_rgb(r, g, b);
        assert_eq!(le.0.swap_bytes(), be.0, "mismatch for ({r}, {g}, {b})");
    }

    // Round-trip through Rgb8Pixel
    let pix_be = Rgb565BigEndianPixel::from_rgb(0xff, 0x25, 0);
    let pix888: Rgb8Pixel = pix_be.into();
    assert_eq!(pix_be, pix888.into());

    let pix_be = Rgb565BigEndianPixel::from_rgb(0x56, 0x42, 0xe3);
    let pix888: Rgb8Pixel = pix_be.into();
    assert_eq!(pix_be, pix888.into());
}

#[test]
fn rgb565_round_trips_through_from_rgb() {
    // Drawing an RGB565 image relies on this to copy pixels unchanged to an RGB565 target.
    for value in 0..=u16::MAX {
        let p = Rgb565Pixel(value);
        assert_eq!(<Rgb565Pixel as TargetPixel>::from_rgb(p.red(), p.green(), p.blue()), p);
        let be = Rgb565BigEndianPixel::from_rgb(p.red(), p.green(), p.blue());
        assert_eq!(be.0, value.to_be());
    }
}

#[test]
fn rgb565_be_blend() {
    // Blending a BE pixel should produce the same visual result as LE
    let color = PremultipliedRgbaColor { red: 127, green: 0, blue: 0, alpha: 127 };

    let mut le = Rgb565Pixel::from_rgb(0, 0, 255);
    le.blend(color);
    let mut be = Rgb565BigEndianPixel::from_rgb(0, 0, 255);
    be.blend(color);
    assert_eq!(le.0.swap_bytes(), be.0);

    // Blend with green over a white background
    let color = PremultipliedRgbaColor { red: 0, green: 200, blue: 0, alpha: 200 };

    let mut le = Rgb565Pixel::from_rgb(255, 255, 255);
    le.blend(color);
    let mut be = Rgb565BigEndianPixel::from_rgb(255, 255, 255);
    be.blend(color);
    assert_eq!(le.0.swap_bytes(), be.0);
}
