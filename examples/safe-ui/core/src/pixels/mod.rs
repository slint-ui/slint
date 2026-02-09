// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com
// SPDX-License-Identifier: MIT

#[cfg(feature = "pixel-bgra8888")]
mod bgra8888;

#[cfg(feature = "pixel-bgra8888")]
pub type PlatformPixel = crate::pixels::bgra8888::Bgra8888Pixel;
#[cfg(feature = "pixel-rgb565")]
pub type PlatformPixel = slint::platform::software_renderer::Rgb565Pixel;
#[cfg(feature = "pixel-rgb888")]
pub type PlatformPixel = slint::Rgb8Pixel;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb8Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[cfg(feature = "pixel-bgra8888")]
impl From<PlatformPixel> for Rgb8Pixel {
    fn from(p: PlatformPixel) -> Self {
        let v = p.0;
        Self { r: ((v >> 16) & 0xFF) as u8, g: ((v >> 8) & 0xFF) as u8, b: (v & 0xFF) as u8 }
    }
}

#[cfg(feature = "pixel-rgb565")]
impl From<PlatformPixel> for Rgb8Pixel {
    fn from(p: PlatformPixel) -> Self {
        let v = p.0;
        let r5 = ((v >> 11) & 0x1F) as u8;
        let g6 = ((v >> 5) & 0x3F) as u8;
        let b5 = (v & 0x1F) as u8;

        Self { r: (r5 << 3) | (r5 >> 2), g: (g6 << 2) | (g6 >> 4), b: (b5 << 3) | (b5 >> 2) }
    }
}

#[cfg(feature = "pixel-rgb888")]
impl From<PlatformPixel> for Rgb8Pixel {
    fn from(p: PlatformPixel) -> Self {
        Self { r: p.r, g: p.g, b: p.b }
    }
}
