// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

extern crate alloc;

use slint::platform::software_renderer;

pub struct Dma2DEnhancedBuffer<'a> {
    data: &'a mut [software_renderer::Rgb565Pixel],
    pixel_stride: usize,
    pending_transfer: bool,
}

impl<'a> i_slint_core::software_renderer::TargetPixelBuffer for Dma2DEnhancedBuffer<'a> {
    type TargetPixel = software_renderer::Rgb565Pixel;

    fn line_slice(&mut self, line_number: usize) -> &mut [Self::TargetPixel] {
        self.wait_for_pending_transfer();
        let offset = line_number * self.pixel_stride;
        &mut self.data[offset..offset + self.pixel_stride]
    }

    fn num_lines(&self) -> usize {
        self.data.len() / self.pixel_stride
    }

    fn fill_rectangle(
        &mut self,
        x: i16,
        y: i16,
        width: i16,
        height: i16,
        color: software_renderer::PremultipliedRgbaColor,
        composition_mode: software_renderer::CompositionMode,
    ) -> bool {
        if color.alpha != 255
            && !matches!(composition_mode, software_renderer::CompositionMode::Source)
        {
            return false;
        }
        self.wait_for_pending_transfer();

        use embassy_stm32::pac::dma2d;
        let dma2d_registers = &embassy_stm32::pac::DMA2D;

        let begin = y as usize * self.pixel_stride as usize + x as usize;
        let to_fill = &mut self.data[begin..begin + width as usize];

        // Transfer mode: from color register to memory
        dma2d_registers.cr().modify(|w| w.set_mode(dma2d::vals::Mode::REGISTER_TO_MEMORY));
        // Output color mode
        dma2d_registers.opfccr().modify(|w| {
            w.set_cm(dma2d::vals::OpfccrCm::RGB565);
        });
        // Output offset
        dma2d_registers.oor().modify(|w| w.set_lo(self.pixel_stride as u16 - width as u16));

        // Number of pixels per line (pl) and number of lines (nl)
        dma2d_registers.nlr().modify(|w| {
            w.set_pl(width as u16);
            w.set_nl(height as u16);
        });

        // Output address
        dma2d_registers.omar().modify(|w| w.set_ma(to_fill.as_ptr() as u32));

        // Color register
        dma2d_registers.ocolr().modify(|w| {
            use i_slint_core::software_renderer::TargetPixel;
            let mut col = software_renderer::Rgb565Pixel::background();
            col.blend(color);
            w.0 = col.0 as u32;
        });

        self.start_transfer();

        true
    }
}

impl<'a> Dma2DEnhancedBuffer<'a> {
    pub fn new(data: &'a mut [software_renderer::Rgb565Pixel], pixel_stride: usize) -> Self {
        Self { data, pixel_stride, pending_transfer: false }
    }

    fn start_transfer(&mut self) {
        let dma2d_registers = &embassy_stm32::pac::DMA2D;
        dma2d_registers
            .cr()
            .modify(|w| w.set_start(embassy_stm32::pac::dma2d::vals::CrStart::START));
        self.pending_transfer = true;
    }
    fn wait_for_pending_transfer(&mut self) {
        if core::mem::take(&mut self.pending_transfer) {
            let dma2d_registers = &embassy_stm32::pac::DMA2D;
            while dma2d_registers.cr().read().start()
                == embassy_stm32::pac::dma2d::vals::CrStart::START
            {}
        }
    }
    pub fn enable_clock() {
        // enable dma2d clock
        embassy_stm32::pac::RCC.ahb1enr().modify(|w| w.set_dma2den(true));
        while embassy_stm32::pac::RCC.ahb1enr().read().dma2den() == false {}
    }
}

impl<'a> Drop for Dma2DEnhancedBuffer<'a> {
    fn drop(&mut self) {
        self.wait_for_pending_transfer();
    }
}
