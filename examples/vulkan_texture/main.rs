// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Renders into a texture with plain Vulkan and shows it in a Slint scene.
//!
//! Slint renders with Skia on top of wgpu here, and wgpu is asked for its Vulkan backend. The
//! Vulkan handles this example draws with are the ones wgpu already created, pulled out of the
//! `GraphicsAPI::WGPU30` the rendering notifier hands over. Nothing here creates a device, a
//! queue or an instance of its own: sharing wgpu's is what lets Slint import the result without
//! a copy.

slint::include_modules!();

use ash::vk;
use slint::wgpu_30::{WGPUConfiguration, WGPUSettings, wgpu};

/// The one format wgpu, Vulkan and Skia's texture import all have to agree on.
const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const VULKAN_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;

/// The render target, and the Vulkan objects that only make sense for that one image.
struct Target {
    texture: wgpu::Texture,
    image_view: vk::ImageView,
    framebuffer: vk::Framebuffer,
}

impl Target {
    fn new(
        wgpu_device: &wgpu::Device,
        device: &ash::Device,
        render_pass: vk::RenderPass,
        width: u32,
        height: u32,
    ) -> Option<Self> {
        // The texture is allocated by wgpu, not by us: wgpu owns the memory and the lifetime,
        // and `slint::Image::try_from` takes a `wgpu::Texture`. We only ever borrow the VkImage
        // underneath it to render into.
        let texture = wgpu_device.create_texture(&wgpu::TextureDescriptor {
            label: Some("vulkan_texture target"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TEXTURE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        // SAFETY: the texture was just created by the Vulkan-backed wgpu device, and the image
        // is kept alive by `texture`, which this `Target` owns.
        let image = unsafe { texture.as_hal::<wgpu::wgc::api::Vulkan>()?.raw_handle() };

        let image_view = unsafe {
            device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(VULKAN_FORMAT)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .level_count(1)
                            .layer_count(1),
                    ),
                None,
            )
        }
        .ok()?;

        let framebuffer = unsafe {
            device.create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(render_pass)
                    .attachments(std::slice::from_ref(&image_view))
                    .width(width)
                    .height(height)
                    .layers(1),
                None,
            )
        }
        .ok()?;

        Some(Self { texture, image_view, framebuffer })
    }

    fn size(&self) -> (u32, u32) {
        let size = self.texture.size();
        (size.width, size.height)
    }

    /// # Safety
    /// The device must be idle, or the objects must otherwise no longer be in use.
    unsafe fn destroy(self, device: &ash::Device) {
        unsafe {
            device.destroy_framebuffer(self.framebuffer, None);
            device.destroy_image_view(self.image_view, None);
        }
    }
}

struct VulkanRenderer {
    wgpu_device: wgpu::Device,
    wgpu_queue: wgpu::Queue,
    device: ash::Device,
    queue: vk::Queue,
    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    /// Signalled once `command_buffer` has run, so we don't re-record it while it's in flight.
    in_flight: vk::Fence,
    /// Whether `in_flight` refers to a submission, i.e. whether it's worth waiting on.
    submitted: bool,
    target: Option<Target>,
    start_time: std::time::Instant,
}

/// What the fragment shader reads out of the push constant block.
#[repr(C)]
#[derive(Clone, Copy)]
struct PushConstants {
    light_color_and_time: [f32; 4],
}

