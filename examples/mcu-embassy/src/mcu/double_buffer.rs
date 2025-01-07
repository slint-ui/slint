// Copyright © 2025 David Haig
// SPDX-License-Identifier: MIT

use embassy_stm32::ltdc::{self, Ltdc, LtdcLayerConfig};
use slint::platform::software_renderer::Rgb565Pixel;

use crate::slint_backend::TargetPixelType;

// A simple double buffer
pub struct DoubleBuffer {
    buf0: &'static mut [TargetPixelType],
    buf1: &'static mut [TargetPixelType],
    is_buf0: bool,
    layer_config: LtdcLayerConfig,
}

impl DoubleBuffer {
    pub fn new(
        buf0: &'static mut [TargetPixelType],
        buf1: &'static mut [TargetPixelType],
        layer_config: LtdcLayerConfig,
    ) -> Self {
        Self { buf0, buf1, is_buf0: true, layer_config }
    }

    pub fn current(&mut self) -> &mut [TargetPixelType] {
        if self.is_buf0 {
            self.buf0
        } else {
            self.buf1
        }
    }

    pub fn swap_temp(&mut self) {
        self.is_buf0 = !self.is_buf0;
    }

    pub async fn swap<T: ltdc::Instance>(
        &mut self,
        ltdc: &mut Ltdc<'_, T>,
    ) -> Result<(), ltdc::Error> {
        let buf = self.current();
        let frame_buffer = buf.as_ptr();
        self.is_buf0 = !self.is_buf0;
        ltdc.set_buffer(self.layer_config.layer, frame_buffer as *const _).await
    }

    // Clears the buffer
    pub fn clear(&mut self) {
        let buf = self.current();
        let solid_black = Rgb565Pixel::default();

        for a in buf.iter_mut() {
            *a = solid_black;
        }
    }
}
