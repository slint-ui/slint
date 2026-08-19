// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#[repr(transparent)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Bgra8888Pixel(pub u32);

impl From<Bgra8888Pixel> for slint::platform::software_renderer::PremultipliedRgbaColor {
    #[inline]
    fn from(pixel: Bgra8888Pixel) -> Self {
        let v = pixel.0;
        slint::platform::software_renderer::PremultipliedRgbaColor {
            blue: (v >> 0) as u8,
            green: (v >> 8) as u8,
            red: (v >> 16) as u8,
            alpha: (v >> 24) as u8,
        }
    }
}

impl From<slint::platform::software_renderer::PremultipliedRgbaColor> for Bgra8888Pixel {
    #[inline]
    fn from(pixel: slint::platform::software_renderer::PremultipliedRgbaColor) -> Self {
        Self(
            (pixel.alpha as u32) << 24
                | ((pixel.red as u32) << 16)
                | ((pixel.green as u32) << 8)
                | (pixel.blue as u32),
        )
    }
}

impl slint::platform::software_renderer::TargetPixel for Bgra8888Pixel {
    fn blend(&mut self, color: slint::platform::software_renderer::PremultipliedRgbaColor) {
        let mut x = slint::platform::software_renderer::PremultipliedRgbaColor::from(*self);
        x.blend(color);
        *self = x.into();
    }
    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self(0xff000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }
    fn background() -> Self {
        Self(0)
    }
}
