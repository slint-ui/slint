// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

//! Delegate the rendering to the [`i_slint_core::software_renderer::SoftwareRenderer`]

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::platform::PlatformError;
pub use i_slint_core::software_renderer::SoftwareRenderer;
use i_slint_core::software_renderer::{PremultipliedRgbaColor, RepaintBufferType, TargetPixel};
use std::rc::Rc;

use crate::display::{Presenter, RenderingRotation};
use crate::drmoutput::DrmOutput;

pub struct SoftwareRendererAdapter {
    renderer: SoftwareRenderer,
    display: Rc<crate::display::swdisplay::SoftwareBufferDisplay>,
    size: PhysicalWindowSize,
}

#[repr(transparent)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DumbBufferPixel(pub u32);

impl From<DumbBufferPixel> for PremultipliedRgbaColor {
    #[inline]
    fn from(pixel: DumbBufferPixel) -> Self {
        let v = pixel.0;
        PremultipliedRgbaColor {
            red: (v >> 16) as u8,
            green: (v >> 8) as u8,
            blue: (v >> 0) as u8,
            alpha: (v >> 24) as u8,
        }
    }
}

impl From<PremultipliedRgbaColor> for DumbBufferPixel {
    #[inline]
    fn from(pixel: PremultipliedRgbaColor) -> Self {
        Self(
            (pixel.alpha as u32) << 24
                | ((pixel.red as u32) << 16)
                | ((pixel.green as u32) << 8)
                | (pixel.blue as u32),
        )
    }
}

impl TargetPixel for DumbBufferPixel {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let mut x = PremultipliedRgbaColor::from(*self);
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

impl SoftwareRendererAdapter {
    pub fn new(
        device_opener: &crate::DeviceOpener,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>, PlatformError> {
        let drm_output = DrmOutput::new(device_opener)?;
        let display = Rc::new(crate::display::swdisplay::SoftwareBufferDisplay::new(drm_output)?);

        let (width, height) = display.drm_output.size();
        let size = i_slint_core::api::PhysicalSize::new(width, height);

        let renderer = Box::new(Self { renderer: SoftwareRenderer::new(), display, size });

        eprintln!("Using Software renderer");

        Ok(renderer)
    }
}

impl crate::fullscreenwindowadapter::FullscreenRenderer for SoftwareRendererAdapter {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn is_ready_to_present(&self) -> bool {
        self.display.drm_output.is_ready_to_present()
    }

    fn render_and_present(
        &self,
        rotation: RenderingRotation,
        draw_mouse_cursor_callback: &dyn Fn(&mut dyn i_slint_core::item_rendering::ItemRenderer),
        ready_for_next_animation_frame: Box<dyn FnOnce()>,
    ) -> Result<(), PlatformError> {
        self.display.map_back_buffer(&mut |mut pixels, age| {
            self.renderer.set_repaint_buffer_type(match age {
                1 => RepaintBufferType::ReusedBuffer,
                2 => RepaintBufferType::SwappedBuffers,
                _ => RepaintBufferType::NewBuffer,
            });

            self.renderer.set_rendering_rotation(match rotation {
                RenderingRotation::NoRotation => {
                    i_slint_core::software_renderer::RenderingRotation::NoRotation
                }
                RenderingRotation::Rotate90 => {
                    i_slint_core::software_renderer::RenderingRotation::Rotate90
                }
                RenderingRotation::Rotate180 => {
                    i_slint_core::software_renderer::RenderingRotation::Rotate180
                }
                RenderingRotation::Rotate270 => {
                    i_slint_core::software_renderer::RenderingRotation::Rotate270
                }
            });

            let buffer: &mut [DumbBufferPixel] = bytemuck::cast_slice_mut(pixels.as_mut());
            self.renderer.render_with_post_render_callback(
                buffer,
                self.size.width as usize,
                Some(draw_mouse_cursor_callback),
            );

            Ok(())
        })?;
        self.display.present_with_next_frame_callback(ready_for_next_animation_frame)?;
        Ok(())
    }

    fn size(&self) -> i_slint_core::api::PhysicalSize {
        self.size
    }

    fn register_page_flip_handler(
        &self,
        event_loop_handle: crate::calloop_backend::EventLoopHandle,
    ) -> Result<(), PlatformError> {
        self.display.drm_output.register_page_flip_handler(event_loop_handle)
    }
}
