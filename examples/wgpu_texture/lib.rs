// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::include_modules!();

use slint::wgpu_27::{wgpu, WGPUConfiguration, WGPUSettings};

struct DemoRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    texture: wgpu::Texture,
    start_time: std::time::Instant,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PushConstants {
    light_color_and_time: [f32; 4],
}

impl DemoRenderer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "shader.wgsl"
            ))),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            push_constant_ranges: &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::FRAGMENT,
                range: 0..16, // full size in bytes, aligned
            }],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::TextureFormat::Rgba8UnormSrgb.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let texture = Self::create_texture(&device, 320, 200);

        Self {
            device: device.clone(),
            queue: queue.clone(),
            pipeline,
            texture,
            start_time: std::time::Instant::now(),
        }
    }

    fn create_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    fn render(
        &mut self,
        light_red: f32,
        light_green: f32,
        light_blue: f32,
        width: u32,
        height: u32,
    ) -> wgpu::Texture {
        if self.texture.size().width != width || self.texture.size().height != height {
            self.texture = Self::create_texture(&self.device, width, height);
        }

        let elapsed: f32 = self.start_time.elapsed().as_millis() as f32 / 500.;
        let push_constants =
            PushConstants { light_color_and_time: [light_red, light_green, light_blue, elapsed] };

        let mut encoder =
            self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rpass.set_pipeline(&self.pipeline);
            rpass.set_push_constants(
                wgpu::ShaderStages::FRAGMENT, // Stage (your constants are for fragment shader)
                0,                            // Offset in bytes (start at 0)
                bytemuck::bytes_of(&push_constants),
            );
            rpass.draw(0..3, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));

        self.texture.clone()
    }
}

pub fn main() {
    let mut wgpu_settings = WGPUSettings::default();
    wgpu_settings.device_required_features = wgpu::Features::PUSH_CONSTANTS;
    wgpu_settings.device_required_limits.max_push_constant_size = 16;

    slint::BackendSelector::new()
        .require_wgpu_27(WGPUConfiguration::Automatic(wgpu_settings))
        .select()
        .expect("Unable to create Slint backend with WGPU based renderer");

    let app = App::new().unwrap();

    let mut renderer = None;

    let app_weak = app.as_weak();

    app.window()
        .set_rendering_notifier(move |state, graphics_api| {
            //eprintln!("rendering state {:#?} {:#?}", state, graphics_api);

            match state {
                slint::RenderingState::RenderingSetup => {
                    match graphics_api {
                        slint::GraphicsAPI::WGPU27 { device, queue, .. } => {
                            renderer = Some(DemoRenderer::new(device, queue));
                        }
                        _ => return,
                    };
                }
                slint::RenderingState::BeforeRendering => {
                    if let (Some(renderer), Some(app)) = (renderer.as_mut(), app_weak.upgrade()) {
                        let texture = renderer.render(
                            app.get_selected_red(),
                            app.get_selected_green(),
                            app.get_selected_blue(),
                            app.get_requested_texture_width() as u32,
                            app.get_requested_texture_height() as u32,
                        );
                        app.set_texture(slint::Image::try_from(texture).unwrap());
                        app.window().request_redraw();
                    }
                }
                slint::RenderingState::AfterRendering => {}
                slint::RenderingState::RenderingTeardown => {
                    drop(renderer.take());
                }
                _ => {}
            }
        })
        .expect("Unable to set rendering notifier");

    app.run().unwrap();
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    slint::android::init(app).unwrap();
    main();
}
