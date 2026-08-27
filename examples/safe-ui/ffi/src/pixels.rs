// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com
// SPDX-License-Identifier: MIT

//! Frame-buffer pixel formats. Slint SC renders packed RGB8; these types
//! convert a pixel into the format the display expects, without depending on
//! the full Slint runtime.

/// The format the display expects, selected by feature.
#[cfg(feature = "pixel-bgra8888")]
pub type PlatformPixel = Bgra8888Pixel;
#[cfg(feature = "pixel-rgb565")]
pub type PlatformPixel = Rgb565Pixel;
#[cfg(feature = "pixel-rgb888")]
pub type PlatformPixel = Rgb888Pixel;

/// An 8-bit-per-channel RGB pixel, for reading the frame buffer back for display.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb8Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// A 32-bit BGRA pixel, blue in the least significant byte.
#[repr(transparent)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Bgra8888Pixel(pub u32);

impl Bgra8888Pixel {
    pub const fn from_rgb8(r: u8, g: u8, b: u8) -> Self {
        Self(0xff000000 | ((r as u32) << 16) | ((g as u32) << 8) | b as u32)
    }
}

impl From<Bgra8888Pixel> for Rgb8Pixel {
    fn from(p: Bgra8888Pixel) -> Self {
        let v = p.0;
        Self { r: (v >> 16) as u8, g: (v >> 8) as u8, b: v as u8 }
    }
}

/// A 16-bit RGB pixel, 5 bits red, 6 bits green, 5 bits blue.
#[repr(transparent)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Rgb565Pixel(pub u16);

impl Rgb565Pixel {
    pub const fn from_rgb8(r: u8, g: u8, b: u8) -> Self {
        Self(((r as u16 & 0xf8) << 8) | ((g as u16 & 0xfc) << 3) | (b as u16 >> 3))
    }
}

impl From<Rgb565Pixel> for Rgb8Pixel {
    fn from(p: Rgb565Pixel) -> Self {
        let v = p.0;
        let r5 = ((v >> 11) & 0x1f) as u8;
        let g6 = ((v >> 5) & 0x3f) as u8;
        let b5 = (v & 0x1f) as u8;
        // Replicate the high bits into the low ones so full channels stay full.
        Self { r: (r5 << 3) | (r5 >> 2), g: (g6 << 2) | (g6 >> 4), b: (b5 << 3) | (b5 >> 2) }
    }
}

/// A 24-bit RGB pixel, one byte per channel.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Rgb888Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb888Pixel {
    pub const fn from_rgb8(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

impl From<Rgb888Pixel> for Rgb8Pixel {
    fn from(p: Rgb888Pixel) -> Self {
        Self { r: p.r, g: p.g, b: p.b }
    }
}
