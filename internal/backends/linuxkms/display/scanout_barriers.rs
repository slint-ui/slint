// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Hands a scanout buffer between the renderer and the display controller.
//!
//! A scanout buffer is an exclusive-sharing Vulkan image.
//! The display controller only gets readable pixels if ownership is released to it after
//! drawing, which also resolves whatever compressed or reordered form the GPU kept it in.
//! wgpu knows nothing of queue-family ownership, so the barriers are recorded with ash,
//! and submitted on wgpu's queue between the renderer's own submissions.
//!
//! The renderer draws into the image as a color target in `COLOR_ATTACHMENT_OPTIMAL`.
//! [`ScanoutBarriers::acquire`] brings it there from `UNDEFINED` before each frame.
//! That discards the shown frame, which the renderer repaints in full anyway,
//! so no ownership acquire has to match the release.
//! [`ScanoutBarriers::release`] moves the image to `GENERAL` for the display.
//!
//! Where wgpu draws into the image, as with FemtoVG, it leaves the image as a color target in
//! `COLOR_ATTACHMENT_OPTIMAL`, the layout the release expects and the acquire restores.
//! Skia draws through its own Vulkan access, after wgpu recorded the image as a color target,
//! see `transition_to_color_target` in the Skia renderer.
//!
//! A release can also signal a semaphore, where the device exports semaphores as sync files.
//! [`ScanoutBarriers::release`] then returns the sync file, for the display to wait for.
//! A semaphore serves once: NXP's Vivante driver exports an already signaled sync file
//! from a semaphore signaled a second time.

// cSpell: ignore dmabuf POLLIN subresource unsignaled Vivante

use std::cell::{Cell, OnceCell};
use std::os::fd::{FromRawFd, OwnedFd};

use ash::vk;
use i_slint_core::platform::PlatformError;
use wgpu_30 as wgpu;

use crate::renderer::dmabuf::{EXTERNAL_SEMAPHORE_FD, vulkan_device, wait_for_gpu};

const COLOR_SUBRESOURCE: vk::ImageSubresourceRange = vk::ImageSubresourceRange {
    aspect_mask: vk::ImageAspectFlags::COLOR,
    base_mip_level: 0,
    level_count: 1,
    base_array_layer: 0,
    layer_count: 1,
};

/// The command buffers of one scanout buffer, re-recorded every time it's drawn into,
/// once `fence` signaled that the last release finished.
struct Slot {
    acquire: vk::CommandBuffer,
    release: vk::CommandBuffer,
    fence: vk::Fence,
    /// Whether a submitted release is still to signal `fence`.
    /// A failed submit leaves the fence unsignaled with nothing on the way to signal it.
    fence_pending: Cell<bool>,
    semaphore: Cell<vk::Semaphore>,
}

pub struct ScanoutBarriers {
    device: wgpu::Device,
    command_pool: vk::CommandPool,
    slots: Vec<Slot>,
    queue_family_index: u32,
    /// Who the image is released to, see
    /// [`scanout_queue_family_index`](crate::renderer::dmabuf::scanout_queue_family_index).
    scanout_queue_family_index: u32,
    external_semaphore_fd: Option<ash::khr::external_semaphore_fd::Device>,
    export_failed: Cell<bool>,
    /// The error that stopped rendering, see [`Self::wait_after_failure`].
    stopped: OnceCell<String>,
}

