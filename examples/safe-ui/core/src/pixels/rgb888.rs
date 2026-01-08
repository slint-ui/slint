// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author Marco Thaller <marco.thaller@kdab.com>
// SPDX-License-Identifier: MIT

#[repr(transparent)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Rgb888Pixel(pub [u8; 3]);

impl Rgb888Pixel {
    #[inline]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self([r, g, b])
    }

    #[inline]
    pub const fn r(&self) -> u8 {
        self.0[0]
    }
    #[inline]
    pub const fn g(&self) -> u8 {
        self.0[1]
    }
    #[inline]
    pub const fn b(&self) -> u8 {
        self.0[2]
    }
}

impl From<Rgb888Pixel> for slint::platform::software_renderer::PremultipliedRgbaColor {
    #[inline]
    fn from(pixel: Rgb888Pixel) -> Self {
        slint::platform::software_renderer::PremultipliedRgbaColor {
            red: pixel.r(),
            green: pixel.g(),
            blue: pixel.b(),
            alpha: 255,
        }
    }
}

impl From<slint::platform::software_renderer::PremultipliedRgbaColor> for Rgb888Pixel {
    #[inline]
    fn from(pixel: slint::platform::software_renderer::PremultipliedRgbaColor) -> Self {
        Self::new(pixel.red, pixel.green, pixel.blue)
    }
}

impl slint::platform::software_renderer::TargetPixel for Rgb888Pixel {
    fn blend(&mut self, color: slint::platform::software_renderer::PremultipliedRgbaColor) {
        let mut x = slint::platform::software_renderer::PremultipliedRgbaColor::from(*self);
        x.blend(color);
        *self = x.into();
    }
    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b)
    }
    fn background() -> Self {
        Self::new(0, 0, 0)
    }
}
