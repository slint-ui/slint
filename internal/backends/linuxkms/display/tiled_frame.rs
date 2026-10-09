// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! An optimally tiled frame that the renderer draws into, copied into the linear scanout
//! buffer at the end of every frame. See [`super::gbmdmabufdisplay::GbmDmabufDisplay::enable_tiled_frame`].
//!
//! Between frames the tiled image is in `COLOR_ATTACHMENT_OPTIMAL`, which is what wgpu
//! believes after an application's render pass into it, and what the renderer leaves it
//! in. The copy's command buffer moves it out of that layout and back, and releases the
//! scanout buffer to the display.

// cSpell: ignore dmabuf subresource

use ash::vk;
use i_slint_core::platform::PlatformError;
use wgpu_30 as wgpu;

const COLOR_SUBRESOURCE: vk::ImageSubresourceRange = vk::ImageSubresourceRange {
    aspect_mask: vk::ImageAspectFlags::COLOR,
    base_mip_level: 0,
    level_count: 1,
    base_array_layer: 0,
    layer_count: 1,
};

pub struct TiledFrame {
    pub texture: wgpu::Texture,
    device: wgpu::Device,
    command_pool: vk::CommandPool,
    /// One command buffer and fence per scanout buffer, reused once its fence signaled.
    commands: Vec<vk::CommandBuffer>,
    fences: Vec<vk::Fence>,
    queue_family_index: u32,
    scanout_queue_family_index: u32,
}

impl TiledFrame {
    pub fn new(
        device: &wgpu::Device,
        size: wgpu::Extent3d,
        format: wgpu::TextureFormat,
        buffer_count: usize,
    ) -> Result<Self, PlatformError> {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Slint linuxkms tiled frame"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            // The usages Skia's `render_to_texture` requires.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[format.add_srgb_suffix()],
        });
        unsafe {
            let hal_device = crate::renderer::dmabuf::vulkan_device(device)?;
            let raw_device = hal_device.raw_device();
            let queue_family_index = hal_device.queue_family_index();
            let scanout_queue_family_index =
                crate::renderer::dmabuf::scanout_queue_family_index(&hal_device);
            let command_pool = raw_device
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::default()
                        .queue_family_index(queue_family_index)
                        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                    None,
                )
                .map_err(|e| format!("Error creating a command pool for the frame copy: {e}"))?;
            let commands = raw_device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(command_pool)
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(buffer_count as u32),
                )
                .map_err(|e| format!("Error allocating command buffers for the frame copy: {e}"))?;
            let fences = (0..buffer_count)
                .map(|_| {
                    raw_device.create_fence(
                        &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                        None,
                    )
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Error creating fences for the frame copy: {e}"))?;
            Ok(Self {
                texture,
                device: device.clone(),
                command_pool,
                commands,
                fences,
                queue_family_index,
                scanout_queue_family_index,
            })
        }
    }

    /// Copies the frame into `scanout`, the image of scanout buffer `index`, and hands that
    /// to the display. Submits after the renderer's work on wgpu's queue.
    pub fn copy_into(&self, index: usize, scanout: &wgpu::Texture) -> Result<(), PlatformError> {
        unsafe {
            let hal_device = crate::renderer::dmabuf::vulkan_device(&self.device)?;
            let raw_device = hal_device.raw_device();
            let source = self
                .texture
                .as_hal::<wgpu::hal::api::Vulkan>()
                .ok_or_else(|| PlatformError::from("The frame is not a Vulkan image"))?
                .raw_handle();
            let destination = scanout
                .as_hal::<wgpu::hal::api::Vulkan>()
                .ok_or_else(|| PlatformError::from("The scanout buffer is not a Vulkan image"))?
                .raw_handle();
            let (commands, fence) = (self.commands[index], self.fences[index]);
            let error = |e: vk::Result| format!("Error copying the frame for scanout: {e}");
            raw_device.wait_for_fences(&[fence], true, u64::MAX).map_err(error)?;
            raw_device.reset_fences(&[fence]).map_err(error)?;
            raw_device
                .begin_command_buffer(
                    commands,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .map_err(error)?;
            // The scanout buffer's last frame is discarded, so no acquire matches the release.
            raw_device.cmd_pipeline_barrier(
                commands,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[
                    vk::ImageMemoryBarrier::default()
                        .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                        .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
                        .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .image(source)
                        .subresource_range(COLOR_SUBRESOURCE),
                    vk::ImageMemoryBarrier::default()
                        .src_access_mask(vk::AccessFlags::empty())
                        .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .image(destination)
                        .subresource_range(COLOR_SUBRESOURCE),
                ],
            );
            let size = self.texture.size();
            let layers = vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            };
            raw_device.cmd_copy_image(
                commands,
                source,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                destination,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[vk::ImageCopy::default()
                    .src_subresource(layers)
                    .dst_subresource(layers)
                    .extent(vk::Extent3D { width: size.width, height: size.height, depth: 1 })],
            );
            raw_device.cmd_pipeline_barrier(
                commands,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[
                    vk::ImageMemoryBarrier::default()
                        .src_access_mask(vk::AccessFlags::empty())
                        .dst_access_mask(
                            vk::AccessFlags::COLOR_ATTACHMENT_READ
                                | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                        )
                        .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                        .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                        .image(source)
                        .subresource_range(COLOR_SUBRESOURCE),
                    vk::ImageMemoryBarrier::default()
                        .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                        .dst_access_mask(vk::AccessFlags::empty())
                        .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                        .new_layout(vk::ImageLayout::GENERAL)
                        .src_queue_family_index(self.queue_family_index)
                        .dst_queue_family_index(self.scanout_queue_family_index)
                        .image(destination)
                        .subresource_range(COLOR_SUBRESOURCE),
                ],
            );
            raw_device.end_command_buffer(commands).map_err(error)?;
            raw_device
                .queue_submit(
                    hal_device.raw_queue(),
                    &[vk::SubmitInfo::default().command_buffers(&[commands])],
                    fence,
                )
                .map_err(error)?;
        }
        Ok(())
    }

    /// Waits for the copy into scanout buffer `index` to finish.
    pub fn wait(&self, index: usize) -> Result<(), PlatformError> {
        unsafe {
            let hal_device = crate::renderer::dmabuf::vulkan_device(&self.device)?;
            hal_device
                .raw_device()
                .wait_for_fences(&[self.fences[index]], true, u64::MAX)
                .map_err(|e| format!("Error waiting for the frame copy: {e}").into())
        }
    }
}

