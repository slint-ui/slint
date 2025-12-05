// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use super::surfman_context::SurfmanRenderingContext;
use winit::dpi::PhysicalSize;

#[derive(thiserror::Error, Debug)]
pub enum TextureError {
    #[error("{0:?}")]
    Surfman(surfman::Error),
    #[error("No surface returned when the surface was unbound from the context")]
    NoSurface,
}

/// A guard that unbinds a surface from the context on creation and rebinds it on drop.
///
/// This is useful for temporarily taking ownership of the surface to perform operations
/// that require it to be unbound, such as importing it into another API.
pub struct SurfaceGuard<'a> {
    context: &'a SurfmanRenderingContext,
    surface: Option<surfman::Surface>,
}

impl<'a> SurfaceGuard<'a> {
    /// Creates a new `SurfaceGuard`, unbinding the current surface from the context.
    pub fn new(context: &'a SurfmanRenderingContext) -> Result<Self, TextureError> {
        let mut surfman_context = context.context.borrow_mut();
        let surface = context
            .device
            .borrow()
            .unbind_surface_from_context(&mut surfman_context)
            .map_err(TextureError::Surfman)?
            .ok_or(TextureError::NoSurface)?;

        Ok(Self { context, surface: Some(surface) })
    }

    /// Returns a reference to the unbound surface.
    pub fn surface(&self) -> &surfman::Surface {
        self.surface.as_ref().unwrap()
    }
}

impl<'a> Drop for SurfaceGuard<'a> {
    fn drop(&mut self) {
        if let Some(surface) = self.surface.take() {
            let device = self.context.device.borrow();
            let mut context = self.context.context.borrow_mut();
            if let Err((_err, mut surface)) = device.bind_surface_to_context(&mut context, surface)
            {
                let _ = device.destroy_surface(&mut context, &mut surface);
            }
        }
    }
}

/// Helper function to create a `wgpu::TextureDescriptor`.
pub fn create_wgpu_texture_descriptor(
    size: PhysicalSize<u32>,
    label: &str,
    usage: wgpu::TextureUsages,
    format: wgpu::TextureFormat,
) -> wgpu::TextureDescriptor<'_> {
    wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: size.width, height: size.height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    }
}

/// Flips an image buffer vertically in place.
///
/// This is used to correct the orientation of textures read from OpenGL, which uses a
/// bottom-left origin, whereas other APIs (like WGPU/Metal/Vulkan) typically use top-left.
pub fn flip_image_vertically(
    pixels: &mut [u8],
    width: usize,
    height: usize,
    bytes_per_pixel: usize,
) {
    let stride = width * bytes_per_pixel;
    let mut row_buffer = vec![0u8; stride];
    for y in 0..height / 2 {
        let top_row_start = y * stride;
        let bottom_row_start = (height - y - 1) * stride;

        // Swap rows
        row_buffer.copy_from_slice(&pixels[top_row_start..top_row_start + stride]);
        pixels.copy_within(bottom_row_start..bottom_row_start + stride, top_row_start);
        pixels[bottom_row_start..bottom_row_start + stride].copy_from_slice(&row_buffer);
    }
}
