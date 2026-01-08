// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author Marco Thaller <marco.thaller@kdab.com>
// SPDX-License-Identifier: MIT

#[repr(transparent)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Rgb565Pixel(pub u16);

impl From<Rgb565Pixel> for slint::platform::software_renderer::PremultipliedRgbaColor {
    #[inline]
    fn from(pixel: Rgb565Pixel) -> Self {
        let v = pixel.0;
        let r = ((v >> 11) & 0x1F) as u8;
        let g = ((v >> 5) & 0x3F) as u8;
        let b = ((v >> 0) & 0x1F) as u8;

        slint::platform::software_renderer::PremultipliedRgbaColor {
            red: (r << 3) | (r >> 2),
            green: (g << 2) | (g >> 4),
            blue: (b << 3) | (b >> 2),
            alpha: 255,
        }
    }
}

impl From<slint::platform::software_renderer::PremultipliedRgbaColor> for Rgb565Pixel {
    #[inline]
    fn from(pixel: slint::platform::software_renderer::PremultipliedRgbaColor) -> Self {
        let r = (pixel.red as u16) >> 3;
        let g = (pixel.green as u16) >> 2;
        let b = (pixel.blue as u16) >> 3;
        Self((r << 11) | (g << 5) | b)
    }
}

impl slint::platform::software_renderer::TargetPixel for Rgb565Pixel {
    fn blend(&mut self, color: slint::platform::software_renderer::PremultipliedRgbaColor) {
        let mut x = slint::platform::software_renderer::PremultipliedRgbaColor::from(*self);
        x.blend(color);
        *self = x.into();
    }
    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        let r = (r as u16) >> 3;
        let g = (g as u16) >> 2;
        let b = (b as u16) >> 3;
        Self((r << 11) | (g << 5) | b)
    }
    fn background() -> Self {
        Self(0)
    }
}