impl ScanoutBarriers {
    /// Barriers for `buffer_count` scanout buffers, addressed by their index.
    pub fn new(device: &wgpu::Device, buffer_count: usize) -> Result<Self, PlatformError> {
        // Safety: the raw device only creates the pool, the buffers and the fences here;
        // they're destroyed in `Drop`, while `device` is still alive.
        unsafe {
            let hal_device = vulkan_device(device)?;
            let raw_device = hal_device.raw_device();
            let queue_family_index = hal_device.queue_family_index();
            let scanout_queue_family_index =
                crate::renderer::dmabuf::scanout_queue_family_index(&hal_device);
            let external_semaphore_fd = hal_device
                .enabled_device_extensions()
                .contains(&EXTERNAL_SEMAPHORE_FD)
                .then(|| {
                    ash::khr::external_semaphore_fd::Device::new(
                        hal_device.shared_instance().raw_instance(),
                        raw_device,
                    )
                });

            let command_pool = raw_device
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::default()
                        .queue_family_index(queue_family_index)
                        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                    None,
                )
                .map_err(|e| format!("Error creating a command pool for scanout barriers: {e}"))?;
            let mut barriers = Self {
                device: device.clone(),
                command_pool,
                slots: Vec::with_capacity(buffer_count),
                queue_family_index,
                scanout_queue_family_index,
                external_semaphore_fd,
                export_failed: Cell::new(false),
                stopped: OnceCell::new(),
            };

            let error = |e: vk::Result| format!("Error creating scanout barriers: {e}");
            for _ in 0..buffer_count {
                let commands = raw_device
                    .allocate_command_buffers(
                        &vk::CommandBufferAllocateInfo::default()
                            .command_pool(command_pool)
                            .level(vk::CommandBufferLevel::PRIMARY)
                            .command_buffer_count(2),
                    )
                    .map_err(error)?;
                let fence = raw_device
                    .create_fence(&vk::FenceCreateInfo::default(), None)
                    .map_err(error)?;
                barriers.slots.push(Slot {
                    acquire: commands[0],
                    release: commands[1],
                    fence,
                    fence_pending: Cell::new(false),
                    semaphore: Cell::new(vk::Semaphore::null()),
                });
            }
            Ok(barriers)
        }
    }

    /// Readies `texture`, the image of scanout buffer `index`, for drawing into,
    /// discarding what it shows.
    ///
    /// Call [`Self::wait_after_failure`] after a frame failed between this and [`Self::release`]:
    /// only the release's fence tells when the command buffers can be re-recorded.
    /// Fails once rendering has stopped.
    pub fn acquire(&self, index: usize, texture: &wgpu::Texture) -> Result<(), PlatformError> {
        if let Some(reason) = self.stopped.get() {
            return Err(format!("Rendering has stopped: {reason}").into());
        }
        let slot = &self.slots[index];
        if slot.fence_pending.get() {
            // Safety: the fence belongs to this device, and a submitted release signals it.
            unsafe {
                let hal_device = vulkan_device(&self.device)?;
                let raw_device = hal_device.raw_device();
                raw_device
                    .wait_for_fences(&[slot.fence], true, u64::MAX)
                    .and_then(|()| raw_device.reset_fences(&[slot.fence]))
                    .map_err(|e| self.stop(format!("Error waiting for a scanout barrier: {e}")))?;
            }
            slot.fence_pending.set(false);
        }
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
            slot.acquire,
            None,
            None,
            texture,
            barrier,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
        )
    }

    /// Hands the drawn `texture`, the image of scanout buffer `index`, to the display
    /// controller.
    ///
    /// Submits after the renderer's work on wgpu's queue, so it is ordered behind it.
    /// With `export_sync_file`, returns a sync file that becomes readable once the GPU finished
    /// both, where the device exports one.
    /// Otherwise, wait for the queue before scanning the buffer out.
    pub fn release(
        &self,
        index: usize,
        texture: &wgpu::Texture,
        export_sync_file: bool,
    ) -> Result<Option<OwnedFd>, PlatformError> {
        let slot = &self.slots[index];
        let semaphore = if export_sync_file { self.renew_semaphore(slot)? } else { None };
        let barrier = vk::ImageMemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_access_mask(vk::AccessFlags::empty())
            .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .new_layout(vk::ImageLayout::GENERAL)
            .src_queue_family_index(self.queue_family_index)
            .dst_queue_family_index(self.scanout_queue_family_index)
            .subresource_range(COLOR_SUBRESOURCE);
        self.submit_barrier(
            slot.release,
            Some(slot.fence),
            semaphore,
            texture,
            barrier,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
        )?;
        slot.fence_pending.set(true);
        Ok(semaphore.and_then(|semaphore| self.export(semaphore)))
    }

    fn renew_semaphore(&self, slot: &Slot) -> Result<Option<vk::Semaphore>, PlatformError> {
        if !self.exports_sync_files() {
            return Ok(None);
        }
        // Safety: the old semaphore was signaled by the release that `slot.fence` covers,
        // which `acquire` waited for, so nothing uses it anymore.
        unsafe {
            let hal_device = vulkan_device(&self.device)?;
            let raw_device = hal_device.raw_device();
            raw_device.destroy_semaphore(slot.semaphore.replace(vk::Semaphore::null()), None);
            let mut export = vk::ExportSemaphoreCreateInfo::default()
                .handle_types(vk::ExternalSemaphoreHandleTypeFlags::SYNC_FD);
            match raw_device
                .create_semaphore(&vk::SemaphoreCreateInfo::default().push_next(&mut export), None)
            {
                Ok(semaphore) => {
                    slot.semaphore.set(semaphore);
                    Ok(Some(semaphore))
                }
                Err(e) => {
                    self.fail_export(format!("Error creating a semaphore to export: {e}"));
                    Ok(None)
                }
            }
        }
    }

    /// The sync file of `semaphore`, whose signal is submitted.
    /// `None` where it signaled already, or the export failed.
    fn export(&self, semaphore: vk::Semaphore) -> Option<OwnedFd> {
        let external_semaphore_fd = self.external_semaphore_fd.as_ref()?;
        // Safety: the semaphore was created exportable as a sync file, and its signal is
        // submitted. A file descriptor the call returns is new and owned by the caller.
        unsafe {
            let fd = external_semaphore_fd
                .get_semaphore_fd(
                    &vk::SemaphoreGetFdInfoKHR::default()
                        .semaphore(semaphore)
                        .handle_type(vk::ExternalSemaphoreHandleTypeFlags::SYNC_FD),
                )
                .inspect_err(|e| {
                    self.fail_export(format!("Error exporting a scanout barrier's sync file: {e}"))
                })
                .ok()?;
            (fd >= 0).then(|| OwnedFd::from_raw_fd(fd))
        }
    }

    /// Whether [`Self::release`] can return a sync file.
    pub fn exports_sync_files(&self) -> bool {
        self.external_semaphore_fd.is_some() && !self.export_failed.get()
    }

    fn fail_export(&self, error: String) {
        eprintln!("{error}. The CPU waits for the GPU from now on");
        self.export_failed.set(true);
    }

    /// Waits for the GPU after a frame failed,
    /// which can leave barriers in flight that no release fence covers.
    /// Where that wait fails, rendering stops,
    /// since [`Self::acquire`] would re-record command buffers that may still be pending.
    pub fn wait_after_failure(&self) -> Result<(), PlatformError> {
        wait_for_gpu(&self.device).map_err(|e| self.stop(e.to_string()))
    }

    /// Whether rendering has stopped, see [`Self::wait_after_failure`].
    #[cfg(skia_wgpu_30)]
    pub fn has_stopped(&self) -> bool {
        self.stopped.get().is_some()
    }

    fn stop(&self, reason: String) -> PlatformError {
        format!("Rendering has stopped: {}", self.stopped.get_or_init(|| reason)).into()
    }

    fn submit_barrier(
        &self,
        commands: vk::CommandBuffer,
        fence: Option<vk::Fence>,
        signal: Option<vk::Semaphore>,
        texture: &wgpu::Texture,
        barrier: vk::ImageMemoryBarrier<'_>,
        src_stage: vk::PipelineStageFlags,
        dst_stage: vk::PipelineStageFlags,
    ) -> Result<(), PlatformError> {
        // Safety: `commands` belongs to this pool and isn't pending: its slot's fence
        // signaled, and the fence of a submission covers every earlier one on the queue.
        // The image handle is read from a live texture and used only within this submission;
        // the queue is wgpu's, submitted to from the one rendering thread, in order with the
        // renderer's own submissions.
        unsafe {
            let hal_device = vulkan_device(&self.device)?;
            let raw_device = hal_device.raw_device();
            let image = texture
                .as_hal::<wgpu::hal::api::Vulkan>()
                .ok_or_else(|| PlatformError::from("The scanout texture is not a Vulkan image"))?
                .raw_handle();
            let barrier = barrier.image(image);
            let error = |e: vk::Result| format!("Error submitting a scanout barrier: {e}");

            raw_device
                .begin_command_buffer(
                    commands,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .map_err(error)?;
            raw_device.cmd_pipeline_barrier(
                commands,
                src_stage,
                dst_stage,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
            raw_device.end_command_buffer(commands).map_err(error)?;

            raw_device
                .queue_submit(
                    hal_device.raw_queue(),
                    &[vk::SubmitInfo::default()
                        .command_buffers(&[commands])
                        .signal_semaphores(signal.as_slice())],
                    fence.unwrap_or_default(),
                )
                .map_err(error)?;
        }
        Ok(())
    }
}

impl Drop for ScanoutBarriers {
    fn drop(&mut self) {
        // Safety: the pool, the fences, and the semaphores were created from this device.
        // They're only destroyed once the queue is idle, so none of them is still in use,
        // and leaked otherwise.
        unsafe {
            if let Ok(hal_device) = vulkan_device(&self.device) {
                let raw_device = hal_device.raw_device();
                if raw_device.queue_wait_idle(hal_device.raw_queue()).is_ok() {
                    for slot in &self.slots {
                        raw_device.destroy_fence(slot.fence, None);
                        raw_device.destroy_semaphore(slot.semaphore.get(), None);
                    }
                    raw_device.destroy_command_pool(self.command_pool, None);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Drive the barriers against whatever Vulkan device the machine offers, with the
    //! validation layers loaded where they are installed, and fail on any validation
    //! error: at every point wgpu touches the image, the image is in the layout wgpu
    //! believes, and no command buffer is re-recorded while pending.
    //!
    //! Ordinary textures stand in for the dma-buf imports;
    //! the barriers don't care where the image's memory came from.
    //! Without a Vulkan device, the tests pass vacuously and say so.

    use super::*;
    use crate::renderer::dmabuf::{validation, wait_for_gpu};

    fn stand_in_scanout_buffers(device: &wgpu::Device) -> [wgpu::Texture; 2] {
        [0, 1].map(|_| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("stand-in scanout buffer"),
                size: wgpu::Extent3d { width: 64, height: 64, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Bgra8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        })
    }

    fn assert_no_validation_errors(round: usize) {
        let errors = validation::errors();
        assert!(errors.is_empty(), "validation errors after round {round}:\n{}", errors.join("\n"));
    }

    /// The way FemtoVG uses the barriers: wgpu draws each frame, and every frame waits for the
    /// GPU after its release.
    #[test]
    fn frames_drawn_by_wgpu_leave_wgpu_and_the_display_in_agreement() {
        let Some((device, queue)) = validation::device() else { return };

        let barriers = ScanoutBarriers::new(&device, 2).expect("barriers");
        eprintln!("Releasing to queue family {:#x}", barriers.scanout_queue_family_index);
        let textures = stand_in_scanout_buffers(&device);

        // Two rounds through both buffers: the first finds each image UNDEFINED, the
        // second finds it GENERAL and released, which is the case the acquire exists for.
        for round in 0..4 {
            let (index, texture) = (round % 2, &textures[round % 2]);
            barriers.acquire(index, texture).expect("acquire");

            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
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

            let sync_file = barriers.release(index, texture, false).expect("release");
            assert!(sync_file.is_none(), "a sync file nobody asked for");
            wait_for_gpu(&device).expect("wait");
            assert_no_validation_errors(round);
        }
    }

    /// The way Skia uses the barriers.
    /// wgpu records each image as a color target before the frame, as `render_to_texture` does,
    /// and the next frame starts before the GPU finished the last one.
    #[test]
    fn frames_drawn_past_wgpu_reuse_command_buffers_only_once_finished() {
        let Some((device, queue)) = validation::device() else { return };

        let barriers = ScanoutBarriers::new(&device, 2).expect("barriers");
        let textures = stand_in_scanout_buffers(&device);

        let mut sync_files = Vec::new();
        for round in 0..6 {
            let (index, texture) = (round % 2, &textures[round % 2]);
            barriers.acquire(index, texture).expect("acquire");
            let mut encoder = device.create_command_encoder(&Default::default());
            encoder.transition_resources(
                std::iter::empty(),
                std::iter::once(wgpu::TextureTransition {
                    texture,
                    selector: None,
                    state: wgpu::TextureUses::COLOR_TARGET,
                }),
            );
            queue.submit(Some(encoder.finish()));
            sync_files.extend(barriers.release(index, texture, true).expect("release"));
            assert_no_validation_errors(round);
        }
        wait_for_gpu(&device).expect("wait");
        assert_no_validation_errors(6);

        // A driver may return no sync file for a semaphore that signaled already.
        eprintln!("The device exported {} sync files", sync_files.len());
        if barriers.exports_sync_files() {
            assert!(!sync_files.is_empty(), "no sync file from a device that exports them");
        }
        for sync_file in &sync_files {
            use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
            use std::os::fd::AsFd;
            let mut fds = [PollFd::new(sync_file.as_fd(), PollFlags::POLLIN)];
            assert_eq!(poll(&mut fds, PollTimeout::ZERO), Ok(1), "a sync file still unsignaled");
        }
    }
}
