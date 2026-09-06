// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Hands a scanout buffer between wgpu and the display controller, for renderers
//! that draw through wgpu alone.
//!
//! A scanout buffer is an exclusive-sharing Vulkan image. The display controller
//! only gets readable pixels if ownership is released to it after drawing, which
//! also resolves whatever compressed or reordered form the GPU kept the image in.
//! Skia emits that release through its own Vulkan access. A renderer that draws
//! purely through wgpu has nothing comparable: wgpu tracks image layouts itself
//! and knows nothing of queue-family ownership. So the barriers are recorded with
//! ash and submitted on wgpu's queue, between wgpu's own submissions.
//!
//! wgpu's view has to stay true at every point wgpu touches the image. wgpu leaves
//! a color target in `COLOR_ATTACHMENT_OPTIMAL` and only transitions on the next
//! use, so [`ScanoutBarriers::release`] pins that state through wgpu first, then
//! moves the image to `GENERAL` for the display. [`ScanoutBarriers::acquire`]
//! brings it back to `COLOR_ATTACHMENT_OPTIMAL` from `UNDEFINED` before wgpu draws
//! again, discarding the shown frame — the renderer repaints in full — which is
//! also why no ownership acquire has to match the release.

// cSpell: ignore dmabuf

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

pub struct ScanoutBarriers {
    device: wgpu::Device,
    queue: wgpu::Queue,
    command_pool: vk::CommandPool,
    /// One command buffer per barrier. Each is re-recorded every frame; the
    /// frame ends with a wait for the whole queue, so neither is still pending
    /// when the next frame records into it.
    acquire_commands: vk::CommandBuffer,
    release_commands: vk::CommandBuffer,
    queue_family_index: u32,
    /// Who the image is released to, see [`Self::new`].
    scanout_queue_family_index: u32,
}

impl ScanoutBarriers {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Self, PlatformError> {
        // Safety: the raw device only creates the pool and buffers here; both are
        // destroyed in `Drop`, while `device` is still alive.
        let (command_pool, commands, queue_family_index, scanout_queue_family_index) = unsafe {
            let hal_device = vulkan_device(device)?;
            let raw_device = hal_device.raw_device();
            let queue_family_index = hal_device.queue_family_index();

            // `VK_QUEUE_FAMILY_FOREIGN_EXT` names a consumer outside Vulkan, which the
            // display controller is; it needs `VK_EXT_queue_family_foreign`, which
            // `super::super::renderer::dmabuf::init_wgpu` asks for. Without it,
            // `VK_QUEUE_FAMILY_EXTERNAL` is the closest core Vulkan offers.
            let scanout_queue_family_index = if hal_device
                .enabled_device_extensions()
                .contains(&ash::ext::queue_family_foreign::NAME)
            {
                vk::QUEUE_FAMILY_FOREIGN_EXT
            } else {
                vk::QUEUE_FAMILY_EXTERNAL
            };

            let command_pool = raw_device
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::default()
                        .queue_family_index(queue_family_index)
                        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                    None,
                )
                .map_err(|e| format!("Error creating a command pool for scanout barriers: {e}"))?;

            let commands = raw_device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(command_pool)
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(2),
                )
                .map_err(|e| {
                    raw_device.destroy_command_pool(command_pool, None);
                    format!("Error allocating command buffers for scanout barriers: {e}")
                })?;

