// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use slint::platform::software_renderer::{
    PremultipliedRgbaColor, Rgb565BigEndianPixel, TargetPixel,
};

#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub(super) struct Pixel(pub Rgb565BigEndianPixel);

pub(super) fn as_be(pixels: &[Pixel]) -> &[Rgb565BigEndianPixel] {
    // Pixel is a transparent wrapper with the same size and alignment.
    unsafe { core::slice::from_raw_parts(pixels.as_ptr().cast(), pixels.len()) }
}

pub(super) fn as_mut(pixels: &mut [Rgb565BigEndianPixel]) -> &mut [Pixel] {
    // The exclusive borrow covers transparent pixels with identical validity.
    unsafe { core::slice::from_raw_parts_mut(pixels.as_mut_ptr().cast(), pixels.len()) }
}

impl TargetPixel for Pixel {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        self.0.blend(color);
    }
    fn blend_slice(pixels: &mut [Self], color: PremultipliedRgbaColor) {
        if color.alpha == u8::MAX && pixels.len() >= 32 {
            let color = Rgb565BigEndianPixel::from_rgb(color.red, color.green, color.blue).0;
            // Each transparent pixel contains exactly one initialized u16.
            let words = unsafe {
                core::slice::from_raw_parts_mut(pixels.as_mut_ptr().cast::<u16>(), pixels.len())
            };
            super::pie_fill::fill(words, color);
        } else {
            let canonical = unsafe {
                core::slice::from_raw_parts_mut(pixels.as_mut_ptr().cast(), pixels.len())
            };
            Rgb565BigEndianPixel::blend_slice(canonical, color);
        }
    }
    fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self(Rgb565BigEndianPixel::from_rgb(red, green, blue))
    }
    fn from_rgb565(value: u16) -> Self {
        Self(Rgb565BigEndianPixel::from_rgb565(value))
    }
}
