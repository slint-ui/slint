// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use core::pin::Pin;
use nema_gfx_rs::*;

use slint::platform::software_renderer;

pub struct NemaGFXEnhancedBuffer<'a> {
    data: &'a mut [software_renderer::Rgb565Pixel],
    width: u32,
    height: u32,
    pixel_stride: usize,
}

impl<'a> i_slint_core::software_renderer::TargetPixelBuffer
    for Pin<&mut NemaGFXEnhancedBuffer<'a>>
{
    type TargetPixel = software_renderer::Rgb565Pixel;
    fn line_slice(&mut self, line_number: usize) -> &mut [Self::TargetPixel] {
        let pixel_stride = self.pixel_stride;
        let offset = line_number * pixel_stride;
        &mut self.data[offset..offset + pixel_stride]
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
        //        defmt::info!("RECT {} {} {} {}", x, y, width, height);

        let mut cl = unsafe { nema_cl_create() };
        unsafe {
            nema_cl_bind(&mut cl);
        }

        unsafe {
            let mut scb = cortex_m::Peripherals::steal().SCB;
            scb.clean_invalidate_dcache_by_address(
                self.data.as_ptr() as _,
                self.data.len() * core::mem::size_of::<software_renderer::Rgb565Pixel>(),
            );
        }

        unsafe {
            nema_bind_dst_tex(
                self.data.as_ptr() as _,
                self.width,
                self.height,
                NEMA_RGB565,
                (self.pixel_stride * core::mem::size_of::<software_renderer::Rgb565Pixel>()) as i32,
            );

            nema_set_clip(0, 0, self.width, self.height);

            nema_set_blend(
                blending_mode_from_composition_mode(composition_mode),
                nema_tex_t_NEMA_TEX0,
                nema_tex_t_NEMA_NOTEX,
                nema_tex_t_NEMA_NOTEX,
            );

            let a16 = color.alpha as u16;
            let r = (color.red as u16 * 255u16 / a16) as u8;
            let g = (color.green as u16 * 255u16 / a16) as u8;
            let b = (color.blue as u16 * 255u16 / a16) as u8;

            nema_fill_rect(
                x as i32,
                y as i32,
                width as i32,
                height as i32,
                nema_rgba(r, g, b, color.alpha),
            );
            nema_bind_dst_tex(0 as _, 0, 0, NEMA_RGB565, 0);

            nema_cl_submit(&mut cl);
            nema_cl_wait(&mut cl);
            nema_cl_destroy(&mut cl);
            let _ = cl;
        }

        unsafe {
            let mut scb = cortex_m::Peripherals::steal().SCB;
            scb.invalidate_dcache_by_address(
                self.data.as_ptr() as _,
                self.data.len() * core::mem::size_of::<software_renderer::Rgb565Pixel>(),
            );
        }

        true
    }
}

impl<'a> NemaGFXEnhancedBuffer<'a> {
    pub fn new(
        data: &'a mut [software_renderer::Rgb565Pixel],
        width: u32,
        height: u32,
        pixel_stride: usize,
    ) -> Self {
        Self { data, width, height, pixel_stride }
    }
}

fn blending_mode_from_composition_mode(
    composition_mode: software_renderer::CompositionMode,
) -> u32 {
    match composition_mode {
        software_renderer::CompositionMode::Source => NEMA_BF_ONE,
        software_renderer::CompositionMode::SourceOver => NEMA_BF_ONE | (NEMA_BF_INVSRCALPHA << 8),
        _ => todo!(),
    }
}
