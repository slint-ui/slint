// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use alloc::boxed::Box;
use core::pin::Pin;
use euclid::num::Round;
use nema_gfx_rs::*;

use slint::platform::software_renderer;
use software_renderer::PhysicalRegion;

pub struct NemaGFXEnhancedBuffer<'a> {
    data: &'a mut [software_renderer::Rgb565Pixel],
    width: u32,
    height: u32,
    pixel_stride: usize,
    ops_count: usize,
    command_list: Option<Box<nema_cmdlist_t>>,
}

/// ```c
/// static inline uint32_t nema_blending_mode(uint32_t src_bf, uint32_t dst_bf, uint32_t blops) {
///    return ( (src_bf) | (dst_bf << 8) | (blops&NEMA_BLOP_MASK) );
/// }
/// ```
fn nema_blending_mode(src_bf: u32, dst_bf: u32, blops: u32) -> u32 {
    (src_bf) | (dst_bf << 8) | (blops & NEMA_BLOP_MASK)
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

    fn fill_background(&mut self, brush: &slint::Brush, region: &PhysicalRegion) -> bool {
        let slint::Brush::SolidColor(color) = brush else { return false };
        unsafe {
            self.ensure_command_list_bound();
            nema_set_clip(0, 0, self.width, self.height);
            nema_set_blend(
                NEMA_BF_ONE,
                nema_tex_t_NEMA_TEX0,
                nema_tex_t_NEMA_NOTEX,
                nema_tex_t_NEMA_NOTEX,
            );

            let color = nema_rgba(color.red(), color.green(), color.blue(), color.alpha());
            for (origin, size) in region.iter() {
                nema_fill_rect(
                    origin.x as i32,
                    origin.y as i32,
                    size.width as i32,
                    size.height as i32,
                    color,
                );
                self.ops_count += 1;
            }
        }
        true
    }

    fn draw_rectangle(
        &mut self,
        args: &software_renderer::DrawRectangleArgs,
        clip: &PhysicalRegion,
    ) -> bool {
        let radius = args.top_left_radius;
        if args.top_right_radius != radius
            || args.bottom_right_radius != radius
            || args.bottom_left_radius != radius
        {
            return false;
        }

        // TODO: gradients
        let slint::Brush::SolidColor(background) = args.background else { return false };
        self.ensure_command_list_bound();
        unsafe {
            nema_set_blend(
                NEMA_BF_ONE | (NEMA_BF_INVSRCALPHA << 8),
                nema_tex_t_NEMA_TEX0,
                nema_tex_t_NEMA_NOTEX,
                nema_tex_t_NEMA_NOTEX,
            );

            let color = nema_rgba(
                background.red(),
                background.green(),
                background.blue(),
                (background.alpha() as u16 * args.alpha as u16 / 255) as _,
            );
            let border_color = args.border.color();
            let border_color = nema_rgba(
                border_color.red(),
                border_color.green(),
                border_color.blue(),
                (border_color.alpha() as u16 * args.alpha as u16 / 255) as _,
            );
            for (origin, size) in clip.iter() {
                nema_set_clip(origin.x as _, origin.y as _, size.width as _, size.height as _);
                if radius <= 0. {
                    nema_fill_rect(
                        args.x.round() as i32,
                        args.y.round() as i32,
                        args.width as i32,
                        args.height as i32,
                        color,
                    );
                } else {
                    nema_fill_rounded_rect_aa(
                        args.x,
                        args.y,
                        args.width,
                        args.height,
                        radius,
                        color,
                    );
                }
                self.ops_count += 1;
                let b = args.border_width;
                if b > 0.1 {
                    nema_draw_rounded_rect_aa(
                        args.x + b / 2.0,
                        args.y + b / 2.0,
                        args.width - b,
                        args.height - b,
                        radius,
                        b,
                        border_color,
                    );
                    self.ops_count += 1;
                }
            }
        }
        true
    }

    fn draw_texture(
        &mut self,
        texture: &software_renderer::DrawTextureArgs,
        clip: &PhysicalRegion,
    ) -> bool {
        if texture.rotation != software_renderer::RenderingRotation::NoRotation
            || texture.tiling.is_some()
        {
            return false;
        }

        let source = texture.source();
        let (mut source_blend_factor, texture_format) = match source.pixel_format {
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

        self.ensure_command_list_bound();

        unsafe {
            nema_bind_src_tex(
                source.data.as_ptr() as _,
                source.width as _,
                source.height as _,
                texture_format,
                source.byte_stride as i32,
                NEMA_FILTER_PS as _,
            );

            nema_set_clip(0, 0, self.width, self.height);

            let mut blop = NEMA_BLOP_NONE;
            if let Some(colorize) = texture.colorize {
                if texture_format == NEMA_A8 {
                    // const color modulation doesn't seem to work with A8 textures, so instead, set the
                    // texture color for the missing channels.
                    nema_set_tex_color(nema_rgba(
                        colorize.red(),
                        colorize.green(),
                        colorize.blue(),
                        colorize.alpha(),
                    ));
                } else {
                    nema_set_recolor_color(nema_rgba(
                        colorize.red(),
                        colorize.green(),
                        colorize.blue(),
                        colorize.alpha(),
                    ));
                    blop = NEMA_BLOP_RECOLOR;
                    source_blend_factor = NEMA_BF_SRCALPHA;
                }
            }

            nema_set_blend(
                nema_blending_mode(source_blend_factor, NEMA_BF_INVSRCALPHA, blop),
                nema_tex_t_NEMA_TEX0,
                nema_tex_t_NEMA_TEX1,
                nema_tex_t_NEMA_NOTEX,
            );

            for (origin, size) in clip.iter() {
                nema_set_clip(origin.x as _, origin.y as _, size.width as _, size.height as _);
                nema_blit_subrect_fit(
                    texture.dst_x as i32,
                    texture.dst_y as i32,
                    texture.dst_width as i32,
                    texture.dst_height as i32,
                    0,
                    0,
                    source.width as _,
                    source.height as _,
                );
                self.ops_count += 1;
            }
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

            nema_bind_dst_tex(
                self.data.as_ptr() as _,
                self.width,
                self.height,
                NEMA_RGB565,
                (self.pixel_stride * core::mem::size_of::<software_renderer::Rgb565Pixel>()) as i32,
            );
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
