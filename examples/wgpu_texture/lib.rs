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

impl DemoRenderer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "shader.wgsl"
            ))),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: None, // Auto-layout
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
                targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
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
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    fn render(&self, _size: slint::PhysicalSize) -> wgpu::Texture {
        let _time = self.start_time.elapsed().as_secs_f32();
        let texture_view = self.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rpass.set_pipeline(&self.pipeline);
            rpass.draw(0..3, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));

        self.texture.clone()
    }
}

#[cfg(not(target_os = "android"))]
fn main() {
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
                        let texture = renderer.render(slint::PhysicalSize::new(
                            app.get_requested_texture_width() as u32,
                            app.get_requested_texture_height() as u32,
                        ));
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
fn main() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        flags: wgpu::InstanceFlags::empty(),
        ..Default::default()
    });

    let adapter = spin_on::spin_on(async {
        instance
            .request_adapter(&Default::default())
            .await
            .expect("Failed to find an appropriate WGPU adapter")
    });

    let (device, queue) = spin_on::spin_on(async {
        adapter.request_device(&Default::default()).await.expect("Failed to create WGPU device")
    });

    slint::BackendSelector::new()
        .require_wgpu_27(WGPUConfiguration::Manual { instance, adapter, device, queue })
        .select()
        .expect("Unable to create Slint backend with WGPU based renderer");

    let app = App::new().unwrap();

    let mut renderer = None;

    let app_weak = app.as_weak();

    app.window()
        .set_rendering_notifier(move |state, graphics_api| {
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
                        let texture = renderer.render(slint::PhysicalSize::new(
                            app.get_requested_texture_width() as u32,
                            app.get_requested_texture_height() as u32,
                        ));
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
