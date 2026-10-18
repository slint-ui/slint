// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use slint::platform::software_renderer::{
    DrawTextureArgs, PhysicalRegion, RenderingRotation, Rgb565BigEndianPixel, TargetPixel,
    TargetPixelBuffer, TexturePixelFormat,
};

pub struct FrameBuffer<'a> {
    pixels: &'a mut [Rgb565BigEndianPixel],
    stride: usize,
}

impl<'a> FrameBuffer<'a> {
    pub fn new(pixels: &'a mut [Rgb565BigEndianPixel], stride: usize) -> Self {
        assert!(stride != 0 && pixels.len().is_multiple_of(stride));
        Self { pixels, stride }
    }
}

impl TargetPixelBuffer for FrameBuffer<'_> {
    type TargetPixel = Rgb565BigEndianPixel;

    fn line_slice(&mut self, line: usize) -> &mut [Self::TargetPixel] {
        &mut self.pixels[line * self.stride..(line + 1) * self.stride]
    }

    fn num_lines(&self) -> usize {
        self.pixels.len() / self.stride
    }

    #[cfg_attr(target_arch = "arm", unsafe(link_section = ".data.ram_func"))]
    #[cfg_attr(target_arch = "arm", inline(never))]
    fn draw_texture(&mut self, args: &DrawTextureArgs, clip: &PhysicalRegion) -> bool {
        if args.alpha != 255
            || args.colorize.is_some()
            || args.tiling.is_some()
            || args.rotation != RenderingRotation::NoRotation
            || args.dst_x < 0
            || args.dst_y < 0
        {
            return false;
        }
        let source = args.source();
        let (width, height) = (source.width as usize, source.height as usize);
        let (x, y) = (args.dst_x as usize, args.dst_y as usize);
        if source.pixel_format != TexturePixelFormat::Rgb565
            || width == 0
            || height == 0
            || width != args.dst_width
            || height != args.dst_height
            || x > self.stride
            || width > self.stride - x
            || y > self.num_lines()
            || height > self.num_lines() - y
            || source.byte_stride < width * 2
            || (height - 1)
                .checked_mul(source.byte_stride)
                .and_then(|n| n.checked_add(width * 2))
                .is_none_or(|end| end > source.data.len())
        {
            return false;
        }
        for (origin, size) in clip.iter() {
            let left = (origin.x as i64).max(x as i64);
            let top = (origin.y as i64).max(y as i64);
            let right = (i64::from(origin.x) + i64::from(size.width)).min((x + width) as i64);
            let bottom = (i64::from(origin.y) + i64::from(size.height)).min((y + height) as i64);
            if left >= right || top >= bottom {
                continue;
            }
            let (left, top, right, bottom) =
                (left as usize, top as usize, right as usize, bottom as usize);
            for row in top..bottom {
                let start = (row - y) * source.byte_stride + (left - x) * 2;
                Self::TargetPixel::copy_from_rgb565(
                    &mut self.pixels[row * self.stride + left..row * self.stride + right],
                    &source.data[start..start + (right - left) * 2],
                );
            }
        }
        true
    }
}
