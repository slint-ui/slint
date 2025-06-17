// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use slint::wgpu_24::{wgpu, WGPUConfiguration, WGPUSettings};

slint::include_modules!();

struct WGPUUnderlay {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,

    start_time: web_time::Instant,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PushConstants {
    effect_time: f32,
    rotation_time: f32,
}

impl WGPUUnderlay {
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
                targets: &[Some(wgpu::TextureFormat::Bgra8Unorm.into())], // FIXME: use surface config
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            device: device.clone(),
            queue: queue.clone(),
            pipeline,
            start_time: web_time::Instant::now(),
        }
    }
}

impl WGPUUnderlay {
    fn render(
        &mut self,
        surface_texture: &wgpu::Texture,
        surface_config: &wgpu::SurfaceConfiguration,
        rotation_enabled: bool,
    ) {
        let elapsed = self.start_time.elapsed().as_millis() as f32;
        let push_constants = PushConstants {
            effect_time: elapsed,
            rotation_time: if rotation_enabled { elapsed } else { 0.0 },
        };

        let mut encoder =
            self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                        store: wgpu::StoreOp::Store,
                    },
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

        /*
        unsafe {
            let gl = &self.gl;

            gl.use_program(Some(self.program));

            // Retrieving the buffer with glow only works with native builds right now. For WASM this requires https://github.com/grovesNL/glow/pull/190
            // That means we can't properly restore the vao/vbo, but this is okay for now as this only works with femtovg, which doesn't rely on
            // these bindings to persist across frames.
            #[cfg(not(target_arch = "wasm32"))]
            let old_buffer =
                std::num::NonZeroU32::new(gl.get_parameter_i32(glow::ARRAY_BUFFER_BINDING) as u32)
                    .map(glow::NativeBuffer);
            #[cfg(target_arch = "wasm32")]
            let old_buffer = None;

            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));

            #[cfg(not(target_arch = "wasm32"))]
            let old_vao =
                std::num::NonZeroU32::new(gl.get_parameter_i32(glow::VERTEX_ARRAY_BINDING) as u32)
                    .map(glow::NativeVertexArray);
            #[cfg(target_arch = "wasm32")]
            let old_vao = None;

            gl.bind_vertex_array(Some(self.vao));

            let elapsed = self.start_time.elapsed().as_millis() as f32;
            gl.uniform_1_f32(Some(&self.effect_time_location), elapsed);
            gl.uniform_1_f32(
                Some(&self.rotation_time_location),
                if rotation_enabled { elapsed } else { 0.0 },
            );

            gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);

            gl.bind_buffer(glow::ARRAY_BUFFER, old_buffer);
            gl.bind_vertex_array(old_vao);
            gl.use_program(None);
        }
        */
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let mut wgpu_settings = WGPUSettings::default();
    wgpu_settings.device_required_features = wgpu::Features::PUSH_CONSTANTS;
    wgpu_settings.device_required_limits.max_push_constant_size = 16;

    slint::BackendSelector::new()
        .require_wgpu_24(WGPUConfiguration::Automatic(wgpu_settings))
        .select()
        .expect("Unable to create Slint backend with WGPU renderer");

    let app = App::new().unwrap();

    let mut underlay = None;

    let app_weak = app.as_weak();

    app.window()
        .set_rendering_notifier(move |state, graphics_api| {
            // eprintln!("rendering state {:#?}", state);

            match state {
                slint::RenderingState::RenderingSetup => {
                    match graphics_api {
                        slint::GraphicsAPI::WGPU24 { device, queue, .. } => {
                            underlay = Some(WGPUUnderlay::new(device, queue));
                        }
                        _ => return,
                    };
                }
                slint::RenderingState::BeforeRendering => {
                    let (surface_texture, surface_config) = match graphics_api {
                        slint::GraphicsAPI::WGPU24 {
                            surface_texture: Some(texture),
                            surface_configuration: Some(config),
                            ..
                        } => (texture, config),
                        _ => return,
                    };
                    if let (Some(underlay), Some(app)) = (underlay.as_mut(), app_weak.upgrade()) {
                        underlay.render(
                            surface_texture,
                            surface_config,
                            app.get_rotation_enabled(),
                        );
                        app.window().request_redraw();
                    }
                }
                slint::RenderingState::AfterRendering => {}
                slint::RenderingState::RenderingTeardown => {
                    drop(underlay.take());
                }
                _ => {}
            }
        })
        .expect("Unable to set rendering notifier");

    app.run().unwrap();
}