            (command_pool, commands, queue_family_index, scanout_queue_family_index)
        };

        Ok(Self {
            device: device.clone(),
            queue: queue.clone(),
            command_pool,
            acquire_commands: commands[0],
            release_commands: commands[1],
            queue_family_index,
            scanout_queue_family_index,
        })
    }

    /// Readies `texture` for wgpu to draw into, discarding what it shows.
    ///
    /// The image comes back from the display in `GENERAL`, owned by the display's
    /// queue family, while wgpu believes it is a color target. This moves it to
    /// `COLOR_ATTACHMENT_OPTIMAL` so that belief is true again. Vulkan accepts
    /// `UNDEFINED` as the old layout of any barrier, which discards the contents
    /// and needs no ownership acquire.
    pub fn acquire(&self, texture: &wgpu::Texture) -> Result<(), PlatformError> {
        let barrier = vk::ImageMemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            )
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .subresource_range(COLOR_SUBRESOURCE);
        self.submit_barrier(
            self.acquire_commands,
            texture,
            barrier,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
        )
    }

    /// Hands the drawn `texture` to the display controller.
    ///
    /// Submits after the renderer's work on wgpu's queue, so it is ordered behind
    /// it. Wait for the queue before scanning the buffer out.
    pub fn release(&self, texture: &wgpu::Texture) -> Result<(), PlatformError> {
        // Pin wgpu's view of the image before taking it away: wgpu transitions on
        // the next use, so whatever state the renderer left the image in, this
        // puts it in `COLOR_ATTACHMENT_OPTIMAL` and records that as wgpu's belief.
        // The barrier below then names the layout the image really is in.
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Slint scanout release"),
        });
        encoder.transition_resources(
            std::iter::empty(),
            std::iter::once(wgpu::TextureTransition {
                texture,
                selector: None,
                state: wgpu::TextureUses::COLOR_TARGET,
            }),
        );
        self.queue.submit(Some(encoder.finish()));

        let barrier = vk::ImageMemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_access_mask(vk::AccessFlags::empty())
            .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .new_layout(vk::ImageLayout::GENERAL)
            .src_queue_family_index(self.queue_family_index)
            .dst_queue_family_index(self.scanout_queue_family_index)
            .subresource_range(COLOR_SUBRESOURCE);
        self.submit_barrier(
            self.release_commands,
            texture,
            barrier,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
        )
    }

    fn submit_barrier(
        &self,
        commands: vk::CommandBuffer,
        texture: &wgpu::Texture,
        barrier: vk::ImageMemoryBarrier<'_>,
        src_stage: vk::PipelineStageFlags,
        dst_stage: vk::PipelineStageFlags,
    ) -> Result<(), PlatformError> {
        // Safety: `commands` belongs to this pool and is not pending, see the field
        // doc; the image handle is read from a live texture and used only within
        // this submission; the queue is wgpu's, submitted to from the one rendering
        // thread, in order with wgpu's own submissions.
        unsafe {
            let hal_device = vulkan_device(&self.device)?;
            let raw_device = hal_device.raw_device();
            let image = texture
                .as_hal::<wgpu::hal::api::Vulkan>()
                .ok_or_else(|| PlatformError::from("The scanout texture is not a Vulkan image"))?
                .raw_handle();
            let barrier = barrier.image(image);

            raw_device
                .begin_command_buffer(
                    commands,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .map_err(|e| format!("Error recording a scanout barrier: {e}"))?;
            raw_device.cmd_pipeline_barrier(
                commands,
                src_stage,
                dst_stage,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
            raw_device
                .end_command_buffer(commands)
                .map_err(|e| format!("Error recording a scanout barrier: {e}"))?;

            raw_device
                .queue_submit(
                    hal_device.raw_queue(),
                    &[vk::SubmitInfo::default().command_buffers(&[commands])],
                    vk::Fence::null(),
                )
                .map_err(|e| format!("Error submitting a scanout barrier: {e}"))?;
        }
        Ok(())
    }
}

impl Drop for ScanoutBarriers {
    fn drop(&mut self) {
        // Safety: the pool was created from this device in `new`. The wait makes
        // sure no command buffer from it is still executing.
        unsafe {
            if let Ok(hal_device) = vulkan_device(&self.device) {
                let raw_device = hal_device.raw_device();
                raw_device.queue_wait_idle(hal_device.raw_queue()).ok();
                raw_device.destroy_command_pool(self.command_pool, None);
            }
        }
    }
}

/// # Safety
/// The returned guard hands out raw Vulkan handles; see `wgpu::Device::as_hal`.
unsafe fn vulkan_device(
    device: &wgpu::Device,
) -> Result<impl std::ops::Deref<Target = wgpu::hal::vulkan::Device> + '_, PlatformError> {
    unsafe { device.as_hal::<wgpu::hal::api::Vulkan>() }
        .ok_or_else(|| PlatformError::from("The wgpu device is not a Vulkan device"))
}

