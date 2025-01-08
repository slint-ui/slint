// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Delegate the rendering to the [`i_slint_core::software_renderer::SoftwareRenderer`]

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::platform::PlatformError;
pub use i_slint_core::software_renderer::SoftwareRenderer;
use i_slint_core::software_renderer::{PremultipliedRgbaColor, RepaintBufferType, TargetPixel};
use std::rc::Rc;

use crate::display::RenderingRotation;

pub struct SoftwareRendererAdapter {
    renderer: SoftwareRenderer,
    display: Rc<dyn crate::display::swdisplay::SoftwareBufferDisplay>,
    presenter: Rc<dyn crate::display::Presenter>,
    size: PhysicalWindowSize,
}

#[repr(transparent)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DumbBufferPixelXrgb888(pub u32);

impl From<DumbBufferPixelXrgb888> for PremultipliedRgbaColor {
    #[inline]
    fn from(pixel: DumbBufferPixelXrgb888) -> Self {
        let v = pixel.0;
        PremultipliedRgbaColor {
            red: (v >> 16) as u8,
            green: (v >> 8) as u8,
            blue: (v >> 0) as u8,
            alpha: (v >> 24) as u8,
        }
    }
}

impl From<PremultipliedRgbaColor> for DumbBufferPixelXrgb888 {
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

impl TargetPixel for DumbBufferPixelXrgb888 {
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
        let display = crate::display::swdisplay::new(device_opener)?;

        let (width, height) = display.size();
        let size = i_slint_core::api::PhysicalSize::new(width, height);

        let renderer = Box::new(Self {
            renderer: SoftwareRenderer::new(),
            display: display.clone(),
            presenter: display.as_presenter(),
            size,
        });

        eprintln!("Using Software renderer");

        Ok(renderer)
    }
}

impl crate::fullscreenwindowadapter::FullscreenRenderer for SoftwareRendererAdapter {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn render_and_present(
        &self,
        rotation: RenderingRotation,
        _draw_mouse_cursor_callback: &dyn Fn(&mut dyn i_slint_core::item_rendering::ItemRenderer),
    ) -> Result<(), PlatformError> {
        self.display.map_back_buffer(&mut |pixels, age, format| {
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

            match format {
                drm::buffer::DrmFourcc::Xrgb8888 => {
                    let buffer: &mut [DumbBufferPixelXrgb888] =
                        bytemuck::cast_slice_mut(pixels.as_mut());
                    self.renderer.render(buffer, self.size.width as usize);
                }
                drm::buffer::DrmFourcc::Rgb565 => {
                    let buffer: &mut [i_slint_core::software_renderer::Rgb565Pixel] =
                        bytemuck::cast_slice_mut(pixels.as_mut());
                    self.renderer.render(buffer, self.size.width as usize);
                }
                _ => {
                    return Err(format!(
                        "Unsupported frame buffer format {format} used with software renderer"
                    )
                    .into())
                }
            }

            Ok(())
        })?;
        self.presenter.present()?;
        Ok(())
    }

    fn size(&self) -> i_slint_core::api::PhysicalSize {
        self.size
    }
}
