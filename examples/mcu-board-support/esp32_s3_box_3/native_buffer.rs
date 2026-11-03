// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use slint::platform::software_renderer::{TargetPixel, TargetPixelBuffer};

pub struct FrameBuffer<'a, P> {
    pixels: &'a mut [P],
    stride: usize,
}

impl<'a, P> FrameBuffer<'a, P> {
    pub fn new(pixels: &'a mut [P], stride: usize) -> Self {
        assert!(stride != 0 && pixels.len().is_multiple_of(stride));
        Self { pixels, stride }
    }
}

impl<P: TargetPixel> TargetPixelBuffer for FrameBuffer<'_, P> {
    type TargetPixel = P;

    fn line_slice(&mut self, line: usize) -> &mut [Self::TargetPixel] {
        &mut self.pixels[line * self.stride..(line + 1) * self.stride]
    }

    fn num_lines(&self) -> usize {
        self.pixels.len() / self.stride
    }
}