impl VulkanRenderer {
    fn new(wgpu_device: &wgpu::Device, wgpu_queue: &wgpu::Queue) -> Option<Self> {
        // SAFETY: the hal guards only live for this block. The handles taken out of them stay
        // valid as long as the wgpu device and queue do, which the rendering notifier guarantees
        // between `RenderingSetup` and `RenderingTeardown`.
        let (device, queue, queue_family_index) = unsafe {
            let hal_device = wgpu_device.as_hal::<wgpu::wgc::api::Vulkan>()?;
            let hal_queue = wgpu_queue.as_hal::<wgpu::wgc::api::Vulkan>()?;
            // `ash::Device` is a handle plus a table of function pointers, so cloning it is
            // cheap and frees us from holding the guard.
            (hal_device.raw_device().clone(), hal_queue.as_raw(), hal_device.queue_family_index())
        };

        let render_pass = Self::create_render_pass(&device)?;
        let (pipeline_layout, pipeline) = Self::create_pipeline(&device, render_pass)?;

        let command_pool = unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(queue_family_index)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )
        }
        .ok()?;

        let command_buffer = unsafe {
            device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
        }
        .ok()?[0];

        let in_flight =
            unsafe { device.create_fence(&vk::FenceCreateInfo::default(), None) }.ok()?;

        Some(Self {
            wgpu_device: wgpu_device.clone(),
            wgpu_queue: wgpu_queue.clone(),
            device,
            queue,
            render_pass,
            pipeline_layout,
            pipeline,
            command_pool,
            command_buffer,
            in_flight,
            submitted: false,
            target: None,
            start_time: std::time::Instant::now(),
        })
    }

    /// The pipeline for the one draw call this example makes: a full-target triangle with the
    /// ray marcher in its fragment shader.
    fn create_pipeline(
        device: &ash::Device,
        render_pass: vk::RenderPass,
    ) -> Option<(vk::PipelineLayout, vk::Pipeline)> {
        // SPIR-V is checked in next to the GLSL it was built from, so building the example
        // doesn't need a shader compiler. See the README for how to regenerate it.
        let vertex_module = Self::create_shader_module(device, include_bytes!("shader.vert.spv"))?;
        let fragment_module =
            Self::create_shader_module(device, include_bytes!("shader.frag.spv"))?;

        let push_constant_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(size_of::<PushConstants>() as u32);

        let pipeline_layout = unsafe {
            device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .push_constant_ranges(std::slice::from_ref(&push_constant_range)),
                None,
            )
        }
        .ok()?;

        let entry_point = c"main";
        let stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vertex_module)
                .name(entry_point),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(fragment_module)
                .name(entry_point),
        ];

        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();
        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        // The target is resized with the window, so both are set at record time instead of
        // rebuilding the pipeline for every size.
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
        let viewport_state =
            vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
        let rasterization = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);
        let multisample = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA);
        let color_blend = vk::PipelineColorBlendStateCreateInfo::default()
            .attachments(std::slice::from_ref(&color_blend_attachment));

        let pipeline = unsafe {
            device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                std::slice::from_ref(
                    &vk::GraphicsPipelineCreateInfo::default()
                        .stages(&stages)
                        .vertex_input_state(&vertex_input)
                        .input_assembly_state(&input_assembly)
                        .viewport_state(&viewport_state)
                        .rasterization_state(&rasterization)
                        .multisample_state(&multisample)
                        .color_blend_state(&color_blend)
                        .dynamic_state(&dynamic_state)
                        .layout(pipeline_layout)
                        .render_pass(render_pass)
                        .subpass(0),
                ),
                None,
            )
        }
        .ok()
        .map(|pipelines| pipelines[0]);

        // The modules are only needed while the pipeline is being built.
        unsafe {
            device.destroy_shader_module(vertex_module, None);
            device.destroy_shader_module(fragment_module, None);
        }

        pipeline.map(|pipeline| (pipeline_layout, pipeline))
    }

    fn create_shader_module(device: &ash::Device, spirv: &[u8]) -> Option<vk::ShaderModule> {
        // `vkCreateShaderModule` takes words, and requires them to be aligned as such, which a
        // byte slice from `include_bytes!` is not.
        let code: Vec<u32> =
            spirv.chunks_exact(4).map(|word| u32::from_le_bytes(word.try_into().unwrap())).collect();
        unsafe {
            device.create_shader_module(
                &vk::ShaderModuleCreateInfo::default().code(&code),
                None,
            )
        }
        .ok()
    }

    /// A single-attachment render pass that hands the image back in the layout wgpu's resource
    /// tracker believes it to be in.
    fn create_render_pass(device: &ash::Device) -> Option<vk::RenderPass> {
        let attachment = vk::AttachmentDescription::default()
            .format(VULKAN_FORMAT)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            // We clear the whole image every frame, so we don't care what was in it. Starting
            // from UNDEFINED also means we don't have to track the layout Skia and wgpu left it
            // in, which is not something this example gets told about.
            .initial_layout(vk::ImageLayout::UNDEFINED)
            // We tell wgpu the texture is a `COLOR_TARGET` before submitting this (see
            // `render`), so leave it in the matching layout. Slint then barriers it from here to
            // the sampling layout when it imports the texture, and that barrier has to start
            // from the layout the image is really in.
            .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

        let color_reference = vk::AttachmentReference::default()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

        let subpass = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(std::slice::from_ref(&color_reference));

        // Two things touched this image before we get here, both on this same queue: our own
        // render pass last frame, and Skia sampling the result of it. The `UNDEFINED` initial
        // layout counts as a write, so the dependency has to cover both. The implicit external
        // dependency starts at TOP_OF_PIPE and wouldn't order us after either.
        let dependency = vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::FRAGMENT_SHADER,
            )
            .src_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::SHADER_READ,
            )
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);

        unsafe {
            device.create_render_pass(
                &vk::RenderPassCreateInfo::default()
                    .attachments(std::slice::from_ref(&attachment))
                    .subpasses(std::slice::from_ref(&subpass))
                    .dependencies(std::slice::from_ref(&dependency)),
                None,
            )
        }
        .ok()
    }

    fn render(&mut self, light_color: [f32; 3], width: u32, height: u32) -> Option<wgpu::Texture> {
        let (width, height) = (width.max(1), height.max(1));

        // Re-recording the command buffer, and dropping a target, both need the previous frame
        // to be off the GPU.
        if self.submitted {
            unsafe { self.device.wait_for_fences(&[self.in_flight], true, u64::MAX) }.ok()?;
        }

        if self.target.as_ref().is_none_or(|target| target.size() != (width, height)) {
            if let Some(old) = self.target.take() {
                unsafe { old.destroy(&self.device) };
            }
            self.target = Target::new(
                &self.wgpu_device,
                &self.device,
                self.render_pass,
                width,
                height,
            );
        }
        let target = self.target.as_ref()?;

        // Our render pass is invisible to wgpu's resource tracking, so tell wgpu the texture is
        // about to be written as a color attachment. Without this wgpu still believes the
        // texture is untouched, and the barrier it later emits to hand the texture to Skia for
        // sampling names a source scope that doesn't cover our writes.
        let mut encoder = self
            .wgpu_device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("vk handover") });
        encoder.transition_resources(
            std::iter::empty(),
            std::iter::once(wgpu::TextureTransition {
                texture: &target.texture,
                selector: None,
                state: wgpu::TextureUses::COLOR_TARGET,
            }),
        );
        self.wgpu_queue.submit(Some(encoder.finish()));

        unsafe {
            self.device.reset_fences(&[self.in_flight]).ok()?;
            self.device
                .reset_command_buffer(self.command_buffer, vk::CommandBufferResetFlags::empty())
                .ok()?;
            self.device
                .begin_command_buffer(
                    self.command_buffer,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .ok()?;

            let clear = vk::ClearValue {
                color: vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 1.0] },
            };
            self.device.cmd_begin_render_pass(
                self.command_buffer,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.render_pass)
                    .framebuffer(target.framebuffer)
                    .render_area(vk::Rect2D::default().extent(vk::Extent2D { width, height }))
                    .clear_values(std::slice::from_ref(&clear)),
                vk::SubpassContents::INLINE,
            );

            self.device.cmd_set_viewport(
                self.command_buffer,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: width as f32,
                    height: height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            self.device.cmd_set_scissor(
                self.command_buffer,
                0,
                &[vk::Rect2D::default().extent(vk::Extent2D { width, height })],
            );
            self.device.cmd_bind_pipeline(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline,
            );

            let elapsed = self.start_time.elapsed().as_millis() as f32 / 500.;
            let push_constants = PushConstants {
                light_color_and_time: [
                    light_color[0],
                    light_color[1],
                    light_color[2],
                    elapsed,
                ],
            };
            self.device.cmd_push_constants(
                self.command_buffer,
                self.pipeline_layout,
                vk::ShaderStageFlags::FRAGMENT,
                0,
                std::slice::from_raw_parts(
                    std::ptr::from_ref(&push_constants).cast::<u8>(),
                    size_of::<PushConstants>(),
                ),
            );

            self.device.cmd_draw(self.command_buffer, 3, 1, 0, 0);

            self.device.cmd_end_render_pass(self.command_buffer);
            self.device.end_command_buffer(self.command_buffer).ok()?;

            // The same queue Slint submits on, so submission order alone puts this drawing
            // ahead of the Skia work that samples the texture. No semaphores needed.
            self.device
                .queue_submit(
                    self.queue,
                    &[vk::SubmitInfo::default()
                        .command_buffers(std::slice::from_ref(&self.command_buffer))],
                    self.in_flight,
                )
                .ok()?;
        }
        self.submitted = true;

        Some(target.texture.clone())
    }
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        unsafe {
            // Everything below is potentially still in use by the queue.
            let _ = self.device.device_wait_idle();

            if let Some(target) = self.target.take() {
                target.destroy(&self.device);
            }
            self.device.destroy_fence(self.in_flight, None);
            self.device.destroy_command_pool(self.command_pool, None);
            self.device.destroy_pipeline(self.pipeline, None);
            self.device.destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_render_pass(self.render_pass, None);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Ask for wgpu, and for its Vulkan backend specifically: `as_hal::<Vulkan>()` returns None
    // on any other one.
    let mut wgpu_settings = WGPUSettings::default();
    wgpu_settings.backends = wgpu::Backends::VULKAN;

    slint::BackendSelector::new()
        .require_wgpu_30(WGPUConfiguration::Automatic(wgpu_settings))
        .select()?;

    let app = App::new()?;

    let mut renderer = None;
    let app_weak = app.as_weak();

    app.window().set_rendering_notifier(move |state, graphics_api| match state {
        slint::RenderingState::RenderingSetup => {
            if let slint::GraphicsAPI::WGPU30 { device, queue, .. } = graphics_api {
                renderer = VulkanRenderer::new(device, queue);
                if renderer.is_none() {
                    eprintln!("This example needs wgpu to be running on its Vulkan backend.");
                }
            }
        }
        slint::RenderingState::BeforeRendering => {
            let (Some(renderer), Some(app)) = (renderer.as_mut(), app_weak.upgrade()) else {
                return;
            };
            let texture = renderer.render(
                [app.get_selected_red(), app.get_selected_green(), app.get_selected_blue()],
                app.get_requested_texture_width() as u32,
                app.get_requested_texture_height() as u32,
            );
            if let Some(texture) = texture {
                app.set_texture(slint::Image::try_from(texture).unwrap());
            }
            // The effect animates, so keep frames coming.
            app.window().request_redraw();
        }
        slint::RenderingState::RenderingTeardown => {
            drop(renderer.take());
        }
        _ => {}
    })?;

    app.run()?;

    Ok(())
}