#[cfg(test)]
mod tests {
    //! Drives the barriers against whatever Vulkan device the machine offers, with
    //! the validation layers loaded where they are installed, and fails on any
    //! validation error. That is the check on the reasoning in the module doc: at
    //! every point wgpu touches the image, the image is in the layout wgpu believes.
    //!
    //! An ordinary texture stands in for the dma-buf import; the barriers don't
    //! care where the image's memory came from. Without a Vulkan device the test
    //! passes vacuously and says so.

    use super::*;
    use crate::renderer::dmabuf::{open_device_with_queue_family_foreign, wait_for_gpu};

    /// Keeps every error wgpu logs, which is how wgpu-hal surfaces the validation
    /// layers' findings.
    struct ErrorRecorder(std::sync::Mutex<Vec<String>>);

    static RECORDER: ErrorRecorder = ErrorRecorder(std::sync::Mutex::new(Vec::new()));

    impl log::Log for ErrorRecorder {
        fn enabled(&self, metadata: &log::Metadata) -> bool {
            metadata.level() <= log::Level::Warn
        }
        fn log(&self, record: &log::Record) {
            eprintln!("[{} {}] {}", record.level(), record.target(), record.args());
            if record.level() == log::Level::Error {
                self.0.lock().unwrap().push(record.args().to_string());
            }
        }
        fn flush(&self) {}
    }

    #[test]
    fn every_frame_leaves_wgpu_and_the_display_in_agreement() {
        let _ = log::set_logger(&RECORDER).map(|()| log::set_max_level(log::LevelFilter::Warn));

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            flags: wgpu::InstanceFlags::VALIDATION | wgpu::InstanceFlags::DEBUG,
            backend_options: wgpu::BackendOptions::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            display: None,
        });
        let Ok(adapter) =
            spin_on::spin_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: None,
                apply_limit_buckets: false,
            }))
        else {
            eprintln!("No Vulkan adapter, so nothing to run the scanout barriers against");
            return;
        };
        eprintln!("Running against {}", adapter.get_info().name);

        // The device the dma-buf renderers get, minus the dma-buf import feature,
        // which the barriers don't need and a software driver may not have.
        let descriptor = wgpu::DeviceDescriptor {
            label: Some("scanout barrier test"),
            required_features: adapter.features() - wgpu::Features::all_experimental_mask(),
            required_limits: adapter.limits(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::default(),
        };
        let (device, queue) = open_device_with_queue_family_foreign(&adapter, &descriptor)
            .unwrap_or_else(|| spin_on::spin_on(adapter.request_device(&descriptor)))
            .expect("creating the device");

        let barriers = ScanoutBarriers::new(&device, &queue).expect("creating the barriers");
        eprintln!("Releasing to queue family {:#x}", barriers.scanout_queue_family_index);

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("stand-in scanout buffer"),
            size: wgpu::Extent3d { width: 64, height: 64, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Three frames: the first finds the image UNDEFINED, the others find it
        // GENERAL and released, which is the case the acquire exists for.
        for frame in 0..3 {
            barriers.acquire(&texture).expect("acquire");

            let mut encoder = device.create_command_encoder(&Default::default());
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frame"),
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

            barriers.release(&texture).expect("release");
            wait_for_gpu(&device).expect("wait");

            let errors = RECORDER.0.lock().unwrap();
            assert!(
                errors.is_empty(),
                "validation errors after frame {frame}:\n{}",
                errors.join("\n")
            );
        }
    }
}