impl Drop for TiledFrame {
    fn drop(&mut self) {
        unsafe {
            if let Some(hal_device) = self.device.as_hal::<wgpu::hal::api::Vulkan>() {
                let raw_device = hal_device.raw_device();
                raw_device.queue_wait_idle(hal_device.raw_queue()).ok();
                for fence in &self.fences {
                    raw_device.destroy_fence(*fence, None);
                }
                raw_device.destroy_command_pool(self.command_pool, None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Drives the copy against whatever Vulkan device the machine offers, with the
    //! validation layers loaded where they are installed, and fails on any validation
    //! error: at every point wgpu touches the frame, the frame is in the layout wgpu
    //! believes.
    //!
    //! Ordinary textures stand in for the dma-buf imports. Without a Vulkan device the
    //! test passes vacuously and says so.

    use super::*;
    use crate::renderer::dmabuf::validation;

    #[test]
    fn every_frame_leaves_wgpu_and_the_display_in_agreement() {
        let Some((device, queue)) = validation::device() else { return };

        let size = wgpu::Extent3d { width: 64, height: 64, depth_or_array_layers: 1 };
        let format = wgpu::TextureFormat::Bgra8Unorm;
        let frame = TiledFrame::new(&device, size, format, 2).expect("creating the frame");
        let scanout_buffers = [0, 1].map(|_| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("stand-in scanout buffer"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        });
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(format.add_srgb_suffix()),
            ..Default::default()
        });

        // Two rounds through both scanout buffers: the first finds each one UNDEFINED,
        // the second finds it released to the display.
        for round in 0..4 {
            let index = round % 2;

            // An application drawing the background.
            let mut encoder = device.create_command_encoder(&Default::default());
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("background"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            queue.submit(Some(encoder.finish()));

            frame.copy_into(index, &scanout_buffers[index]).expect("copy");
            frame.wait(index).expect("wait");

            let errors = validation::errors();
            assert!(
                errors.is_empty(),
                "validation errors after round {round}:\n{}",
                errors.join("\n")
            );
        }
    }
}
