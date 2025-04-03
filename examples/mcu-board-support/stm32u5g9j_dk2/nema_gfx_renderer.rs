// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use alloc::boxed::Box;
use core::pin::Pin;
use nema_gfx_rs::*;

use slint::platform::software_renderer;

pub struct NemaGFXEnhancedBuffer<'a> {
    data: &'a mut [software_renderer::Rgb565Pixel],
    width: u32,
    height: u32,
    pixel_stride: usize,
    ops_count: usize,
    command_list: Option<Box<nema_cmdlist_t>>,
}

impl<'a> i_slint_core::software_renderer::TargetPixelBuffer
    for Pin<&mut NemaGFXEnhancedBuffer<'a>>
{
    type TargetPixel = software_renderer::Rgb565Pixel;
    fn line_slice(&mut self, line_number: usize) -> &mut [Self::TargetPixel] {
        let cnt = core::mem::take(&mut self.ops_count);
        if cnt > 0 {
            defmt::info!("OPS {}", cnt);
        }

        self.finish_command_list();

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
        self.ensure_command_list_bound();

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
                match composition_mode {
                    software_renderer::CompositionMode::Source => NEMA_BF_ONE,
                    software_renderer::CompositionMode::SourceOver => {
                        NEMA_BF_ONE | (NEMA_BF_INVSRCALPHA << 8)
                    }
                    _ => todo!(),
                },
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
        }

        self.ops_count += 1;

        true
    }

    fn draw_texture(
        &mut self,
        x: i16,
        y: i16,
        width: i16,
        height: i16,
        src_texture: software_renderer::Texture<'_>,
        colorize: u32,
        alpha: u8,
        rotation: software_renderer::RenderingRotation,
        composition_mode: software_renderer::CompositionMode,
    ) -> bool {
        let (source_blend_factor, texture_format) = match src_texture.pixel_format {
            software_renderer::TexturePixelFormat::Rgb => (NEMA_BF_SRCALPHA, NEMA_RGB24),
            software_renderer::TexturePixelFormat::Rgba => (NEMA_BF_SRCALPHA, NEMA_RGBA8888),
            software_renderer::TexturePixelFormat::RgbaPremultiplied => {
                (NEMA_BF_ONE, NEMA_RGBA8888)
            }
            software_renderer::TexturePixelFormat::AlphaMap => (NEMA_BF_SRCALPHA, NEMA_A8),
            software_renderer::TexturePixelFormat::SignedDistanceField => {
                return false;
            }
        };

        if src_texture.delta_x != (1 << 0x8) || src_texture.delta_y != (1 << 0x8) {
            return false;
        }

        //defmt::info!("BLIT");

        self.ensure_command_list_bound();

        unsafe {
            nema_bind_src_tex(
                src_texture.bytes.as_ptr() as _,
                src_texture.width as _,
                src_texture.height as _,
                texture_format,
                (src_texture.pixel_stride * src_texture.pixel_format.bpp() as u16) as i32,
                NEMA_FILTER_PS as _,
            );

            nema_bind_dst_tex(
                self.data.as_ptr() as _,
                self.width,
                self.height,
                NEMA_RGB565,
                (self.pixel_stride * core::mem::size_of::<software_renderer::Rgb565Pixel>()) as i32,
            );

            nema_set_clip(0, 0, self.width, self.height);

            let colorize = slint::Color::from_argb_encoded(colorize);

            nema_set_const_color(nema_rgba(
                colorize.red(),
                colorize.green(),
                colorize.blue(),
                alpha,
            ));

            if texture_format == NEMA_A8 {
                // const color modulation doesn't seem to work with A8 textures, so instead, set the
                // texture color for the missing channels.
                nema_set_tex_color(nema_rgba(colorize.red(), colorize.green(), colorize.blue(), 0));
            }

            nema_set_blend(
                source_blend_factor
                    | (match composition_mode {
                        software_renderer::CompositionMode::Source => NEMA_BF_ZERO,
                        software_renderer::CompositionMode::SourceOver => NEMA_BF_INVSRCALPHA,
                        _ => todo!(),
                    } << 8)
                    | (NEMA_BLOP_MODULATE_RGB & NEMA_BLOP_MASK),
                nema_tex_t_NEMA_TEX0,
                nema_tex_t_NEMA_TEX1,
                nema_tex_t_NEMA_NOTEX,
            );

            nema_blit_subrect(
                x as _,
                y as _,
                width as _,
                height as _,
                (src_texture.source_offset_x >> 4) as _,
                (src_texture.source_offset_y >> 4) as _,
            );
        }

        self.ops_count += 1;

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
        Self { data, width, height, pixel_stride, ops_count: 0, command_list: None }
    }

    fn ensure_command_list_bound(&mut self) {
        if self.command_list.is_some() {
            return;
        }

        unsafe {
            // Make sure that all data is written back to memory, so that the GPU sees it.
            let mut scb = cortex_m::Peripherals::steal().SCB;
            scb.clean_invalidate_dcache_by_address(
                self.data.as_ptr() as _,
                self.data.len() * core::mem::size_of::<software_renderer::Rgb565Pixel>(),
            );

            let mut cl = Box::new(nema_cl_create());
            nema_cl_bind_circular(cl.as_mut());
            self.command_list = Some(cl);
        }
    }

    fn finish_command_list(&mut self) {
        let Some(mut cl) = self.command_list.take() else { return };
        unsafe {
            nema_cl_submit(cl.as_mut());
            nema_cl_wait(cl.as_mut());
            nema_cl_destroy(cl.as_mut());

            let mut scb = cortex_m::Peripherals::steal().SCB;
            scb.invalidate_dcache_by_address(
                self.data.as_ptr() as _,
                self.data.len() * core::mem::size_of::<software_renderer::Rgb565Pixel>(),
            );
        }
    }
}

impl<'a> Drop for NemaGFXEnhancedBuffer<'a> {
    fn drop(&mut self) {
        self.finish_command_list();
    }
}
